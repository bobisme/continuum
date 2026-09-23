# docs/19 test policy, made executable

`check_test_policy.py` is the CI check for the test-policy requirements of
[`notes/plan/docs/19_TEST_STRATEGY.md`](../../notes/plan/docs/19_TEST_STRATEGY.md).
Each top-level bullet of docs/19 section N is the requirement `TEST-N-<ordinal>`
in `notes/plan/notes/PLAN_REQUIREMENTS.json` (category `test-policy`).

```sh
python3 tools/test-policy/check_test_policy.py --self-test     # fixtures only
python3 tools/test-policy/check_test_policy.py                 # fixtures, real run, evidence freshness
python3 tools/test-policy/check_test_policy.py --evidence      # rewrite evidence/, then commit it
python3 tools/test-policy/check_test_policy.py --section 2     # one section
python3 tools/test-policy/check_test_policy.py --where TEST-2-04
```

`just test-policy` runs the first two. `just check` includes it. Exit 0 when
every check holds, 1 otherwise. Python 3 stdlib only, no network, no cargo.

```
tools/test-policy/
├── check_test_policy.py          the driver: discovery, binding, fixtures, evidence, delivered link
├── tsys.py                       finite transition systems: shape, rules, oracle, reduction, generator
├── sections/s<N>_<slug>.py       one module per docs/19 section (or part of a section)
├── fixtures/s<N>_<slug>/*.json   violating fixtures and one negative control, per module
└── evidence/s<N>_<slug>.json     retained output, keyed by requirement ID
```

## What the driver enforces for every section

| Check | Fails when |
|---|---|
| binding | a claimed ID is not in the registry, its summary differs from the registry, it is claimed by two modules, or a rule serves an ID the module does not claim |
| violating fixtures | a fixture is not caught by the rule it names, names a rule of a different ID, a claimed ID has no caught fixture, a rule has no caught fixture, or the module has no `"expect": "clean"` control or the control produces a finding |
| positive enforcement | the module's real run reports a failure for a claimed ID |
| retained output | `evidence/<module>.json` differs from what this revision produces (it is deterministic, so a stale file is a failure, not noise) |
| delivered link | the registry marks an ID `satisfied` (a `(delivered: …)` annotation on its docs/19 bullet) but no module claims it, or its status is not `enforced`, or it has failures |

The link from a requirement ID to its evidence is therefore mechanical in both
directions: `--where <ID>` names the file, the evidence is keyed by the ID, and
a delivered annotation without passing evidence fails the gate.

## Adding a section (the extension point)

A later section is a new file. It never edits a line another section owns, and
it never edits the `Justfile`.

1. Create `sections/s<N>_<slug>.py`. The driver discovers `s[0-9]*_*.py` in
   sorted order. More than one module per section is allowed (for example
   `s2_cancellation_fairness.py` for TEST-2-07/08); IDs may not overlap.
2. Define the module contract:

   | Name | Meaning |
   |---|---|
   | `SECTION: int` | the docs/19 section number; must match the file name prefix |
   | `TITLE: str` | the section title |
   | `OBLIGATIONS: dict[str, str]` | claimed ID → registry summary, exactly as `PLAN_REQUIREMENTS.json` spells it |
   | `RULES: dict[str, str]` | rule name → the claimed ID it serves |
   | `check_fixture(system) -> list[dict]` | run every rule on one fixture payload; each finding carries at least `rule` |
   | `real_run() -> dict` | `{"requirements": {ID: {"status": "enforced" \| "partial", "failures": [...], ...}}, ...}`; any other top-level keys are copied into the evidence file |

3. Add `fixtures/s<N>_<slug>/*.json`. A violating fixture is
   `{"requirement": ID, "rule": rule, "description": "...", "system": <payload>}`;
   the control is `{"expect": "clean", "description": "...", "system": <payload>}`.
   The payload shape is the module's choice.
4. Run `--evidence`, commit the evidence file, and run the check without it.
5. Only when an ID's status is `enforced` with no failures, annotate its docs/19
   bullet `(delivered: bn-… — tools/test-policy/evidence/<module>.json)` and run
   `just traceability`. When the substance has not arrived, report `partial`
   with the absence stated in the entry, and do not annotate.

A §3 metamorphic relation or a §4 mutant that needs systems should import
`tsys` (the driver puts this directory on `sys.path`) rather than define a
second system shape. Extending `tsys` is shared ground: add functions, do not
change the meaning of existing fields.

## Section 2: generated transition systems (TEST-2-01 … TEST-2-06)

Module `sections/s2_generated_systems.py`, evidence
[`evidence/s2_generated_systems.json`](evidence/s2_generated_systems.json).

docs/19 §2: "A small generator creates finite systems with [the features] …
For small sizes, enumerate all interleavings and configurations. Compare every
optimized engine and reduction against this oracle."

`tsys.generate(seed)` is that generator. Its PRNG is an explicit SplitMix64 on
the seed (INV-005), so a corpus is a function of its seed range; the evidence
records the range (0..255) and the SHA-256 of the canonical corpus. The oracle
(`tsys.explore`) visits every reachable state, enumerates every maximal
interleaving and every Mazurkiewicz trace class, and evaluates the dynamic
rules on every reachable state. A cyclic state graph or an exceeded bound is a
typed `Inconclusive`, never a pass (INV-008). The reduction under test is a
sleep-set exploration driven by the declared independence relation; its
terminal states and trace classes must equal the oracle's.

