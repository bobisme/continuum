# ADR-0029: Target semantic equivalence, not a TLA+ source clone

**Status:** Accepted  
**Date:** 2026-07-24  
**Amended:** 2026-07-31 — the oracle-shipping rule stated normatively (PR 0, plan §21.1)

## Context

The TLA+ corpus is a critical benchmark, but reproducing every parser, module-system, proof-language, TLC override, and historical edge case would consume the project and constrain better language design.

Users need the ability to express and verify the same systems and properties. They do not necessarily need every file to run unchanged.

The compatibility Tribunal, the staged TLA frontend, and differential testing all invoke foreign oracle tooling during development. Without a stated boundary, "used in the Tribunal" drifts into "bundled with the product": a Java runtime, TLC and SANY, Apalache, TLAPS, and solver binaries end up inside the release surface. That drift would expand the trusted computing base and the supply-chain surface, make the docs/12 §5 release assets depend on foreign toolchains, and put the reproducible-build covenant of plan §20 (INV-014) at the mercy of build systems Continuum does not pin. Plan §21.1 makes the rule a Phase A program decision and names this ADR as its home.

## Decision

Continuum 1.0 requires native semantic equivalents for the pinned corpus. Source compatibility is an independent interoperability lane.

The optional TLA frontend proceeds in stages:

1. SANY/TLC oracle export in the Tribunal;
2. import of a normalized semantic IR;
3. native parser/elaborator for the executable subset;
4. diagnostics and source mapping;
5. only then, broader compatibility.

Java or upstream tools are never silently invoked by release binaries.

### Normative rule: foreign oracle tooling does not ship

*Foreign oracle tooling* means TLC, SANY, and the rest of the `tlaplus/tlaplus` toolset; Apalache; TLAPS; and any external SMT, SAT, or CHC solver.

1. Continuum release artifacts — release binaries, installers, packaged distributions, container images, and the docs/12 §5 release assets — MUST NOT contain, bundle, vendor, or embed foreign oracle tooling, nor any runtime (such as a JVM) whose only purpose is to execute it.
2. Ordinary verification MUST NOT require foreign oracle tooling to be present. An installation with no oracle available MUST complete the ordinary verification path.
3. Release binaries MUST NOT silently invoke foreign tooling. Any invocation MUST be explicitly requested and MUST be recorded in the evidence graph with the tool's identity and version.
4. Where a lane cannot produce a result without an absent oracle, the result MUST be a typed `Unsupported` or inconclusive value in the assurance envelope. Silent downgrade to a weaker claim that still reads as success is prohibited.

The rule constrains what the project distributes, not what a user installs. Accordingly:

5. Foreign oracles MAY execute in the compatibility Tribunal, in CI, in development environments, and in user workflows where the user has independently installed the tool. Discovery of a user-installed tool at runtime is not shipping.
6. Every such invocation crosses exactly one adapter crate or process boundary (ADR-0019), and the assurance value of its output is governed by ADR-0017: solver-trusted unless an independently checked certificate exists.
7. Artifacts *derived* from an oracle run — normalized IR, transcripts, minimized fixtures, parity evidence — MAY ship, because they are data rather than tooling, provided their own licenses permit redistribution ([`corpus/tla-examples/REDISTRIBUTION_AUDIT.md`](../corpus/tla-examples/REDISTRIBUTION_AUDIT.md)).

## Consequences

The model language can be typed, fragment-aware, and designed for refinement into Rust. Corpus parity still prevents semantic retreat.

Users with existing TLA+ assets receive a migration path, but the product's core value does not wait on full language emulation.

**Release packaging.** The docs/12 §5 asset list — source, locked dependencies, toolchain, benchmark manifests, claim ledger, known unsupported features, certificate schemas, conformance corpus — contains no foreign executable, and the packaging job is a Rust-only build from pinned sources. The conformance-corpus asset needs one specific exclusion: the upstream corpus subtree vendors a third-party Java archive at `specifications/ewd998/impl/lib/gson-2.8.6.jar`, which a naive copy of the corpus tree would ship in violation of this rule, with a third-party notice obligation attached.

**Adapter isolation.** Each oracle lives behind one adapter boundary per ADR-0019, built and exercised by the Tribunal and CI rather than by the product. The kernel does not depend on any adapter, so the trusted checking base stays independent of foreign toolchain churn.

**Licensing.** Because oracles are not distributed, their upstream licenses do not propagate into Continuum's release artifacts and do not constrain the product license decision (plan §21.1, docs/12 §10). Verified 2026-07-31: `tlaplus/tlaplus` MIT, TLAPM BSD-2-Clause, Apalache Apache-2.0, Z3 MIT, cvc5 modified BSD — with a documented option to build against GPL-licensed libraries — kissat MIT, CaDiCaL MIT. The cvc5 build-configuration caveat is precisely the hazard this rule neutralizes: a solver Continuum never ships cannot impose its build's terms on Continuum's users.

**Costs.** The Tribunal's environment is heavier than the product's and must be described separately. Users who want oracle cross-checks install those tools themselves, and differential coverage is therefore unavailable in environments where they cannot be installed.

## Compatibility

No semantic epoch change. The rule constrains packaging and invocation, not semantics, and no artifact format changes.

## Security

Bundling foreign toolchains would enlarge both the TCB and the supply-chain surface described in docs/09. Under this rule a user-installed oracle runs behind a process boundary and its output is untrusted input to be normalized and checked, never a privileged claim.

## Alternatives considered

1. **Bundle TLC and a solver for convenience.** Best first-run experience, but it ships a JVM and foreign binaries, imports their licenses and vulnerabilities, and makes a success claim ambiguous about which tool produced it.
2. **Depend on oracles for ordinary verification and treat their absence as an error.** Simpler engineering, but it forfeits the native sovereignty stated in `corpus/tla-examples/CORPUS_POLICY.md` and makes Continuum a wrapper.
3. **Leave the boundary as convention.** The status quo before this amendment; conventions erode under release pressure, which is exactly when the cost is highest.

## Validation and rollback

Validation: a release-artifact check enumerates every shipped file and fails on JVM, `tlaplus`, Apalache, TLAPS, or solver binaries; a no-oracle installation test runs the ordinary verification path in an environment where none of those tools exists and asserts a typed result rather than a failure or a silent downgrade.

Rollback: if a lane is shown to be unbuildable without a bundled solver, the escape hatch is a separately distributed, explicitly named optional package carrying its own license manifest — never a silent addition to the default release binary — and it requires a superseding ADR.

## References

- [ADR-0017](0017-solver-and-tcb-policy.md) — solver evidence is trusted only when independently checked.
- [ADR-0019](0019-adapter-isolation.md) — one adapter boundary per foreign system.
- [ADR-0021](0021-tla-examples-corpus-contract.md) — the corpus release contract this rule protects.
- [docs/12](../docs/12_GOVERNANCE_AND_ENGINEERING.md) §5 release assets, §10 licensing and ecosystem.
- [plan](../plan.md) §20 adapter and dependency rules, §21.1 the Phase A program decisions.
- [`corpus/tla-examples/CORPUS_POLICY.md`](../corpus/tla-examples/CORPUS_POLICY.md) — "Upstream is an oracle, not a runtime dependency".
