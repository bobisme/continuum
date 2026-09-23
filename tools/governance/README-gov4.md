# GOV §4 onward — the shared obligation harness

`check_obligations.py` is the shared harness for executable policy obligations
from [`docs/12`](../../notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md) §4 and
[`docs/19`](../../notes/plan/docs/19_TEST_STRATEGY.md) §2–§10. Each Bone that
makes a slice of those obligations executable adds one **obligation set**. The
first set is `obligations/gov4_semantic.py`: GOV-4-01 … GOV-4-06, what a
semantic change must ship with (bn-37b1).

```sh
python3 tools/governance/check_obligations.py --self-test      # every fixture, every set
python3 tools/governance/check_obligations.py                  # tree rules + merge-base delta
python3 tools/governance/check_obligations.py --evidence       # self-test, then write evidence
python3 tools/governance/check_obligations.py --base origin/main --require-base   # CI
python3 tools/governance/check_obligations.py --set gov-4-semantic --self-test
python3 tools/governance/check_obligations.py --base <rev> --regime in-force      # blast radius
```

`just check` runs the first two lines (the `governance` recipe). CI also runs
the `--require-base` line. Exit 0 when every enforced rule holds, 1 otherwise.
Stdlib only, no network. The only subprocess is `git`.

```
tools/governance/
├── check_obligations.py                 the harness: discovery, views, fixtures, regime, evidence
├── obligations/<set>.py                 one obligation set per Bone slice
├── fixtures/obligations/<set name>/     that set's fixture pairs
├── fixtures/obligations/_harness/       the harness's own fixture pairs
├── reviews/<name>.toml                  review records, shipped by the change they review
└── evidence/<set name>.json             retained output, keyed by requirement id
```

## Extending it: add a set, touch nothing shared

A sibling Bone (GOV §4 07–21, TEST §2–§10) adds these files and edits no
existing line:

1. `obligations/<name>.py` defining `SET = ObligationSet(...)`. Import the
   contract with `from check_obligations import ObligationSet, Rule, Context,
   Finding, TREE, DELTA, …`. Files whose name starts with `_` are skipped.
2. `fixtures/obligations/<set name>/<fixture id>/fixture.json`, plus payloads.
   Every sub-check of every rule needs at least one violating fixture, and every
   requirement id needs a violating fixture of its own. The self-test enforces
   both.
3. `evidence/<set name>.json`, written by `--evidence`.
4. The `(delivered: bn-…)` record on each source bullet the set enforces in
   full, then `just traceability`. The harness check
   `delivery-agrees-with-registry` fails when a set and the registry disagree.
   A requirement enforced only in part goes in the set's `undelivered` map with
   the reason, and its bullet stays unannotated.

The Justfile, the CI workflow, this harness, and the other sets stay unchanged.
Discovery rejects two sets with one name, two sets that own one requirement id,
a rule that names an id its set does not own, and an id with no rule.

### The contract

| Field | Meaning |
|---|---|
| `name` | the set name; also the fixture directory and the evidence stem |
| `source` | the dossier section the ids come from |
| `requirements` | `{id: source bullet}`. The set owns these ids and no other set may. |
| `rules` | `Rule(name, requirements, checks, mode, enforces, boundary, fn)` |
| `evidence` | the evidence path |
| `record_keys` | `{section: (keys…)}`: the review-record keys this set reads |
| `undelivered` | `{id: reason}`: ids enforced only in part. The bullet stays unannotated. |
| `notes` | free text copied into the evidence |

A rule's `fn(ctx)` returns `Finding(rule, check, message)` values. A finding
whose rule or check is not declared is a harness contract failure.

- **`mode = TREE`**: reads `ctx.head` (a `check_code_policy.Tree`) only. It
  runs on every invocation, and its real-run result goes into the evidence file.
  Use it for "the repository has X" obligations, for example most of docs/19.
- **`mode = DELTA`**: also reads `ctx.base.read_text(path)` and `ctx.changed`.
  It runs when a base resolves (`check_revision_delta.resolve_base`, unchanged).
  Its verdict goes to stdout and CI, never into the evidence file. Use it for
  "a change of class X requires Y" obligations, for example docs/12 §4.

