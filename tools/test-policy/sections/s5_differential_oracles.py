"""docs/19 §5 "Differential oracles" — TEST-5-01 … TEST-5-06.

docs/19 §5's own text: "Use independently implemented systems where semantics
overlap ... Disagreement halts the relevant claim and creates a minimized
fixture." Six of its seven bullets are this bone's scope (bn-7usz); the
seventh, "external temporal/probabilistic tools for extension lanes"
(TEST-5-07), is a later module's job — see `unclaimed` in the evidence file.

# What already exists, and is reused rather than duplicated

`tools/test-policy/differential_corpus.py` (bn-1kgnz, under bn-3ly0) already
runs a real, independently-checked differential: `tsys.py`'s Python oracle
against `continuum-engine-reference::semantic`'s Rust oracle, over one shared
seeded corpus, committed as `evidence/differential_corpus.json` and consumed
by `crates/continuum-engine-reference/tests/semantic_differential.rs` (which
runs under `just check`'s existing `test` step). This module imports
`differential_corpus` and `tsys` and calls their existing functions — it does
not redefine the corpus generator, the oracle, or the reduction (README-test.md,
"Adding a section": "import tsys ... rather than define a second system
shape"). TEST-5-01 and TEST-5-02 below are bound to that shared substance.

A Python re-derivation of a Rust-side comparison is evidence about the Python
side only, never evidence that the Rust code agrees — this module's own
no-cargo constraint (README-test.md) means it cannot execute
`semantic_differential.rs` to find out. So TEST-5-01 additionally reads that
Rust file as data (`rust-differential-test-drift-detected`) and mechanically
checks that its differential test still exists, is not `#[ignore]`d, and
still reads the same committed golden this module verifies — the drift tie
between the Python port and its Rust source, rather than a prose assertion
that the tie holds.

# Why every ID here is `partial`, not `enforced`

Per README-test.md step 5 and this bone's own instruction: annotate delivered
only what is enforced in full against real substance. Checked directly in
this environment before writing a line of the checks below:

- `continuum-engine-explicit` ("the optimized explicit-state engine",
  docs/01 §7.2) and `continuum-engine-dpor` ("the partial-order reduction
  engine", same doc) are both documented PR-1/IMPL-01 scaffolds — 21 lines
  each, no landed behavior (`crates/continuum-engine-{explicit,dpor}/src/lib.rs`).
  There is no optimized evaluator and no DPOR implementation in this
  workspace.
- `tlc`, `quint`, and `apalache` are absent from `PATH` and this harness has
  no network access to fetch them.
- No `asupersync` crate is a dependency anywhere in this workspace (checked:
  no `crates/*/Cargo.toml` names it), and `continuum-asupersync`'s own
  substrate binding is a typed absence (`src/binding.rs`, PR-14 exit note:
  "open until the binding lands"). No real Lab report exists to compare
  against.
- `cargo-kani` 0.68.0 IS installed here, but no gate invokes it, and this
  section may not add one (README-test.md: a new section "never edits the
  Justfile"; `check_test_policy.py` is stdlib-only, no cargo).
- No SAT/SMT solver (`z3`, `cvc5`, or similar) is installed and there is no
  network to fetch one.

Every ID below therefore states its own absence and, where some real
substance does exist (the differential corpus; `tsys`'s own reduction; the
Continuum obligation model; Lean's kernel-checked axiom manifest), cites and
re-derives it rather than asserting a mock success. Where none exists
(TEST-5-03, half of TEST-5-05, half of TEST-5-06), the check is a
freestanding, from-scratch demonstration of the same underlying concept
(bounded exhaustive verification), explicitly not a run of the named tool —
the same "freestanding, mathematically forced demonstration" bar
`s4_mutation_testing.py` set for its own absent-substance IDs.
"""

from __future__ import annotations

import itertools
import json
from pathlib import Path
from typing import Any

import differential_corpus
import tsys

SECTION = 5
TITLE = "Differential oracles"

OBLIGATIONS = {
    "TEST-5-01": "reference model evaluator versus optimized evaluator",
    "TEST-5-02": "exhaustive enumerator versus DPOR",
    "TEST-5-03": "TLC/Quint/Apalache for model subsets",
    "TEST-5-04": "asupersync Lab reports versus Continuum obligation model",
    "TEST-5-05": "Kani/Verus/other Rust tools for local components",
    "TEST-5-06": "SAT/SMT solvers and proof checkers",
}

