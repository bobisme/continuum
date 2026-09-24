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
file; the two-revision half of GOV-1-08/09, which diffs the head against the
merge base, is `tools/governance/check_revision_delta.py` (15 fixture pairs).

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

- reference tests; (delivered: bn-37b1)
- metamorphic tests; (delivered: bn-37b1)
- differential tests; (delivered: bn-37b1)
- updated schemas; (delivered: bn-37b1)
- migration note; (delivered: bn-37b1)
- security review;
- claim impact. (delivered: bn-2tm3)

The first five of these obligations are enforced by the `gov-4-semantic`
obligation set, `tools/governance/obligations/gov4_semantic.py`, run by
`tools/governance/check_obligations.py` (self-testing, 37 fixture pairs plus 7
for the shared harness; evidence in
`tools/governance/evidence/gov-4-semantic.json`). A semantic change ships a
review record under `tools/governance/reviews/` that names its tests, schema
statement, and typed migration verdict, and the check resolves every name
against the tree and the delta. The security review is enforced at the record
level only and is not delivered: review state lives outside the repository,
so no committed artifact lets a checker confirm it. Claim impact extends the
same `[semantic]` record with `claim_impact`, checked by the
`gov-4-impact-reduction-pack` obligation set below.

### POR/reduction changes

Require:

- no-reduction differential campaign; (delivered: bn-2tm3)
- mutation of dependence relation; (delivered: bn-2tm3)
- liveness/property-class analysis; (delivered: bn-2tm3)
- performance report. (delivered: bn-2tm3)

These four obligations are enforced by the `gov-4-impact-reduction-pack`
obligation set, `tools/governance/obligations/gov4_impact_reduction_pack.py`,
run by the same harness (evidence in
`tools/governance/evidence/gov-4-impact-reduction-pack.json`). A change to
`continuum-engine-dpor` (docs/01 §7.2, the partial-order reduction engine)
ships a review record's `[por_reduction]` table: a no-reduction differential
test pairing the reduction engine against an unreduced oracle, a mutation
test naming one of docs/19 §4's dependence-relation mutation kinds, a typed
RFC 0014 property-preservation tier, and a performance report against one of
RFC 0014's `Evaluation` metrics.

### Pack changes

Require:

- fidelity profile; (delivered: bn-2tm3)
- host/Lab conformance; (delivered: bn-4x90)
- fault coverage; (delivered: bn-4x90)
- independence review;
- version bump. (delivered: bn-4x90)

The first of these obligations is enforced by the `gov-4-impact-reduction-pack`
obligation set (bn-2tm3). A change to a domain pack (a `continuum-effects-*`
crate, RFC 0002) ships a review record's `[pack]` table naming one of RFC
0002's required fidelity profiles. The `gov-4-pack-kernel` obligation set
(bn-4x90) extends the same `[pack]` table with the next three: host/Lab
conformance tests naming both paths, fault-coverage tests spanning at least
two of RFC 0002's fault-algebra operators, and a typed, increasing version
bump. Independence review is enforced at the record level only (a reviewer
distinct from the author, a review id, an `approved` verdict) and is not
delivered, for the same reason GOV-4-06's security review is not: Seal keeps
review state outside the repository, so no committed artifact lets a checker
confirm the named review exists, covers this change, and approved it.

Host/Lab conformance is deferred, not delivered, for a pack while no host path
exists (lead rulings, bn-3ohe and cr-1dl1d7). Such a pack's record may state
`[pack.host_conformance]` with `status = "not-applicable"`, a reason, a
`profile_test` and a `no_std_lane_test`, in place of host/Lab tests. The claim
that the pack performs no host effect rests on a compiler run, not on a source
scan. The pack must be `#![no_std]`, so it links only `core` and `alloc` and, with
`unsafe_code` forbidden, has no FFI route. Its lane test, run by `just check`,
compiles the dev and release host builds of a copy of the pack and demands the
unresolved-`std` error for a planted `std` use. The checker's own gate is
structural and lexical. It keeps the lane's builds the only builds: `#![no_std]`
as the crate root's first item; no `cfg`, `cfg_attr`, macro definition,
`include!`, `#[path]`, `asm!` or raw token in `src/`; exactly one `extern crate`,
which is `alloc`; no `build.rs`; and a manifest with only `[package]` and
`[lints] workspace = true`. The source rules are checked token by token by one
lexer that knows every token kind of the Rust Reference: every string, byte, C and
raw literal form, character literals, lifetimes and labels, and nested block
comments. So a line break, an alias or a literal does not hide a declaration, and
source the lexer cannot classify fails the rule (cr-35ujnx). The lexer is
`tools/governance/rust_lexer.py`, with a Rust twin that a test holds to it token for
token. The pack also reads nothing from its build environment at compile time: the
rules refuse `env!`, `option_env!`, `include!`, `include_str!`, `include_bytes!`,
`file!` and `cfg`, and the lane checks rustc's dep-info for both profiles, which
must name no `env-dep` variable and no file outside `src/`. The manifest rules are
never judged from the manifest text. The gating evidence is Cargo's resolved view,
`cargo metadata --no-deps --offline`, of the pack in its real workspace and of the
lane's copy. It must show no `custom-build` or `proc-macro` target, no `links`, no
dependency, no feature, and a library at the pack's own `src/lib.rs`. So a quoted,
dotted or inline-table `build` or `links` key, or a `build.rs` found with no key, is
refused (cr-35ujnx). The governance checker reads the manifest with `tomllib`, a
structural cross-check.
`#![no_std]` does not forbid `extern crate std`, so this rule carries that part of
the claim. Both named tests must be live, in the pack, and not
`should_panic`, `cfg`-gated or ignored. The lane test must plant the `std` use and
check both profiles, and it must show that a split, aliased `extern crate std`
compiles but fails the structure rule, and the profile test must `assert_eq!` the declared profile's
`host` to `HostQualification::None`, which every profile the pack declares must
also say (the docs/09 T06 claim of no host semantics). Any gap fails closed and
the full obligation applies, as it does for an empty reason or a record that
names neither the tests nor the form. When a host path lands, the pack stops
being `no_std` and the obligation applies in full.

A pack profile's name identifies its canonical bytes (RFC 0002 correction 1, bn-1oj6,
cr-37bshu). A pack change that alters those bytes publishes a new profile under a new
name and keeps the old profile byte for byte, so the version bump a pack change
requires is a new name, never an edit of a published one. Each pack's registry test
(the time pack's version test, for its one profile) pins every (name, fingerprint)
pair it declares, so a content change under a published name fails it; editing a
registry entry or removing a published profile is refused in review. An Intent
Contract moves to a new profile only by an intent revision that adds its name.

### Kernel changes

Require:

- two reviewers;
- fuzz corpus; (delivered: bn-4x90)
- mutation tests; (delivered: bn-2b4e)
- code-size report; (delivered: bn-2b4e)
- no unchecked optimization. (delivered: bn-2b4e)

The first two of these obligations are enforced by the same
`gov-4-pack-kernel` obligation set. A change to a `continuum-kernel-*` crate
(RFC 0005 "Kernel layering") ships a review record's `[kernel]` table naming a
committed fuzz corpus (files present at head, replayed by a fresh test that
reaches the change) and two reviewer entries. Two reviewers is enforced at the
record level only, for the same reason independence review above is not
delivered: the record names two distinct, non-author, approved reviewers, but
neither review's existence is checkable from a committed artifact. The
remaining three (mutation tests, code-size report, no unchecked optimization)
are a later Bone's.

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