| ID | Feature | Rules |
|---|---|---|
| TEST-2-01 | typed state variables | `type-declared`, `init-typed`, `expr-typed`, `value-in-type` |
| TEST-2-02 | guarded transitions | `guard-declared`, `guard-boolean`, `transition-wellformed` |
| TEST-2-03 | explicit read/write footprints | `footprint-declared`, `footprint-covers-reads`, `footprint-covers-writes` |
| TEST-2-04 | independent/dependent pairs | `independence-wellformed`, `independence-static`, `independence-diamond`, `reduction-agrees` |
| TEST-2-05 | conflicts | `conflict-wellformed`, `conflict-not-independent`, `conflict-complete` |
| TEST-2-06 | obligations | `obligation-declared`, `obligation-linear`, `obligation-leak` |

Each ID has three layers of enforcement:

1. **Corpus.** All 256 generated systems pass every rule with no inconclusive
   outcome, and the corpus exhibits each feature: all three variable kinds, a
   guard that blocks a ready process, shared reads and writes, co-enabled
   independent and dependent pairs, a reachable conflict, a reachable acquire.
   A zero count fails as a vacuous pass. The reduction must also explore fewer
   runs than the oracle.
2. **Mutation.** Every system is mutated per rule family (out-of-type initial
   value, unbounded increment, removed or non-boolean guard, dropped read or
   write, dependent pair declared independent, dropped or independent-declared
   conflict, skipped discharge, double acquire). Static mutants must always be
   detected. Mutants whose detection depends on reachability have their
   expected outcome computed from the unmutated oracle run, and the result
   must match in both directions. A reduction disagreement without a diamond
   violation fails.
3. **Fixtures.** Twenty violating fixtures, at least one per rule, and the
   `clean` control.

Independence has two declared forms (docs/02 §3 "proven statically …
checked dynamically"): `[a, b]` is justified by disjoint footprints and checked
statically; `{"pair": [a, b], "justification": "dynamic"}` claims commutation
despite overlapping footprints and only the oracle's diamond can check it.

### Boundaries

Recorded per ID in the evidence file:

- **Engines.** The reduction compared against the oracle is the harness's own
  sleep-set exploration. `continuum-engine-explicit` and `continuum-engine-dpor`
  are documented stubs, so docs/19's "compare every optimized engine" has no
  engine to compare yet. The corpus is canonical JSON so an engine can consume
  the same bytes when it lands.
- **Types.** bool, bounded int, and enum only.
- **Footprints.** Variables and obligation identities. Pack-typed resources
  (message, timer, lifecycle; docs/02 §4) are not modeled.
- **Independence.** One relation per system, not the view- and
  property-indexed `Indep(e,f | View, PropertyClass)` of docs/02 §4.
- **Conflicts.** Transition-level disabling in a reachable state, not
  hereditary event-structure conflict over CIR events.
- **Obligations.** One owner process each. Transfer to a finalizer
  (docs/02 §7) is not modeled.
- TEST-2-07 (cancellation phases) and TEST-2-08 (fairness annotations) are not
  claimed by this module.

## Section 2: cancellation phases and fairness annotations (TEST-2-07 … TEST-2-08)

Module `sections/s2_cancellation_fairness.py`, evidence
[`evidence/s2_cancellation_fairness.json`](evidence/s2_cancellation_fairness.json).

docs/19 §2 also asks the generator to produce "cancellation phases" and
"fairness annotations", left unclaimed by `s2_generated_systems.py` above.
This module claims both without editing a line of that module, `tsys.py`, or
the `Justfile`. Rather than widen `tsys`'s system shape, a payload here is
`{"base": <tsys system>, "cancel": {...}, "fairness": [...]}`: `base` is
checked by `tsys.check` unmodified, and the two extension fields are checked
by functions that reuse `tsys.compile_system`, `tsys.enabled`, `tsys.fire`,
`tsys.explore`, `tsys.Finding`, and `tsys.SplitMix64`.

`cancel["<process>"]` declares one `request` transition and `drain`/
`finalize` transition lists (docs/02 §7 "Cancellation calculus":
`Active -> request -> Cancelling -> drain* -> finalize* -> (obligations == ∅)
-> Cancelled`). `fairness` names weakly-fair transitions. Corpus, mutation,
and fixtures layer the same way as the sibling module's methodology:

| ID | Feature | Rules |
|---|---|---|
| TEST-2-07 | cancellation phases | `cancel-declared`, `cancel-phase-order`, `cancel-finalize-discharges`, `cancel-cancelled-clears-obligations` |
| TEST-2-08 | fairness annotations | `fairness-declared`, `fairness-witnessed-choice`, `fairness-not-stuck` |

### Boundaries

- **TEST-2-07.** No separate effect-protocol state (Idle/Reserved/Committed/
  Aborted, docs/02 §7) and no finalizer ownership transfer.
- **TEST-2-08.** Only weak fairness, and only within one finite maximal run: a
  declared-fair transition must be co-enabled with another transition
  somewhere reachable, and must never be guard-enabled at a state with no
  successor (the terminating-run analogue of weak fairness under stutter
  closure). **Strong fairness is not covered.** It needs infinitely-often
  reasoning over an infinite or cyclic run, and this harness's oracle only
  enumerates interleavings on an acyclic state graph (a cyclic one is a typed
  `Inconclusive`, never a pass). No fixture or corpus system here can
  demonstrate a strong-fairness violation.