RULES: dict[str, str] = {
    "evaluator-disagreement-detected": "TEST-5-01",
    "rust-differential-test-drift-detected": "TEST-5-01",
    "reduction-cross-check-disagreement": "TEST-5-02",
    "model-subset-invariant-violation-detected": "TEST-5-03",
    "obligation-lab-replay-disagreement": "TEST-5-04",
    "bounded-component-property-violation-detected": "TEST-5-05",
    "axiom-manifest-violation-detected": "TEST-5-06",
    "cnf-satisfiability-mischeck-detected": "TEST-5-06",
}

ROOT = Path(__file__).resolve().parents[3]
LEAN_AXIOM_MANIFEST = ROOT / "lean/artifacts/axiom-manifest-t0-t1.json"
RUST_DIFFERENTIAL_TEST = ROOT / "crates/continuum-engine-reference/tests/semantic_differential.rs"
RUST_DIFFERENTIAL_TEST_FN = "differential_corpus_agrees_with_python_oracle"
RUST_DIFFERENTIAL_GOLDEN_REF = "evidence/differential_corpus.json"

POR_SEEDS = range(64)
OBLIGATION_SEED_SCAN = range(80)


def _finding(rule: str, subject: str, message: str) -> tsys.Finding:
    return tsys.Finding(rule, RULES[rule], subject, message)


def _malformed(rule: str, what: str) -> list[dict]:
    return [_finding(rule, "<payload>", f"payload must have exactly {what}").as_json()]


# ---------------------------------------------------------------------------
# TEST-5-01: reference model evaluator versus optimized evaluator
#
# Reuses differential_corpus.generate_common/analyze (bn-1kgnz's shared
# corpus and oracle facts) unmodified. A fixture's "claim" is an independent
# assertion about a corpus system's oracle facts; the rule recomputes the
# truth with the existing function and compares.
# ---------------------------------------------------------------------------

_CLAIM_FIELDS = ("states", "interleavings_complete", "trace_classes")


def _check_evaluator(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"seed", "claim"}):
        return _malformed("evaluator-disagreement-detected", "seed/claim")
    seed, claim = payload["seed"], payload["claim"]
    if not (isinstance(seed, int) and 0 <= seed < differential_corpus.CORPUS_SIZE):
        return [_finding("evaluator-disagreement-detected", "<payload>", f"seed must be 0..{differential_corpus.CORPUS_SIZE - 1}").as_json()]
    system = differential_corpus.generate_common(seed)
    truth = differential_corpus.analyze(system)
    diffs = []
    for key in _CLAIM_FIELDS:
        if claim.get(key) != truth[key]:
            diffs.append(f"{key}: claimed {claim.get(key)!r}, the independently recomputed reference-model-evaluator run reports {truth[key]!r}")
    claimed_dep = sorted(tuple(p) for p in claim.get("dependent", []))
    true_dep = sorted(tuple(p) for p in truth["dependent"])
    if claimed_dep != true_dep:
        diffs.append(f"dependent pair set {claimed_dep} differs from the independently recomputed true dependence relation {true_dep}")
    if claim.get("findings") != []:
        diffs.append(f"claimed findings {claim.get('findings')!r} is not empty (this corpus is clean by construction, see differential_corpus.py's module doc)")
    return [_finding("evaluator-disagreement-detected", f"diff-{seed}", d).as_json() for d in diffs]


# A Python re-derivation of the golden is evidence about the Python side only.
# It is not evidence that continuum-engine-reference's Rust oracle agrees — that
# claim is only as good as crates/continuum-engine-reference/tests/semantic_differential.rs
# actually running. This rule reads that Rust source file as data (it does not
# execute it — no cargo, README-test.md) and mechanically ties TEST-5-01's cited
# Rust evidence to three structural facts a silent drift could break: the test
# function still exists, it is not `#[ignore]`d, and it still reads the same
# committed golden this module also verifies. A payload carries the file's own
# text as `source`, so a fixture can exercise the rule without touching the real
# file.


def _is_ignored(source: str, fn_name: str) -> bool:
    lines = source.splitlines()
    fn_idx = next((i for i, line in enumerate(lines) if f"fn {fn_name}" in line), None)
    if fn_idx is None:
        return False
    i = fn_idx - 1
    while i >= 0:
        line = lines[i].strip()
        if line.startswith("#[") or line.startswith("//") or line == "":
            if "ignore" in line:
                return True
            i -= 1
            continue
        break
    return False


