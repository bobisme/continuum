# GOV §1 — code and semantic policy

Executable enforcement of the twelve obligations in
[`notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md`](../../notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md)
§1 "Repository constitution", carrying the requirement ids `GOV-1-01` … `GOV-1-12`
from `notes/plan/notes/PLAN_REQUIREMENTS.json`.

```
tools/governance/
├── check_code_policy.py          the twelve rules; stdlib only
├── dependency-rationale.toml     GOV-1-07's rationale and TCB classification manifest
├── fixtures/code/<id>/           56 violating fixtures — inert data
└── evidence/gov-1.json           retained output, keyed by requirement id
```

## Running it

```sh
python3 tools/governance/check_code_policy.py --self-test   # every fixture must be caught
python3 tools/governance/check_code_policy.py               # the real check
python3 tools/governance/check_code_policy.py --evidence tools/governance/evidence/gov-1.json
```

Exit 0 when every rule holds, 1 otherwise. `--evidence` runs the self-test *and*
the real check, then writes both into the evidence file — so a committed evidence
file cannot claim a fixture was caught unless it was.

Not yet wired into `just check`; the intended recipe mirrors `boundaries`:

```
policy:
    python3 tools/governance/check_code_policy.py --self-test
    python3 tools/governance/check_code_policy.py
```

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
| GOV-1-08 | `semantic-change-adr` | ADR files and `adr/README.md` are a bijection; every ADR declares a docs/12 §2 status; every `ADR-NNNN`/`RFC NNNN` citation in crate sources resolves; every semantic-core crate carrying code cites a numbered decision record. |
| GOV-1-09 | `epoch-discipline` | The schemas README still states the `MUST advance schema_epoch` rule; every schema's `$id` epoch equals its `schema_epoch`; every instance names an epoch its schema is; the six epoch kinds in code and in prose are one set; `EpochAdvance::new` requires a `Compatibility` verdict. |
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

The weakest links, stated plainly:

- **GOV-1-09** cannot see a breaking schema edit made without touching `$id` or
  `schema_epoch`. It enforces that the epoch machinery is coherent and cannot be
  bypassed, not that it was used.
- **GOV-1-11** has two sub-checks that match normative sentences in
  documentation. They prove a claim is still made, not that a differential
  campaign was run; that evidence belongs to docs/19.
- **GOV-1-08**'s "semantic code cites a decision record" is crate-scoped. A
  semantic change made inside an already-cited module without amending its ADR
  is not visible here and stays a docs/12 §4 review obligation.

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

## Evidence

`evidence/gov-1.json` is keyed by requirement id and records, per obligation: the
rule name, what it enforces, what was scanned, the real-run result and any
violations, the fixtures proven caught in that same run's self-test, and the
`boundary` string for proxy rules. It also records the tier partition, the
self-test summary, and every granted waiver.

It contains no timestamp, commit id, or absolute path: it is a function of the
repository contents alone, so re-running the check on an unchanged tree rewrites
it byte-for-byte. A diff in `evidence/gov-1.json` always means the policy state
moved.

## Known gaps

1. **No revision-diff gate.** `GOV-1-09` (and the "was this change semantic?"
   half of `GOV-1-08`) would be genuinely enforceable in CI by comparing the
   working tree against the merge base — a schema whose bytes changed without its
   `schema_epoch` advancing, past the PR 5 freeze, is a mechanical violation. That
   needs a two-revision runner and belongs with the CI wiring, not here.
2. **External dependencies are checked at the declared level.** The workspace
   declares one external dependency (`blake3`, vendored behind the ADR-0013
   hasher seam by bn-30eym and recorded in `dependency-rationale.toml`), so
   `GOV-1-07`'s external-crate handling is exercised by a real entry as well as
   by its fixtures; `GOV-1-04`/`05`/`06`'s external halves remain fixture-proven. Deep transitive auditing of third-party crates is
   PR 9's kernel covenant tooling.
3. **Out-of-tree packs.** `GOV-1-10` reaches every pack manifest in the
   repository — today one. A pack shipped out of tree is governed by the same
   schema but not by this run.
