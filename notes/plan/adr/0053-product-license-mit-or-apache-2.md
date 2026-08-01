# ADR-0053: Product license is `MIT OR Apache-2.0`, and the asupersync rider is accepted and disclosed

**Status:** Accepted  
**Date:** 2026-07-31  
**Decision owners:** project owner (privileged decision; PR 0 program decisions, bone `bn-2br`)  
**Provenance:** project owner decision, 2026-07-31, on the record assembled in
[`notes/LICENSE_DECISION_PACKAGE.md`](../notes/LICENSE_DECISION_PACKAGE.md)

## Context

[plan](../plan.md) §21.1 and [docs/12](../docs/12_GOVERNANCE_AND_ENGINEERING.md) §10 required
a permissive product license "compatible with asupersync and solver adapters", and left the
choice as the last open PR 0 program decision. `Cargo.toml` carried an explicit deferral
comment in place of a `license` field, no `LICENSE` file existed, and the repository made no
statement to recipients about what they receive.

The decision package assembled the constraints and verified the upstream facts on 2026-07-31.
Two of those facts shape this ADR:

1. **The substrate is not under a standard permissive license.** asupersync 0.3.10 declares
   `LicenseRef-MIT-OpenAI-Anthropic-Rider` — MIT plus a rider that grants no rights to
   OpenAI, L.L.C., Anthropic, PBC, their affiliates, or anyone acting on their behalf, for
   their benefit, or under their direction, and that requires "any distribution of the
   Software or any Derivative Works [to] include this rider provision unmodified". There is
   no relicensing clause and no linking carve-out. [ADR-0001](0001-asupersync-execution-substrate.md)
   makes asupersync the execution substrate but records nothing about its license.
   Consequently "permissive" cannot be satisfied *transitively* for a binary that links the
   substrate, however Continuum licenses its own code.
2. **Solver licenses do not reach the product.** With
   [ADR-0029](0029-semantic-equivalence-not-tla-source-cloning.md)'s rule normative — foreign
   oracle tooling never ships in release artifacts — solvers are separately installed
   processes behind an adapter boundary, so their terms do not propagate into Continuum's
   release artifacts and do not constrain this choice. The pinned corpus likewise carries its
   own notices ([`corpus/tla-examples/REDISTRIBUTION_AUDIT.md`](../corpus/tla-examples/REDISTRIBUTION_AUDIT.md))
   and constrains nothing here beyond permitting third-party notices to ship alongside.

The package deliberately did not choose. This ADR records the choice that was made.

## Decision

### D0 — the asupersync rider exposure is **accepted and disclosed**

Continuum links asupersync as its execution substrate and accepts that the
`LicenseRef-MIT-OpenAI-Anthropic-Rider` passes through to distributions. The exposure is not
scoped away by making model-only the default build, not renegotiated with the licensor, and
not avoided by replacing the substrate. The obligation the project takes on instead is
**disclosure**: the rider's effect is stated in plain language where a recipient will see it
before acquiring a binary, rather than left to be discovered inside a bundled dependency
license listing.

### D1 — the product license is **`MIT OR Apache-2.0`**

First-party Continuum code, schemas, IDL, and certificate/CIR specification text are licensed
under the dual grant at the recipient's election. `LICENSE-MIT` and `LICENSE-APACHE` are the
canonical texts at the repository root; `[workspace.package] license = "MIT OR Apache-2.0"` is
the machine-readable form.

The dual grant is the Rust ecosystem default, it lets each recipient elect the term that fits
their constraint — Apache-2.0 for the express patent grant and retaliation clause that
enterprise toolchain review commonly requires of a component making correctness claims,
MIT where GPLv2 compatibility matters — and it keeps the open-format commitment of docs/12
§10, [ADR-0008](0008-proof-carrying-results.md), and
[ADR-0035](0035-proof-receipts-and-axiom-manifests.md) meaningful by making third-party
checker implementations against the schemas and certificate formats unambiguously permitted.

D1 changes nothing about D0: the election is over *Continuum's* grant, and no election
removes the rider from a binary that links asupersync.

## Consequences

**Rider disclosure is now a release obligation.** Any Continuum release artifact that links
asupersync — release binaries, installers, packaged distributions, container images — carries
the rider unmodified to every recipient, and its release notes and license bundle must say so.
Continuum cannot promise recipients an unencumbered binary for such a build, and must not
imply one.

**Model-only builds do not carry the rider.** [ADR-0001](0001-asupersync-execution-substrate.md)
states that "Continuum remains capable of model-only execution without asupersync", and plan
§20 forbids the model core from depending on it, so a build that excludes the substrate
distributes only first-party code under the dual grant plus its other dependencies' terms.
This is a real product configuration, not a hypothetical, and the disclosure must distinguish
the two cases rather than blanket every artifact with the rider.

**Inbound terms must match outbound.** A dual outbound grant requires that contributions
arrive under both licenses; until D3 below is settled, contributions are accepted on the
inbound=outbound understanding stated in the README's License section.

**Apache-2.0 inbound is now unambiguously compatible.** The one solver-adjacent license that
can reach the linked build — an Alethe checker crate derived from Carcara (Apache-2.0), which
Continuum would link rather than invoke — composes with an `MIT OR Apache-2.0` outbound grant
with the NOTICE obligation preserved. Under an MIT-only outbound this would have needed
separate handling.