def _check_rust_test_drift(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"path", "source"}):
        return _malformed("rust-differential-test-drift-detected", "path/source")
    path, source = payload["path"], payload["source"]
    out = []
    if RUST_DIFFERENTIAL_TEST_FN not in source:
        out.append(
            _finding(
                "rust-differential-test-drift-detected",
                path,
                f"does not define {RUST_DIFFERENTIAL_TEST_FN}: the Rust-side differential test TEST-5-01 cites as evidence is missing or renamed",
            ).as_json()
        )
    if RUST_DIFFERENTIAL_GOLDEN_REF not in source:
        out.append(
            _finding(
                "rust-differential-test-drift-detected",
                path,
                f"does not reference {RUST_DIFFERENTIAL_GOLDEN_REF}: the Python-generated golden and its Rust reader have drifted apart",
            ).as_json()
        )
    if RUST_DIFFERENTIAL_TEST_FN in source and _is_ignored(source, RUST_DIFFERENTIAL_TEST_FN):
        out.append(
            _finding(
                "rust-differential-test-drift-detected",
                path,
                f"{RUST_DIFFERENTIAL_TEST_FN} is marked #[ignore]: the Rust-side differential test does not actually run under `cargo test`",
            ).as_json()
        )
    return out


def _run_5_01() -> tuple[list[str], dict]:
    failures: list[str] = []
    golden = differential_corpus.GOLDEN_PATH
    if not golden.exists():
        return [f"{golden} is missing; run tools/test-policy/differential_corpus.py --write"], {"corpus_size": 0}
    committed = golden.read_bytes()
    recomputed = differential_corpus.canonical_bytes()
    if committed != recomputed:
        failures.append("the differential-corpus golden is stale relative to this revision (tools/test-policy/differential_corpus.py's own comparison, re-derived here for TEST-5-01 traceability)")
    data = json.loads(committed.decode("utf-8"))
    systems = data.get("systems", [])
    if not systems:
        failures.append("the differential-corpus golden has no systems: the check would be vacuous")
    nonvacuous = sum(1 for e in systems if e.get("oracle", {}).get("states", 0) > 1)
    if not nonvacuous:
        failures.append("no corpus system explores more than one state: the evaluator-agreement check would be vacuous")
    with_dependent = sum(1 for e in systems if e.get("oracle", {}).get("dependent"))
    if not with_dependent:
        failures.append("no corpus system exhibits a genuine dependent pair: the check would be vacuous")

    if not RUST_DIFFERENTIAL_TEST.exists():
        failures.append(f"{RUST_DIFFERENTIAL_TEST} is missing: TEST-5-01's cited Rust-side differential test does not exist, so there is no evidence about the Rust code")
        rust_evidence: dict = {"rust_test_path": str(RUST_DIFFERENTIAL_TEST.relative_to(ROOT)), "rust_test_exists": False}
    else:
        rel = str(RUST_DIFFERENTIAL_TEST.relative_to(ROOT))
        source = RUST_DIFFERENTIAL_TEST.read_text(encoding="utf-8")
        hits = _check_rust_test_drift({"path": rel, "source": source})
        for h in hits:
            failures.append(f"the real Rust differential test has drifted from what TEST-5-01 cites: {h['message']}")
        rust_evidence = {
            "rust_test_path": rel,
            "rust_test_exists": True,
            "rust_test_fn": RUST_DIFFERENTIAL_TEST_FN,
            "rust_test_ignored": _is_ignored(source, RUST_DIFFERENTIAL_TEST_FN),
            "rust_test_references_golden": RUST_DIFFERENTIAL_GOLDEN_REF in source,
        }

    return failures, {
        "corpus_size": len(systems),
        "systems_with_states_gt_1": nonvacuous,
        "systems_with_dependent_pair": with_dependent,
        **rust_evidence,
    }


# ---------------------------------------------------------------------------
# TEST-5-02: exhaustive enumerator versus DPOR
#
# Reuses tsys.generate (the shared TEST-2 corpus generator) and tsys.explore
# (which already runs the exhaustive enumeration and the sleep-set reduction
# in one pass, and already records their agreement as its own "reduction-agrees"
# finding, TEST-2-04's). This module re-derives the same run and binds an
# independent "claim" about its outcome to TEST-5-02, under its own rule —
# it does not re-claim TEST-2-04's rule or ID.
# ---------------------------------------------------------------------------

def _por_truth(seed: int) -> dict:
    system = tsys.generate(seed)
    static = tsys.check_static(system)
    if static:
        raise AssertionError(f"seed {seed}: tsys.generate produced an ill-formed system: {[f.as_json() for f in static]}")
    report = tsys.explore(system)
    if report.inconclusive:
        raise AssertionError(f"seed {seed}: tsys.explore was inconclusive: {report.inconclusive}")
    return {
        "interleavings": report.interleavings,
        "trace_classes": report.trace_classes,
        "reduced_runs": report.reduced_runs,
        "reduced_classes": report.reduced_classes,
        "reduction_disagreement": any(f.rule == "reduction-agrees" for f in report.findings),
    }


