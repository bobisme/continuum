# GOV §1 — code and semantic policy

Executable enforcement of the twelve obligations in
[`notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md`](../../notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md)
§1 "Repository constitution", carrying the requirement ids `GOV-1-01` … `GOV-1-12`
from `notes/plan/notes/PLAN_REQUIREMENTS.json`.

```
tools/governance/
├── check_code_policy.py          the twelve rules over one revision; stdlib only
├── check_revision_delta.py       GOV-1-08/09 over two revisions (merge base → head)
├── dependency-rationale.toml     GOV-1-07's rationale and TCB classification manifest
├── fixtures/code/<id>/           56 violating fixtures — inert data
├── fixtures/delta/<id>/          15 base→head fixture pairs — inert data
├── evidence/gov-1.json           retained output, keyed by requirement id
└── evidence/gov-1-delta.json     the delta gate's rules, fixtures, and degradation behaviour
```

## Running it

```sh
python3 tools/governance/check_code_policy.py --self-test   # every fixture must be caught
python3 tools/governance/check_code_policy.py               # the real check
python3 tools/governance/check_code_policy.py --evidence tools/governance/evidence/gov-1.json

python3 tools/governance/check_revision_delta.py --self-test          # every fixture pair
python3 tools/governance/check_revision_delta.py                      # merge base → working tree
python3 tools/governance/check_revision_delta.py --base origin/main --require-base   # CI
```

Exit 0 when every rule holds, 1 otherwise. `--evidence` runs the self-test *and*
the real check, then writes both into the evidence file — so a committed evidence
file cannot claim a fixture was caught unless it was. The delta gate is the one
exception, and deliberately: its `--evidence` writes the self-test and the gate's
static description but *not* the run's verdict, which is a function of two
revisions rather than of the tree (see "Evidence" below).

Both are wired into `just check` via the `governance` recipe, which runs each
checker's self-test before its real run.

## What each rule enforces

| Id | Rule | Enforced on the real repository |
|---|---|---|
| GOV-1-01 | `toolchain-pinned` | `rust-toolchain.toml` pins an exact `major.minor.patch` channel; `[workspace.package]` sets `edition` and the `rust-version` floor; every crate inherits the edition rather than pinning its own. |
| GOV-1-02 | `unsafe-forbidden` | `[workspace.lints.rust] unsafe_code = "forbid"`; every crate opts in with `[lints] workspace = true`; no manifest re-declares the lint; no source relaxes it by attribute; no `unsafe` item in any crate source, tests included. |
| GOV-1-03 | `deterministic-collections` | No `HashMap`/`HashSet`/`hash_map`/`hash_set`/`DashMap` symbol and no nondeterministic-collection dependency in the semantic core. The crate tiering is checked against plan §20 in the same rule. |
| GOV-1-04 | `no-ambient-time-rng` | No clock, RNG, or process-environment read, and no clock/RNG dependency, anywhere in the semantic core. |
| GOV-1-05 | `no-platform-hashing` | No platform hasher symbol or dependency in the semantic core; the identity seam still declares `trait ContentHasher` + `struct HashAlgorithm` and imports no `std::hash`; the canonical value module derives no `Hash`. |
| GOV-1-06 | `no-network-in-checker` | The workspace-internal closure of the four `continuum-kernel-*` crates and `continuum-certificate` contains no network-capable member, no network or async-runtime dependency, and no socket type. |
| GOV-1-07 | `dependency-rationale` | Every declared dependency has exactly one entry in `dependency-rationale.toml` with a one-line rationale, a TCB class, and accurate `origin`/`used_by`/`kinds`; anything the certificate checker links must be classified `trusted-checking-base`. |
| GOV-1-08 | `semantic-change-adr` | ADR files and `adr/README.md` are a bijection; every ADR declares a docs/12 §2 status; every `ADR-NNNN`/`RFC NNNN` citation in crate sources resolves; every semantic-core crate carrying code cites a numbered decision record. **Delta half** (`semantic-change-adr-delta`): every semantic-tier file the change touches or adds carries a cited record, or the change lands with one. |
| GOV-1-09 | `epoch-discipline` | The schemas README still states the `MUST advance schema_epoch` rule; every schema's `$id` epoch equals its `schema_epoch`; every instance names an epoch its schema is; the six epoch kinds in code and in prose are one set; `EpochAdvance::new` requires a `Compatibility` verdict. **Delta half** (`epoch-discipline-delta`): a schema whose bytes changed advanced its epoch (once the freeze is in force), an epoch never retreats, and an advance ships its compatibility statement. |
| GOV-1-10 | `pack-operation-contract` | `domain-pack.schema.json` requires and closes the operation contract; docs/17 §4's `OperationContract` and the schema's operation properties agree in both directions; every pack manifest in the dossier states the whole contract for every operation. |
| GOV-1-11 | `reference-precedes-optimization` | docs/01 §7.1 precedes §7.2 and still declares differential truth; §7.2 still requires evidence checked outside the search; the reference engine exists, declares itself the oracle, and has no dependency on any optimized engine; every optimized engine still states the serialization boundary. |
| GOV-1-12 | `ambiguity-is-an-error` | The constitution still states the rule; every schema closes its top level; every artifact schema `const`-pins and requires `schema_id`/`schema_epoch`; the verdict vocabulary is closed at three; an inconclusive verdict must carry its INV-008 reason, structurally in `continuum-task`. |