**Crate metadata is consistent but publication stays closed.** `[workspace.package] license`
carries the expression and every crate inherits it with `license.workspace = true`, so
`cargo metadata` reports the grant for all 42 crates rather than leaving it unstated. Every
crate still sets `publish = false`; that remains a release-readiness decision, not a licensing
one, and lifting it needs no further license decision.

### Residual decisions — recorded, and adjustable without superseding this ADR

These are the decision package's D2–D6. Each is recorded at its current setting; changing one
is an ordinary edit to the artifact named, not a supersession of D0 or D1.

| # | Decision | Current setting | Adjustable by |
|---|---|---|---|
| D2 | scope of D1 | code, schemas, IDL, and certificate/CIR specification text; dossier prose (`notes/plan/**`) travels under the same grant rather than a separate documentation license | this table, if a CC-BY-4.0 split for prose is later preferred |
| D3 | inbound contribution terms | inbound=outbound, stated in the README; no DCO or CLA required yet | a `CONTRIBUTING` file plus an edit here |
| D4 | crate metadata | `[workspace.package] license` set and inherited by every crate (`license.workspace = true`); `publish = false` retained on every crate | `Cargo.toml`, when publication is wanted |
| D5 | copyright holder string | "Continuum contributors" — deliberately not a named entity, and expected to change if an entity is formed to hold the copyright | the `LICENSE-*` headers |
| D6 | trademark and name policy | undecided; out of license scope | a separate note |

## Compatibility

No semantic epoch change and no artifact format change. The `license` field is metadata; it
alters no build output. Nothing in the assurance envelope, the evidence graph, or any schema
depends on it.

## Security

Adding a `license` field and two text files changes no attack surface. The security-relevant
half is D0: accepting the rider means the project has a *disclosure* obligation it can fail —
a release that ships an asupersync-linked binary without carrying the rider text forward is a
license breach with automatic termination as its stated remedy, not merely a documentation
gap. That makes rider propagation a release-artifact property worth checking mechanically,
alongside the ADR-0029 no-foreign-oracle check.

## Performance hypothesis

None. This decision has no runtime or build-time effect.

## Alternatives considered

1. **MIT alone.** Simplest and GPLv2-compatible, but it extracts no patent grant from
   contributors and leaves downstream users with an implied-license argument of
   jurisdiction-dependent strength — weak for a project whose contributions (certifying DPOR
   under observer-indexed independence, proof-producing reductions, incremental reuse
   auditing) are patentable subject matter in some jurisdictions.
2. **Apache-2.0 alone.** Strongest patent posture, but GPLv2-incompatible, which would block a
   GPLv2 project from embedding a Continuum crate — a real if narrow loss for a verification
   library that should embed anywhere.
3. **BSD-3-Clause.** No patent grant, a non-endorsement clause to carry, and low Rust
   ecosystem fit; nothing it offers is not better supplied by the dual grant.
4. **Scoping the rider away (D0 alternative): make model-only the default and treat
   asupersync-linked builds as an opt-in.** Rejected: it would demote the substrate ADR-0001
   deliberately chose, and the wedge that makes Continuum useful on real code is exactly the
   asupersync-linked configuration.
5. **Negotiating alternate terms with the licensor, or replacing the substrate (D0
   alternatives).** Rejected as disproportionate to a bounded, disclosable exposure; either
   remains available later, and neither is foreclosed by accepting and disclosing now.

## Validation and rollback

Validation:

- `LICENSE-MIT` and `LICENSE-APACHE` exist at the repository root and `cargo metadata` reports
  `MIT OR Apache-2.0` for every workspace crate;
- the README's License section states both the dual grant and the rider pass-through, and
  docs/12 §10 names the chosen license rather than deferring it;
- when release packaging exists, its artifact check asserts that any artifact linking
  asupersync carries the rider text unmodified and that a model-only artifact does not claim
  the rider applies to it.

Rollback: D1 can be widened (adding a license to the disjunction) without recipient harm,
since existing recipients keep the grant they received; it cannot be narrowed retroactively,
so narrowing is only ever prospective and requires a superseding ADR. D0 can be revisited if
the substrate is replaced or its terms change — that too is a superseding ADR, not an edit
here. The D2–D6 residuals above are adjustable in place.

## References

- [`notes/LICENSE_DECISION_PACKAGE.md`](../notes/LICENSE_DECISION_PACKAGE.md) — the analysis,
  verified upstream facts, candidate comparison, and recommendation this decision accepts.
- [ADR-0001](0001-asupersync-execution-substrate.md) — asupersync as execution substrate, and
  the model-only capability that bounds the rider's reach.
- [ADR-0029](0029-semantic-equivalence-not-tla-source-cloning.md) — foreign oracle tooling does
  not ship, which is why solver licenses do not constrain this choice.
- [ADR-0017](0017-solver-and-tcb-policy.md), [ADR-0019](0019-adapter-isolation.md) — solver
  trust and adapter boundaries.
- [docs/12](../docs/12_GOVERNANCE_AND_ENGINEERING.md) §10 licensing and ecosystem, §5 release
  assets.
- [plan](../plan.md) §21.1 program decisions, §20 dependency rules.
- [`corpus/tla-examples/REDISTRIBUTION_AUDIT.md`](../corpus/tla-examples/REDISTRIBUTION_AUDIT.md)
  — corpus notices, unaffected by this decision.