def _check_por(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"seed", "claim"}):
        return _malformed("reduction-cross-check-disagreement", "seed/claim")
    seed, claim = payload["seed"], payload["claim"]
    if not isinstance(seed, int) or seed < 0:
        return [_finding("reduction-cross-check-disagreement", "<payload>", "seed must be a non-negative int").as_json()]
    truth = _por_truth(seed)
    diffs = [
        f"{key}: claimed {claim.get(key)!r}, the independently recomputed exhaustive-enumerator-vs-reduction run reports {v!r}"
        for key, v in truth.items()
        if claim.get(key) != v
    ]
    return [_finding("reduction-cross-check-disagreement", f"seed-{seed}", d).as_json() for d in diffs]


def _run_5_02() -> tuple[list[str], dict]:
    failures: list[str] = []
    reduced_something = False
    saw_obligation = False
    for seed in POR_SEEDS:
        truth = _por_truth(seed)
        hits = _check_por({"seed": seed, "claim": truth})
        if hits:
            failures.append(f"seed {seed}: self-consistent claim was itself flagged: {hits[0]['message']}")
        if truth["reduced_runs"] < truth["interleavings"]:
            reduced_something = True
        if tsys.generate(seed).get("obligations"):
            saw_obligation = True
    if not reduced_something:
        failures.append("no seed in the corpus explored fewer reduced runs than the oracle: the reduction check would be vacuous")
    if not saw_obligation:
        failures.append("no seed in the corpus carried an obligation: coverage would be vacuous")
    return failures, {"seeds": f"{POR_SEEDS.start}..{POR_SEEDS.stop - 1}", "reduced_something": reduced_something}


# ---------------------------------------------------------------------------
# TEST-5-03: TLC/Quint/Apalache for model subsets
#
# No external model checker is installed or reachable (module doc). The
# substance below is a freestanding bounded-state-machine interpreter,
# written fresh here — it does not import tsys, so it is a genuinely second,
# independent implementation of "declare a small typed model, enumerate its
# reachable states, check a declared invariant on every one of them", the
# same finite-model-checking concept an external tool would apply to a model
# subset.
# ---------------------------------------------------------------------------


def _bounded_reachable(model: dict) -> tuple[set[tuple[int, ...]], list[str]]:
    bounds: dict[str, tuple[int, int]] = {v: (d["min"], d["max"]) for v, d in model["vars"].items()}
    order = sorted(bounds)
    init = tuple(model["init"][v] for v in order)
    idx = {v: i for i, v in enumerate(order)}

    def guard_ok(t: dict, state: tuple[int, ...]) -> bool:
        return all(state[idx[c["var"]]] == c["eq"] for c in t.get("guard", []))

    def step(t: dict, state: tuple[int, ...]) -> tuple[int, ...] | None:
        values = list(state)
        for name, delta in t["delta"].items():
            values[idx[name]] += delta
        for name, i in idx.items():
            lo, hi = bounds[name]
            if not (lo <= values[i] <= hi):
                return None
        return tuple(values)

    seen = {init}
    frontier = [init]
    while frontier:
        nxt = []
        for s in frontier:
            for t in model["transitions"]:
                if guard_ok(t, s):
                    s2 = step(t, s)
                    if s2 is not None and s2 not in seen:
                        if len(seen) > 20_000:
                            raise AssertionError("bounded model exceeded its state cap; TEST-5-03's models must stay tiny")
                        seen.add(s2)
                        nxt.append(s2)
        frontier = nxt
    return seen, order


_INV_OPS = {
    "le": lambda a, b: a <= b,
    "ge": lambda a, b: a >= b,
    "eq": lambda a, b: a == b,
    "ne": lambda a, b: a != b,
}


def _check_invariant(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"model", "invariant"}):
        return _malformed("model-subset-invariant-violation-detected", "model/invariant")
    model, inv = payload["model"], payload["invariant"]
    states, order = _bounded_reachable(model)
    i = order.index(inv["var"])
    op = _INV_OPS[inv["op"]]
    bad = sorted(s for s in states if not op(s[i], inv["val"]))
    return [
        _finding(
            "model-subset-invariant-violation-detected",
            f"state={list(s)}",
            f"declared invariant {inv['var']} {inv['op']} {inv['val']} fails at reachable state {list(s)} (of {len(states)} states enumerated)",
        ).as_json()
        for s in bad
    ]


