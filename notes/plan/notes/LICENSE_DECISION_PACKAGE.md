# Product License Decision Package

> **STATUS: PREPARED — AWAITING PRIVILEGED DECISION.**
> This document does not choose a license. It assembles the constraints, the verified
> facts, the candidate set, and a recommendation so that the choice can be made once and
> recorded. No `LICENSE` file has been added, no `license` field has been set in
> `Cargo.toml`, and no ADR has been written. Selecting the product license is a human
> action (plan §21.1 and the swarm execution map in
> [`START_HERE_IMPLEMENTATION.md`](START_HERE_IMPLEMENTATION.md)); an agent may only
> prepare the package.

**Prepared:** 2026-07-31 (PR 0 program decisions, bone `bn-2br`)
**Decision required by:** [plan](../plan.md) §21.1 — "the product license (permissive,
compatible with asupersync and solver adapters)"; [docs/12](../docs/12_GOVERNANCE_AND_ENGINEERING.md)
§10 — "Choose a permissive license compatible with asupersync and solver adapters."
**Consumers of the decision:** the workspace `[workspace.package] license` field (deferred
in `Cargo.toml` with a comment naming PR 0), `publish = false` on every crate, the docs/12
§5 release assets, and any public benchmark distribution.

---

## 1. What has to be decided

The plan states two constraints — *permissive*, and *compatible with asupersync and solver
adapters* — and nothing else. Everything below either verifies those constraints against
reality or surfaces a question the constraints do not answer.

## 2. Verified findings that change the shape of the decision

### F1 — asupersync is not under a standard permissive license (blocking)

[ADR-0001](../adr/0001-asupersync-execution-substrate.md) makes asupersync the execution
substrate. It does not record the substrate's license, and neither does any other document
in the dossier. Verified upstream on 2026-07-31:

| Fact | Value |
|---|---|
| Package | `asupersync` on crates.io, latest 0.3.10 |
| SPDX expression | `LicenseRef-MIT-OpenAI-Anthropic-Rider` |
| License file title | "MIT License (with OpenAI/Anthropic Rider)" |
| Restricted parties | OpenAI, L.L.C.; Anthropic, PBC; their affiliates; and any person or entity acting on their behalf, for their benefit, or under their direction |
| Grant to restricted parties | none ("no rights are granted to any Restricted Party") |
| Pass-through | required — "any distribution of the Software or any Derivative Works must include this rider provision unmodified" |
| Relicensing clause | none |
| Linking clause | none (the text says nothing about static or dynamic linking) |
| Breach effect | automatic termination, cessation and destruction of copies, injunctive relief, fees |

Consequences for this decision:

1. `LicenseRef-MIT-OpenAI-Anthropic-Rider` is not an OSI-approved license. A term that
   withholds rights from named parties fails the no-discrimination criteria, so
   "permissive" in plan §21.1 cannot be satisfied *transitively* however Continuum licenses
   its own code.
2. Continuum may still license its own source permissively — the license of a work is not
   dictated by its dependencies. But a Continuum release binary that links asupersync
   carries the rider to every recipient, and the pass-through clause requires the rider to
   travel with it. Continuum cannot promise recipients an unencumbered binary while
   asupersync is linked in, regardless of which candidate below is chosen.
3. The exposure is bounded by design: ADR-0001 already states that "Continuum remains
   capable of model-only execution without asupersync", and plan §20 forbids the model core
   from depending on it. A model-only build is therefore a real product configuration, not
   a hypothetical.
4. This is a fact about the program, not about the license choice, and it should be
   decided **before** or **with** the license — see decision D0.

### F2 — solver linkage is a narrow question, and [ADR-0029](../adr/0029-semantic-equivalence-not-tla-source-cloning.md) narrows it further

What the dossier actually says about solvers: [ADR-0017](../adr/0017-solver-and-tcb-policy.md)
treats SMT/SAT/CHC solvers as accelerators whose UNSAT results are trusted unless
independently checked; [ADR-0019](../adr/0019-adapter-isolation.md) gives each foreign
system one adapter crate or process boundary and keeps the kernel free of it; RFC 0005,
RFC 0012, and docs/10 name *proof formats* — SMT-LIB 2.x, Alethe, LRAT/FRAT — and docs/13
cites Carcara as a Rust Alethe checker. **No solver vendor is named anywhere as a
dependency.** The adapters touch normalized artifacts and proof formats, not a pinned
vendor.

With the ADR-0029 rule now normative — foreign oracle tooling, solvers included, never
ships in release binaries — solvers are separately installed processes invoked across an
adapter boundary. Their licenses do not propagate into Continuum's release artifacts.
Verified upstream licenses, for the record (2026-07-31): Z3 MIT; cvc5 modified BSD, whose
`COPYING` warns it "can be configured (however, by default it is not) to link against some
GPLed libraries"; kissat MIT; CaDiCaL MIT; Carcara Apache-2.0; Apalache Apache-2.0;
`tlaplus/tlaplus` (TLC/SANY) MIT; TLAPM BSD-2-Clause.

Two residual points remain for the human:

- The one place a solver-adjacent license does reach the linked TCB is a Rust checker crate
  Continuum links rather than invokes — an Alethe checker derived from Carcara would bring
  Apache-2.0 inbound terms into the kernel-adjacent build. Apache-2.0 inbound is compatible
  with an Apache-2.0 or dual-licensed outbound, and with MIT outbound only in the usual
  "permissive-in, permissive-out" sense, with the NOTICE obligation preserved.
- The cvc5 GPL-linkable build configuration is a live hazard for anyone who *ships* a
  cvc5 build. ADR-0029 means Continuum does not.

### F3 — the corpus does not constrain the product license