`Context` also gives: `semantic_changes()` (GOV-1-08's predicate),
`records_in_delta()` / `head_records()`, `workspace()`, `crate_of(path)`, and
`reaches(crate, targets)` (the transitive dependency relation). Shared helpers:
`resolve_test(ctx, "crates/<c>/(tests|src)/<f>.rs::<fn>")`, which accepts only
a non-ignored `#[test]`, plus `test_is_fresh`, `find_fn`, `markdown_list`, and
`requirement_summaries`.

### Regime

A set comes into force for branches cut **after** it landed. The harness reads
the set's own module file at the base. If the file is present, the delta rules
are enforced. If it is absent, their findings are **deferred**: printed and
reported, but not failures. No configuration is needed. `--regime
in-force|deferred` overrides this for a measurement. Tree rules have no regime.

### Review records

A change-class obligation reads a review record,
`tools/governance/reviews/<name>.toml`, that the change adds or modifies. The
harness owns the envelope:

- the record parses (`review-record-parses`);
- `[change]` names `bone` (`bn-…`), `author`, and a `summary` of at least 24
  characters (`review-record-change-table`);
- every other top-level table is a section some set declares in `record_keys`,
  and every key in it is one some set declares (`review-record-sections-closed`).
  A typo is an error, not an extension.

Sets that share a section declare their own keys in it. For example, GOV-4-07
(claim impact) adds `claim_impact` to `[semantic]` from its own file, and the
allowed keys are the union.

## GOV-4-01 … GOV-4-06: semantic changes

Source: docs/12 §4 "Semantic changes / Require:". The seventh item, claim
impact (GOV-4-07), is sibling Bone bn-2tm3's.

**Trigger.** A semantic change is GOV-1-08's delta predicate, reused unchanged:
a `src/` file of a `check_code_policy.SEMANTIC_CORE` crate whose
comment-stripped code changed, or that the delta added. A test-only edit, a
doc-comment edit, and a boundary-crate edit are not semantic changes.

**The record.** A change with a semantic change ships:

```toml
[change]
bone = "bn-xxxx"
author = "continuum-dev"
summary = "what changed, in one line"

[semantic]
covers = ["crates/continuum-engine-reference/src/"]        # paths, or dir prefixes ending in /
reference_tests = ["crates/continuum-engine-reference/tests/x.rs::pins_new_step"]
metamorphic_tests = [{ test = "crates/…/tests/x.rs::rename_is_invisible", relation = "alpha-renaming" }]
differential_tests = [
  { test = "crates/…/tests/x.rs::explicit_matches_reference", oracle = "continuum-engine-reference", subject = "continuum-engine-explicit" },
]

[semantic.schemas]
updated = []                                               # schema documents this delta changed
reason = "no artifact shape moves: the step is internal"  # owed when `updated` is empty
unaffected = { context-pack = "names the class for a log line only" }

[semantic.migration]
verdict = "Preserved"                                      # plan §4.6: Preserved | Revalidate | Incompatible
note = "continuum-engine-reference callers need no change"

[semantic.security_review]
reviewer = "continuum-security"
review = "<seal review id>"
verdict = "approved"
```

| Id | Rule | Sub-checks: a violation is… |
|---|---|---|
| all six | `semantic-review-record` (delta) | `semantic-change-has-review-record`: a semantic change that no record in the delta covers. `review-record-covers-a-semantic-change`: a `covers` entry that matches no semantic change. |
| all six | `semantic-review-list` (tree) | `semantic-review-list-registered`: docs/12 §4 no longer lists the seven items in order, or the registry's GOV-4-01…06 summaries drifted (the ids are positional). `metamorphic-relation-vocabulary-present`: docs/19 §3's relation list is gone. |
| GOV-4-01 | `reference-tests` | `-tests-named` (none named), `-test-resolves` (no such non-ignored `#[test]`), `-test-exercises-the-change` (no test lives in or depends on a changed crate), `-test-is-fresh` (no named test is new or has a changed body) |
| GOV-4-02 | `metamorphic-tests` | the four above, plus `metamorphic-relation-registered` (the `relation` is not in docs/19 §3, which is read live) and `metamorphic-relation-stated-at-the-test` (the test's doc comment and body never name it) |
| GOV-4-03 | `differential-tests` | named, resolves, fresh, plus `differential-pair-distinct-implementations` (oracle and subject are the same crate or not workspace crates), `differential-test-uses-both-implementations` (the test file never names one as a Rust path), `differential-test-exercises-the-change` (neither side is or reaches a changed crate) |
| GOV-4-04 | `updated-schemas` | `schema-impact-declared` (no `updated` list, or an empty one with no reason), `declared-schema-update-in-delta` (a listed document did not change), `schema-change-declared` (a changed document is not listed), `schema-bearing-source-addressed` (a changed source names a schema class URI that is neither updated nor declared unaffected with a reason) |
| GOV-4-05 | `migration-note` | `migration-note-present`, `migration-verdict-closed` (not a plan §4.6 verdict), `migration-note-names-the-change` (under 40 characters, or it names no changed crate), `incompatible-verdict-advances-an-epoch` (`Incompatible` with no `schema_epoch` advance in the delta, GOV-1-09) |
| GOV-4-06 | `security-review` | `security-review-present`, `security-review-independent` (reviewer is the change author), `security-review-approved` (verdict is not `approved`) |

### Boundaries

Each rule's `boundary` field in the evidence file states what the rule does
not see. The main limits:

- Tests: the check proves that the named test exists, runs under `just check`,
  sits in or depends on the changed crate, and moved with the change. It does
  not judge whether the assertions pin the new behaviour. A metamorphic test
  names its relation, but the check does not prove that the test checks it.
- Differential tests: only in-workspace pairs are recognized. docs/19 §5's
  external oracles (TLC, Kani, solvers) wait for a harness that runs them.
- Schemas: a class is recognized only when the source names its URI. A shape
  change through an unnamed type is caught only by review.
- Migration: the verdict is typed and consistent with the epoch. Whether it is
  the correct verdict is a review judgement.

### GOV-4-06 is not delivered

The security-review rule is enforced, but the record is a pointer and not the
review. Seal keeps review state outside the repository (`.seal/` holds only a
version file), so no committed artifact lets a checker confirm that the named
review exists, covers this change, and approved it. The set lists GOV-4-06 in
`undelivered`, its docs/12 bullet has no `(delivered: …)` record, and
`delivery-agrees-with-registry` keeps it that way. It becomes deliverable when
review verdicts are committed or attested where a checker can read them.

## What it finds on real history

On a fresh workspace the delta has no semantic change, and the run passes. On
any base cut before this set landed, findings are deferred. Asked to enforce
anyway, over the history since the Evidence Graph commit, it reported 63
uncovered semantic changes on 2026-09-22:

```sh
python3 tools/governance/check_obligations.py --base 5179094~1 --regime in-force
```

That is the expected result. No change before this set shipped a review record,
and the deferral regime is the reason those changes do not fail today.

## Operational impact

From the day this set lands, a branch cut from trunk that changes semantic-core
code must ship a review record. Tests, schema, and migration entries can be
satisfied inside the change. The security-review entry needs an approved
review by someone other than the author, so it must come last, after the Seal
review. Branches already in flight are deferred until they are re-cut or
synced onto a trunk that contains the set.