_SUBSET_MODELS = [
    {
        "name": "bounded-counter",
        "model": {
            "vars": {"c": {"min": 0, "max": 3}},
            "init": {"c": 0},
            "transitions": [{"name": "inc", "guard": [{"var": "c", "eq": v}], "delta": {"c": 1}} for v in range(3)],
        },
        "invariant": {"var": "c", "op": "le", "val": 3},
    },
    {
        "name": "two-flag-mutex",
        "model": {
            "vars": {"a": {"min": 0, "max": 1}, "b": {"min": 0, "max": 1}},
            "init": {"a": 0, "b": 0},
            "transitions": [
                {"name": "set-a", "guard": [{"var": "a", "eq": 0}, {"var": "b", "eq": 0}], "delta": {"a": 1}},
                {"name": "clear-a", "guard": [{"var": "a", "eq": 1}], "delta": {"a": -1}},
                {"name": "set-b", "guard": [{"var": "b", "eq": 0}, {"var": "a", "eq": 0}], "delta": {"b": 1}},
                {"name": "clear-b", "guard": [{"var": "b", "eq": 1}], "delta": {"b": -1}},
            ],
        },
        "invariant": {"var": "a", "op": "ne", "val": 2},
    },
]


def _run_5_03() -> tuple[list[str], dict]:
    failures: list[str] = []
    total_states = 0
    for spec in _SUBSET_MODELS:
        hits = _check_invariant({"model": spec["model"], "invariant": spec["invariant"]})
        if hits:
            failures.append(f"{spec['name']}: declared-clean model subset was flagged: {hits[0]['message']}")
        states, _ = _bounded_reachable(spec["model"])
        total_states += len(states)
    if total_states == 0:
        failures.append("no model subset explored more than zero states: the check would be vacuous")
    return failures, {"models": [m["name"] for m in _SUBSET_MODELS], "states_enumerated": total_states}


# ---------------------------------------------------------------------------
# TEST-5-04: asupersync Lab reports versus Continuum obligation model
#
# The "Continuum obligation model" side reuses tsys.compile_system/enabled/
# fire (the same primitives the TEST-2-06 obligation rules run on). The "Lab
# report" side is a fabricated linear-run shape (word + claimed final open
# set) standing in for real asupersync Lab output, which does not exist in
# this repository (module doc).
# ---------------------------------------------------------------------------


def _replay(c: tsys.Compiled, word: list[str]) -> tuple[tsys.State | None, str | None]:
    state = tsys.initial_state(c)
    by_name = {t["name"]: t for t in c.transitions}
    for name in word:
        t = by_name.get(name)
        if t is None:
            return None, f"unknown transition {name!r}"
        if not tsys.enabled(c, state, t):
            return None, f"{name!r} is not enabled in the Continuum obligation model at this point in the replay"
        nxt, findings = tsys.fire(c, state, t)
        if nxt is None:
            return None, f"replaying {name!r} produced obligation-model findings: {[f.message for f in findings]}"
        state = nxt
    return state, None


def _greedy_word(c: tsys.Compiled) -> list[str]:
    state = tsys.initial_state(c)
    word: list[str] = []
    for _ in range(10_000):
        en = [t for t in c.transitions if tsys.enabled(c, state, t)]
        if not en:
            break
        t = en[0]
        nxt, findings = tsys.fire(c, state, t)
        if nxt is None or findings:
            break
        word.append(t["name"])
        state = nxt
    return word


def _check_obligation(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"system", "lab_report"}):
        return _malformed("obligation-lab-replay-disagreement", "system/lab_report")
    system, lab = payload["system"], payload["lab_report"]
    static = tsys.check_static(system)
    if static:
        return [f.as_json() for f in static]
    c = tsys.compile_system(system)
    final, err = _replay(c, lab.get("word", []))
    if err is not None:
        return [_finding("obligation-lab-replay-disagreement", "lab_report.word", f"the Lab report's run is not a valid run of the Continuum obligation model: {err}").as_json()]
    true_open = sorted(final[1])
    claimed_open = sorted(lab.get("claimed_open", []))
    if true_open != claimed_open:
        return [
            _finding(
                "obligation-lab-replay-disagreement",
                "lab_report.claimed_open",
                f"the Lab report claims open obligations {claimed_open} at the end of its run, but replaying the same run against the Continuum obligation model finds {true_open} open",
            ).as_json()
        ]
    return []


def _run_5_04() -> tuple[list[str], dict]:
    failures: list[str] = []
    seeds_with_obligation: list[int] = []
    for seed in OBLIGATION_SEED_SCAN:
        system = tsys.generate(seed)
        if not system.get("obligations"):
            continue
        c = tsys.compile_system(system)
        word = _greedy_word(c)
        final, err = _replay(c, word)
        if err is not None:
            failures.append(f"seed {seed}: its own greedy word failed to replay: {err}")
            continue
        lab_report = {"word": word, "claimed_open": sorted(final[1])}
        hits = _check_obligation({"system": system, "lab_report": lab_report})
        if hits:
            failures.append(f"seed {seed}: a self-consistent Lab report was itself flagged: {hits[0]['message']}")
        seeds_with_obligation.append(seed)
        if len(seeds_with_obligation) >= 6:
            break
    if not seeds_with_obligation:
        failures.append("no generated system carried an obligation: the check would be vacuous")
    return failures, {"seeds_checked": seeds_with_obligation}