The pinned `tlaplus/Examples` corpus is MIT at the repository level, with three permissive
per-family exceptions. Details and the per-family verdicts are in
[`corpus/tla-examples/REDISTRIBUTION_AUDIT.md`](../corpus/tla-examples/REDISTRIBUTION_AUDIT.md).
The corpus is a separate distributed artifact carrying its own notices; it does not
interact with the choice below beyond requiring that whatever is chosen permit shipping
third-party notices alongside it (every candidate does).

## 3. Candidates

All four satisfy "permissive". They differ in patent posture, notice burden, and ecosystem
fit.

| Candidate | Patent grant | Notice burden | Rust ecosystem fit | GPLv2 compatibility |
|---|---|---|---|---|
| MIT | none (implied-license argument only) | one short notice | very high | yes |
| Apache-2.0 | express grant + retaliation termination (§3) | NOTICE file, modification marking (§4) | high | no (GPLv3 only) |
| `MIT OR Apache-2.0` | recipient may elect it | both files, standard Rust convention | highest — the de facto default | yes, via the MIT election |
| BSD-3-Clause | none | notice + non-endorsement clause | low in Rust | yes |

### Patent considerations for a verification-tool ecosystem

This matters more here than for an average library, for three reasons.

1. Continuum's own contributions are patentable subject matter in some jurisdictions —
   certifying DPOR under observer-indexed independence, proof-producing reductions,
   incremental reuse auditing. Contributors may hold or later obtain patents reading on
   code they contributed. Apache-2.0 §3 extracts an express grant from every contributor at
   contribution time; MIT extracts nothing and leaves downstream users with an implied-
   license argument whose strength varies by jurisdiction.
2. The adoption path in docs/12 §8 runs through enterprises with real distributed systems.
   Enterprise toolchain approval for a component that *makes correctness claims* commonly
   requires an express patent grant; MIT-only frequently triggers a legal review that
   Apache-2.0 does not.
3. Apache-2.0's retaliation clause (a patent suit against the project terminates the
   suer's patent license) is a defensive asset for a small project shipping novel
   algorithms.

The cost of Apache-2.0 alone is GPLv2 incompatibility, which would prevent a GPLv2 project
from embedding a Continuum crate — a real if narrow constraint for a verification library
that ideally embeds anywhere.

## 4. Recommendation (agent's reasoning — not a decision)

**`MIT OR Apache-2.0`, applied uniformly to first-party Continuum code, schemas, IDL, and
certificate/CIR specification text.**

Reasoning:

1. It is the Rust ecosystem default, so it minimizes friction for contributors, for
   downstream crates, and for the `cargo`-native distribution the plan assumes.
2. The recipient elects: enterprises take Apache-2.0 for the patent grant; GPLv2 projects
   take MIT. The project does not have to guess which constraint matters more.
3. It satisfies "permissive" in docs/12 §10 without weakening the open-format commitment —
   keeping certificate formats and CIR openly specified (docs/12 §10, ADR-0008, ADR-0035)
   is a licensing act as much as a documentation one, and a dual permissive grant on the
   schema and IDL files makes third-party checker implementations unambiguously allowed.
4. It changes nothing about F1. The rider travels with any binary that links asupersync no
   matter what Continuum chooses, which is why D0 is listed first below.

Secondary recommendations, each independently decidable:

- Release notes for any binary linking asupersync should state the rider's effect
  explicitly rather than let recipients discover it in a dependency license bundle.
- Dossier prose (`notes/plan/**` markdown) may reasonably carry a documentation license
  (CC-BY-4.0) rather than a code license, but a single dual grant over everything is
  simpler and adequate; this is a taste call, not a constraint.
- Corpus ports keep upstream notices per the redistribution audit regardless of the choice.

## 5. Decision checklist for the human

| # | Decision | Options | Recorded in |
|---|---|---|---|
| D0 | asupersync rider exposure | accept and disclose / scope to model-only default builds / negotiate alternate terms with the licensor / replace the substrate | amendment to ADR-0001 |
| D1 | product license | MIT / Apache-2.0 / `MIT OR Apache-2.0` / BSD-3-Clause / other | `LICENSE` file(s) at repo root + new ADR |
| D2 | scope of D1 | code only / code + schemas + IDL / everything including dossier prose | the ADR |
| D3 | inbound contribution terms | inbound=outbound statement (dual licensing requires one) / DCO / CLA | `CONTRIBUTING` + the ADR |
| D4 | crate metadata | set `[workspace.package] license`; decide when `publish = false` lifts | `Cargo.toml` |
| D5 | copyright holder string | "The Continuum contributors" / a named entity | `LICENSE` headers |
| D6 | trademark and name policy | out of license scope, usually decided together | separate note |

Recording the outcome takes four edits: the `LICENSE` file(s), the `Cargo.toml` workspace
field and its deferral comment, a new ADR (0053 is the next free number as of this
writing), and the docs/12 §10 sentence, which currently says "Choose a permissive license"
and should say which one was chosen.

## 6. What this package deliberately does not do

- It does not choose, and it does not pre-commit the project by adding files that imply a
  choice.
- It does not give legal advice. F1 in particular describes a license's plain terms and
  their engineering consequences; whether the rider is enforceable, and against whom, is a
  question for counsel.
- It does not treat the two remaining PR 0 program decisions as open: the ADR-0029 rule is
  recorded normatively in the ADR, and the corpus redistribution audit is complete. Only
  the license remains.

## 7. Evidence

All upstream facts were verified on 2026-07-31 against live sources: the crates.io API for
the asupersync package metadata, the project's `LICENSE` file for the rider terms, and
GitHub's license endpoint (with the license text read directly wherever the endpoint
reported `NOASSERTION`) for every solver, oracle, and corpus repository named above. Dossier
citations are to the files in this repository at the commit that introduced this document.