## Which rules are proxies, and why

`GOV-1-01` … `GOV-1-07` are code policy and are enforced directly against the
manifests and sources they talk about. Their `boundary` field is `null` for
`GOV-1-01`/`GOV-1-02` and states a real scope limit for the rest.

`GOV-1-08` … `GOV-1-12` are *process* obligations. No program reading one
revision can observe "this change was semantic", "this change was breaking", or
"this optimization was written after its reference". Each of those rules
therefore enforces a checkable core — an artifact-level invariant whose violation
would be a necessary consequence of breaking the obligation — and every one of
them carries an explicit `boundary` string in `evidence/gov-1.json` naming what
it does not see. Read those fields before quoting a rule as proof of the bullet.

For **GOV-1-08 and GOV-1-09 the proxy is no longer the whole enforcement**:
`check_revision_delta.py` reads two revisions and enforces the delta half. Both
requirements now carry a `delta_gate` field in `evidence/gov-1.json` naming the
rule that does it. See "The two-revision gate" below for what it does and does
not see.

The weakest links that remain, stated plainly:

- **GOV-1-11** has two sub-checks that match normative sentences in
  documentation. They prove a claim is still made, not that a differential
  campaign was run; that evidence belongs to docs/19.
- **GOV-1-08**'s single-revision half is crate-scoped: a change inside an
  already-cited module is invisible to it. The delta gate closes that at *file*
  granularity — a changed file must itself cite a record or land with one — but
  neither half decides whether the cited record actually governs the change.
  That is still the docs/12 §4 review obligation.
- **GOV-1-09**'s in-place-edit check is **deferred, not enforced**, while
  `schemas/README.md` carries its pre-freeze draft clause. See the freeze regime
  below: this is the convention's own rule, not a weakening of the gate.

## The two-revision gate

`check_revision_delta.py` takes a base revision (default: `git merge-base HEAD
main`, falling back to `origin/main`) and a head (default: the working tree,
untracked files included; `--head <rev>` for a revision) and enforces two rules
on the delta:

| Rule | Sub-check | A violation is |
|---|---|---|
| `semantic-change-adr-delta` (GOV-1-08) | `semantic-change-cites-a-decision-record` | a semantic-tier source file whose *code* changed, citing no `ADR-NNNN`/`RFC NNNN` at head, in a delta that adds or modifies no decision record |
| | `added-semantic-source-cites-a-decision-record` | a semantic-tier source file the delta *adds* that cites no decision record — a new module names its own record, and an unrelated ADR edit does not excuse it |
| `epoch-discipline-delta` (GOV-1-09) | `schema-bytes-changed-without-epoch-advance` | a schema document whose bytes changed with `schema_epoch` unchanged (freeze-gated) |
| | `published-schema-document-deleted` | a schema document present at base and gone at head (freeze-gated) |
| | `schema-epoch-never-retreats` | a `schema_epoch` that went down — enforced in every regime |
| | `epoch-advance-publishes-a-compatibility-statement` | an epoch advance with no markdown in the same delta naming that class together with `Preserved`/`Revalidate`/`Incompatible` (plan §4.6) — enforced in every regime |

"Semantic tier" is `check_code_policy.py`'s `SEMANTIC_CORE`, **imported** rather
than re-declared, so the delta rule inherits the partition GOV-1-03's
`tier-partition-covers-plan-20` already holds to plan §20. A change counts as
code when the comment-stripped, whitespace-normalized text differs: a doc-comment
rewrite or a `cargo fmt` pass is not a semantic change. The same inline
`// continuum:allow(<check>): <reason>` waiver the source scans honour applies
here, with the same 12-character reason floor, and every granted waiver is listed
in the run's `waivers_granted`.

**Freeze regime.** `schemas/README.md` states its own: "Until PR 5 freezes the
interface (plan §25), epoch 1 is a draft: the documents in this directory are
edited in place and no compatibility statement is owed for those edits." While
that clause is present, the two freeze-gated checks produce **deferred**
findings — printed on stderr, listed under `deferred` in the report, and not
failures. When the clause goes, they become violations. An absent or reworded
README is read as *in force*: the doubt fails closed. `--freeze
in-force|pre-freeze` states the regime explicitly.

**Degradation.** With no resolvable base — outside a work tree, no `git`, unborn
HEAD, no trunk ref, or a shallow checkout with no common ancestor — the runner
prints the reason on stderr, reports `"status": "skip"` with the resolution
trail, and says the delta rules were not enforced. It exits 0 so a fresh clone
does not fail the local gate, and exits 1 under `--require-base`, which is how
CI states that a base is owed. An explicit `--base` that does not resolve is a
hard failure rather than a skip: a wrong input must not degrade into a pass.
There is no path on which an unenforced run reports a pass. Nine resolution
scenarios — including all five skip paths and both explicit-base paths — are
driven through a stubbed `git` by `--self-test`, so the degradation path is
tested without needing a repository in any particular state.

**Fixture pairs.** `fixtures/delta/<id>/` holds a `fixture.json` with a `base`
map and a `head` map of repository paths to inert `.fixture` payloads, plus an
`expect` of `violation`, `deferred`, or `clean`. Unlike the single-revision
fixtures these pairs are *hermetic* — they do not overlay the real tree, because
a delta rule's whole input is its pair — so the self-test is independent of git
and of repository state. They are still held to the tree in the one way they can
rot: every `crates/<name>/` path a pair names must still be a crate
`check_code_policy.py` classifies, so a renamed or reclassified crate fails the
self-test loudly instead of silently testing nothing. Seven of the fifteen pairs
are *passing* pairs — a change landing with its ADR, a file that already cites
its record, a comment-only edit, a boundary-tier crate, a waived change, a new
file that cites its record, an epoch advance with its compatibility statement —
so the gate is proven not to fire on the shapes it must allow.

## Scope decisions worth arguing with

**Crate tiering.** Plan §20 lists the crates but does not partition them by
policy class, so `check_code_policy.py` does — once, in the open, with the
classification rule written above the table. `GOV-1-03`'s
`tier-partition-covers-plan-20` sub-check fails if §20 and the table disagree in
either direction, so a new crate cannot be added without being classified. The
three tiers are:

- **semantic core** (27 crates) — computes, encodes, compares, or checks a
  semantic artifact. docs/19 §7's determinism matrix applies to all of them.
  `GOV-1-03`/`04`/`05` are enforced here. The search engines are inside this tier
  deliberately: a seed-dependent frontier yields a seed-dependent counterexample.
- **boundary** (9 crates) — the effect packs, asupersync, Forge, the benchmark
  harness, the security crate, the proof-service client. Owning an ambient
  resource behind a declared capability is their job; excluding them is the
  design, not an oversight. `GOV-1-06` is what keeps them out of the checker.
- **adapters** (6 crates) — protocol surfaces owning no semantic state.

**Waivers.** `GOV-1-03`/`04`/`05`'s source scans honour an inline
`// continuum:allow(<check>): <reason>` comment covering its own line and the
next. A reason shorter than 12 characters is reported rather than honoured, and
every granted waiver is listed in `evidence/gov-1.json` under `waivers_granted`
— today that list is empty. The waiver exists so a legitimate future use is a
visible, reviewable line rather than a reason to weaken the rule.

**GOV-1-05's `Hash` prohibition** is scoped to the canonical value module
(`crates/continuum-value/src/value.rs`) alone. `continuum-value`'s epoch tokens
and assurance dimensions, and `continuum-workspace`'s path and handle types,
derive `Hash` legitimately — they are labels, not canonical states. The
prohibition is on fingerprinting a *value*, not on being a map key.

## Fixtures

56 violating fixtures under `fixtures/code/`, at least four per obligation. Each
is a directory holding a `fixture.json` and, where needed, payload files with a
`.fixture` suffix. They are inert data: nothing under `tools/` is a Cargo
workspace member or inside `notes/plan/`, so no fixture is ever compiled by
`cargo` or validated by the dossier validator. The runner reads them as text,
overlays them onto an in-memory view of the repository, runs the *same* rule
function the real check runs, and throws the view away.

A fixture expresses its violation one of two ways:

- `overlay` — replace (or create) a whole file from a payload;
- `substitute` — apply find/replace edits to the real file. The anchor **must**
  match exactly once; a fixture whose anchor has drifted fails the self-test
  loudly instead of silently testing nothing.

Fixtures also name the sub-check they must trip, so a fixture that is caught by
the wrong part of a rule is a self-test failure.

The two-revision gate's fixtures are *pairs* and live under `fixtures/delta/`;
they are described under "The two-revision gate" above, because a pair expresses
a base and a head rather than an overlay.

## Evidence

`evidence/gov-1.json` is keyed by requirement id and records, per obligation: the
rule name, what it enforces, what was scanned, the real-run result and any
violations, the fixtures proven caught in that same run's self-test, the
`boundary` string for proxy rules, and the `delta_gate` naming the two-revision
half where one exists. It also records the tier partition, the self-test summary,
and every granted waiver.

It contains no timestamp, commit id, or absolute path: it is a function of the
repository contents alone, so re-running the check on an unchanged tree rewrites
it byte-for-byte. A diff in `evidence/gov-1.json` always means the policy state
moved.

`evidence/gov-1-delta.json` is the same discipline applied to a gate whose input
is *two revisions*: it records the rules, their sub-checks, the fixture pairs
proven caught in that run's self-test, the base-resolution and degradation
behaviour with all nine scenarios, the freeze regime, and the boundary — and
deliberately **not** the real run's verdict, which is a function of history
rather than of the tree and would make a committed file change whenever history
moves. The verdict goes to stdout and to CI; this file is the standing proof
that the gate is not vacuous.

## What the delta gate finds on the current tree

Run against its default base (the merge base with `main`) from a fresh workspace
the delta is empty and the gate passes. Run against the repository's first commit
— i.e. asked "what does the whole history look like as one delta?" — it reports
two things that are worth a lead's attention and are **not** fixed here, because
this Bone adds the enforcement and does not edit the enforced:

```sh
python3 tools/governance/check_revision_delta.py --base "$(git rev-list --max-parents=0 HEAD)"
```

1. **21 semantic-core source files carry code and cite no numbered decision
   record.** They are invisible to the single-revision rule because that one is
   crate-scoped and every one of those crates has *some* file that cites a
   record:

  - `crates/continuum-certificate/src/family.rs`
  - `crates/continuum-cir/src/lib.rs`
  - `crates/continuum-cml-elab/src/lib.rs`
  - `crates/continuum-cml-syntax/src/lib.rs`
  - `crates/continuum-debugger/src/lib.rs`
  - `crates/continuum-engine-dpor/src/lib.rs`
  - `crates/continuum-engine-explicit/src/lib.rs`
  - `crates/continuum-engine-liveness/src/lib.rs`
  - `crates/continuum-engine-reference/src/diehard.rs`
  - `crates/continuum-engine-reference/src/domain.rs`
  - `crates/continuum-engine-reference/src/ident.rs`
  - `crates/continuum-engine-reference/src/lib.rs`
  - `crates/continuum-engine-reference/src/witness.rs`
  - `crates/continuum-incremental/src/lib.rs`
  - `crates/continuum-model-core/src/lib.rs`
  - `crates/continuum-observer/src/lib.rs`
  - `crates/continuum-refinement/src/lib.rs`
  - `crates/continuum-repair/src/lib.rs`
  - `crates/continuum-task/src/region/schedule.rs`
  - `crates/continuum-workspace/src/diff.rs`
  - `crates/continuum-workspace/src/seal.rs`

   Nothing in `just check` fails on them today: the gate judges *deltas*, and a
   delta that does not touch these files says nothing about them. The next
   change to any one of them will be asked for its record.

2. **15 schema documents have been edited in place since the first commit**
   without an epoch advance (`assurance-result`, `benchmark-task`, `cir`,
   `context-pack`, `corpus-port`, `crashpack`, `domain-pack`,
   `evidence-graph-node`, `intent-contract`, `proof-receipt`,
   `repair-transaction`, `semantic-diff`, `synthesis-candidate`,
   `verification-task`, `workspace-snapshot`). All fifteen are reported as
   **deferred**, which is correct and not a bug: `schemas/README.md` permits
   exactly this until PR 5 freezes the interface. The list is the blast radius
   the freeze will inherit — on the day that clause is removed, every one of
   these becomes a violation unless its epoch has moved.

Over the 40 most recent trunk commits, run commit-by-commit, the gate reports one
violation (`8b0b130`, `crates/continuum-context/src/lib.rs` changed with no cited
record and no ADR in the delta — since fixed on trunk) and one deferred finding
(`f6c3247`, an in-place edit to `context-pack.schema.json`). It is not a noisy
rule.

## Known gaps

1. **The delta gate sees two revisions, not a history.** A base and a head, not
   the commits between them: an edit made and reverted inside a branch is
   correctly invisible, and a squashed force-push is judged on its result. It
   also reads schema *documents* — a breaking change made in
   `continuumd-native-protocol.idl` or in an RFC without touching a schema file
   is outside `epoch-discipline-delta`.
2. **External dependencies are checked at the declared level.** The workspace
   declares one external dependency (`blake3`, vendored behind the ADR-0013
   hasher seam by bn-30eym and recorded in `dependency-rationale.toml`), so
   `GOV-1-07`'s external-crate handling is exercised by a real entry as well as
   by its fixtures; `GOV-1-04`/`05`/`06`'s external halves remain fixture-proven. Deep transitive auditing of third-party crates is
   PR 9's kernel covenant tooling.
3. **Out-of-tree packs.** `GOV-1-10` reaches every pack manifest in the
   repository — today one. A pack shipped out of tree is governed by the same
   schema but not by this run.