# ---------------------------------------------------------------------------
# TEST-5-05: Kani/Verus/other Rust tools for local components
#
# No gate can invoke the installed cargo-kani without a Justfile edit this
# section may not make (module doc). The substance below exhaustively
# enumerates every input of a tiny bounded local component — the same bar
# a Kani/Verus harness proves over the same bound by symbolic execution
# instead of enumeration.
# ---------------------------------------------------------------------------


def _local_component(op: str, bound: int, a: int, b: int) -> int:
    if op == "sat-add":
        return min(a + b, bound)
    if op == "raw-add":
        return a + b  # the bug: no saturation, so a + b can exceed bound
    raise ValueError(f"unknown component op {op!r}")


def _check_component(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"component"}):
        return _malformed("bounded-component-property-violation-detected", "component")
    spec = payload["component"]
    op, bound = spec["op"], spec["bound"]
    bad = []
    for a in range(bound + 1):
        for b in range(bound + 1):
            r = _local_component(op, bound, a, b)
            if not (0 <= r <= bound):
                bad.append((a, b, r))
    return [
        _finding(
            "bounded-component-property-violation-detected",
            f"{op}({a},{b})",
            f"{op}({a}, {b}) = {r}, outside the declared bound [0, {bound}] over all {(bound + 1) ** 2} input pairs enumerated",
        ).as_json()
        for a, b, r in bad
    ]


_COMPONENT_SPECS = [
    {"op": "sat-add", "bound": 7},
    {"op": "sat-add", "bound": 15},
]


def _run_5_05() -> tuple[list[str], dict]:
    failures: list[str] = []
    pairs_checked = 0
    for spec in _COMPONENT_SPECS:
        hits = _check_component({"component": spec})
        if hits:
            failures.append(f"{spec}: the declared-correct component was flagged: {hits[0]['message']}")
        pairs_checked += (spec["bound"] + 1) ** 2
    if pairs_checked == 0:
        failures.append("no component input pair was enumerated: the check would be vacuous")
    return failures, {"components": _COMPONENT_SPECS, "input_pairs_enumerated": pairs_checked}


# ---------------------------------------------------------------------------
# TEST-5-06: SAT/SMT solvers and proof checkers
#
# Proof-checker half: reads lean/artifacts/axiom-manifest-t0-t1.json
# read-only (this module does not own lean/ and never runs `lake build`) —
# a real, already kernel-checked artifact (ADR-0035, gated by `just lean`
# under `just check`) — and independently re-verifies its own
# axiom-emptiness claim. SAT/SMT half: no solver is installed or reachable
# (module doc), so it is a freestanding brute-force satisfiability check
# over tiny CNF instances — decidable by exhaustive enumeration at this
# size, the same core a SAT solver performs, not a run of one.
# ---------------------------------------------------------------------------


def _check_axiom_manifest(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"manifest"}):
        return _malformed("axiom-manifest-violation-detected", "manifest")
    m = payload["manifest"]
    theorems = m.get("theorems", [])
    bad = [t for t in theorems if t.get("axioms")]
    out = []
    if m.get("theoremsWithAxioms", 0) != 0:
        out.append(
            _finding(
                "axiom-manifest-violation-detected",
                "theoremsWithAxioms",
                f"the manifest's own count field is {m.get('theoremsWithAxioms')!r}, not 0",
            ).as_json()
        )
    for t in bad:
        out.append(
            _finding(
                "axiom-manifest-violation-detected",
                t.get("theorem", "<unknown>"),
                f"depends on axioms {t.get('axioms')!r}, contradicting ADR-0035's T0/T1 empty-manifest requirement",
            ).as_json()
        )
    return out


def _run_5_06_axioms() -> tuple[list[str], dict]:
    failures: list[str] = []
    if not LEAN_AXIOM_MANIFEST.exists():
        return [f"{LEAN_AXIOM_MANIFEST} is missing; run `cd lean && sh scripts/axiom-manifest.sh`"], {}
    data = json.loads(LEAN_AXIOM_MANIFEST.read_text(encoding="utf-8"))
    hits = _check_axiom_manifest({"manifest": data})
    if hits:
        failures.append(f"the committed Lean axiom manifest itself has a violation: {hits[0]['message']}")
    count = data.get("theoremCount", 0)
    if count == 0:
        failures.append("the Lean axiom manifest lists zero theorems: the proof-checker half would be vacuous")
    return failures, {"theorem_count": count, "manifest": str(LEAN_AXIOM_MANIFEST.relative_to(ROOT))}


def _brute_force_sat(num_vars: int, clauses: list[list[int]]) -> tuple[bool, tuple[bool, ...] | None]:
    for bits in itertools.product((False, True), repeat=num_vars):
        def val(lit: int, bits: tuple[bool, ...] = bits) -> bool:
            v = bits[abs(lit) - 1]
            return v if lit > 0 else not v

        if all(any(val(lit) for lit in clause) for clause in clauses):
            return True, bits
    return False, None


def _check_cnf(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"num_vars", "clauses", "claim"}):
        return _malformed("cnf-satisfiability-mischeck-detected", "num_vars/clauses/claim")
    n, clauses, claim = payload["num_vars"], payload["clauses"], payload["claim"]
    if not (isinstance(n, int) and 0 < n <= 12):
        return [_finding("cnf-satisfiability-mischeck-detected", "<payload>", "num_vars must be 1..12 (brute-force bound)").as_json()]
    sat, witness = _brute_force_sat(n, clauses)
    truth = "sat" if sat else "unsat"
    if truth != claim:
        detail = f", witness {witness}" if witness else ""
        return [_finding("cnf-satisfiability-mischeck-detected", f"vars={n}", f"claimed {claim!r}, brute-force enumeration over all {2 ** n} assignments finds {truth!r}{detail}").as_json()]
    return []


_CNF_INSTANCES = [
    {"num_vars": 3, "clauses": [[1, 2], [-1, 3], [-2, -3]], "claim": "sat"},
    {"num_vars": 3, "clauses": [[1], [-1]], "claim": "unsat"},
    {"num_vars": 4, "clauses": [[1, 2, 3, 4], [-1, -2], [-3, -4], [1, -2], [-1, 2]], "claim": "sat"},
]


def _run_5_06_sat() -> tuple[list[str], dict]:
    failures: list[str] = []
    saw_sat = False
    saw_unsat = False
    for inst in _CNF_INSTANCES:
        hits = _check_cnf(inst)
        if hits:
            failures.append(f"{inst}: its own correct claim was flagged: {hits[0]['message']}")
        if inst["claim"] == "sat":
            saw_sat = True
        else:
            saw_unsat = True
    if not (saw_sat and saw_unsat):
        failures.append("the CNF corpus does not exercise both a satisfiable and an unsatisfiable instance: the check would be vacuous")
    return failures, {"instances": len(_CNF_INSTANCES)}


def _run_5_06() -> tuple[list[str], dict]:
    fa, ea = _run_5_06_axioms()
    fs, es = _run_5_06_sat()
    return fa + fs, {"axiom_manifest": ea, "cnf": es}


# ---------------------------------------------------------------------------
# Driver contract
# ---------------------------------------------------------------------------

_KIND_CHECKERS = {
    "evaluator": _check_evaluator,
    "rust-drift": _check_rust_test_drift,
    "por": _check_por,
    "invariant": _check_invariant,
    "obligation": _check_obligation,
    "component": _check_component,
    "axioms": _check_axiom_manifest,
    "cnf": _check_cnf,
}


def check_fixture(system: Any) -> list[dict]:
    if not isinstance(system, dict) or system.get("kind") not in _KIND_CHECKERS:
        return [_finding("evaluator-disagreement-detected", "<payload>", "payload must have a known 'kind'").as_json()]
    payload = {k: v for k, v in system.items() if k != "kind"}
    return _KIND_CHECKERS[system["kind"]](payload)


_ABSENCE = {
    "TEST-5-01": (
        "continuum-engine-explicit ('the optimized explicit-state engine', docs/01 §7.2) is a documented "
        "PR-1/IMPL-01 scaffold with no landed behavior (crates/continuum-engine-explicit/src/lib.rs); there is no "
        "optimized evaluator in this workspace to compare against continuum-engine-reference (self-described as "
        "'the reference path every optimized path is measured against'). The enforced substance is the existing "
        "tsys.py-vs-continuum-engine-reference differential (bn-1kgnz: differential_corpus.py, "
        "semantic_differential.rs) — two independently implemented reference-role evaluators of the same declared "
        "model agreeing over a committed golden. A Python re-derivation of that golden is evidence about the "
        "Python side only, not about the Rust code, so this module does two more things mechanically rather than "
        "asserting them in prose: it reads crates/continuum-engine-reference/tests/semantic_differential.rs (data, "
        "not executed — no cargo, README-test.md) and checks that its "
        "differential_corpus_agrees_with_python_oracle test still exists, is not #[ignore]d, and still reads the "
        "same committed golden this module verifies (the drift tie between the Python port and its Rust source). "
        "The Rust test's own pass/fail still only runs under `just check`'s existing `test` step; this module does "
        "not and cannot re-execute it."
    ),
    "TEST-5-02": (
        "continuum-engine-dpor ('the partial-order reduction engine', docs/01 §7.2) is a documented PR-1/IMPL-01 "
        "scaffold with no landed behavior; there is no DPOR implementation in this workspace, and this Python, "
        "no-cargo harness cannot invoke continuum-engine-reference's bfs path either. The enforced substance is "
        "tsys.py's own exhaustive-enumerator-vs-sleep-set-reduction comparison (already relied on by TEST-2-04's "
        "reduction-agrees rule) — a static-independence partial-order reduction in the same family as DPOR, but "
        "not DPOR itself (DPOR computes independence dynamically at runtime; sleep sets here are driven by the "
        "system's statically declared independence). Re-derived under a fresh rule for TEST-5-02's own "
        "traceability, not a re-claim of TEST-2-04's ID or rule."
    ),
    "TEST-5-03": (
        "tlc, quint, and apalache are absent from PATH in this environment and this harness has no network access "
        "to fetch them. The enforced substance is a freestanding, independently written (no tsys import) "
        "bounded-state invariant checker over a declared finite model subset — the same core concept (bounded "
        "exhaustive enumeration, an invariant evaluated on every reachable state) an external model checker "
        "applies to a model subset — not a run of TLC, Quint, or Apalache themselves."
    ),
    "TEST-5-04": (
        "No asupersync crate is a dependency anywhere in this workspace, and continuum-asupersync's own substrate "
        "binding is a typed absence (src/binding.rs, PR-14 exit note: 'open until the binding lands'); no real "
        "Lab report exists in this repository. The enforced substance replays a fabricated Lab-report shape (a "
        "linear run plus a claimed final open-obligation set) against the Continuum obligation model's own "
        "tsys.py replay (compile_system/enabled/fire — the same substance TEST-2-06's obligation rules run on) "
        "for the same declared system. One side of the comparison is real; the Lab report is hand-authored test "
        "data, not genuine asupersync output."
    ),
    "TEST-5-05": (
        "cargo-kani 0.68.0 is present in this environment, but no gate invokes it: adding one needs a new "
        "Justfile recipe, which a section module may not add (README-test.md, 'it never edits the Justfile'), and "
        "check_test_policy.py is stdlib-only with no cargo. The enforced substance is a freestanding brute-force "
        "verifier that exhaustively enumerates every input of a tiny bounded local component — the same bound a "
        "Kani/Verus harness would prove over by symbolic execution instead of enumeration — not a run of Kani or "
        "Verus. A `just kani` recipe wiring a real #[kani::proof] harness is a follow-up, lead-owned (Justfile "
        "edit)."
    ),
    "TEST-5-06": (
        "No SAT/SMT solver (z3, cvc5, or similar) is installed and there is no network access to fetch one. The "
        "proof-checker half is real: lean/artifacts/axiom-manifest-t0-t1.json is Lean's own kernel-checked, "
        "machine-captured axiom output (ADR-0035, gated by `just lean` under `just check`); this module reads "
        "that committed artifact read-only and independently re-verifies its axiom-emptiness claim. The SAT/SMT "
        "half has no solver to invoke, so its check is a freestanding brute-force satisfiability verifier over "
        "tiny CNF instances — decidable by exhaustive enumeration at this size, the same core a SAT solver "
        "performs, not a run of Z3 or cvc5."
    ),
}


def real_run() -> dict[str, Any]:
    runners = {
        "TEST-5-01": _run_5_01,
        "TEST-5-02": _run_5_02,
        "TEST-5-03": _run_5_03,
        "TEST-5-04": _run_5_04,
        "TEST-5-05": _run_5_05,
        "TEST-5-06": _run_5_06,
    }
    results = {}
    for rid, summary in OBLIGATIONS.items():
        failures, evidence = runners[rid]()
        results[rid] = {
            "status": "partial",
            "rules": sorted(r for r, q in RULES.items() if q == rid),
            "failures": failures,
            "absence": _ABSENCE[rid],
            "evidence": evidence,
        }
    return {
        "requirements": results,
        "unclaimed": ["TEST-5-07"],
        "note": "every ID here is 'partial': see each entry's 'absence' for the specific reason, per this bone's instruction not to overclaim absent external oracles.",
    }
