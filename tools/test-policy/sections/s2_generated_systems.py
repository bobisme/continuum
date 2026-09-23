"""docs/19 §2 "Generated transition systems" — TEST-2-01 … TEST-2-06.

The section text: "A small generator creates finite systems with: typed state
variables; guarded transitions; explicit read/write footprints;
independent/dependent pairs; conflicts; obligations; [...] For small sizes,
enumerate all interleavings and configurations. Compare every optimized engine
and reduction against this oracle."

This module claims the first six bullets. TEST-2-07 (cancellation phases) and
TEST-2-08 (fairness annotations) are left to a later section module.

Enforcement has three layers, and each obligation has all three:

1. Corpus. `tsys.generate` over SEEDS produces the systems. Every system must
   pass every static rule and every oracle rule, with no inconclusive outcome,
   and the corpus as a whole must exhibit each feature (a feature the corpus
   never exercises is a vacuous pass, so it fails).
2. Mutation. Each generated system is mutated once per rule family, and the
   rule must detect the mutant. Where detection depends on reachability, the
   expected outcome is computed from the unmutated oracle run and must match
   exactly, in both directions.
3. Fixtures. Hand-written violating systems under `fixtures/s2_generated_systems/`,
   run by the driver's self-test.
"""

from __future__ import annotations

import copy
import hashlib
from typing import Any

import tsys

SECTION = 2
TITLE = "Generated transition systems"
OBLIGATIONS = {
    "TEST-2-01": "typed state variables",
    "TEST-2-02": "guarded transitions",
    "TEST-2-03": "explicit read/write footprints",
    "TEST-2-04": "independent/dependent pairs",
    "TEST-2-05": "conflicts",
    "TEST-2-06": "obligations",
}
RULES = tsys.RULES
SEEDS = range(256)

BOUNDARIES = {
    "TEST-2-01": [
        "Variable types are bool, bounded int, and enum. Records, sets, maps, and sequences are not generated.",
    ],
    "TEST-2-02": [
        "A guard is one boolean expression over declared variables. Guards over obligation state are not generated.",
    ],
    "TEST-2-03": [
        "Footprint resources are state variables and obligation identities. Pack-typed resources (message, timer, lifecycle; docs/02 §4) are not modeled.",
    ],
    "TEST-2-04": [
        "Independence is one relation per system. The view/property-indexed relation Indep(e,f | View, PropertyClass) of docs/02 §4 is not modeled.",
        "The reduction compared against the oracle is this harness's sleep-set exploration. continuum-engine-explicit and continuum-engine-dpor are documented stubs, so no optimized engine is compared yet.",
    ],
    "TEST-2-05": [
        "A conflict is transition-level disabling in a reachable state. Hereditary event-structure conflict over CIR events (docs/02 §2) is not modeled.",
    ],
    "TEST-2-06": [
        "An obligation has one owner process and is acquired and discharged by it. Ownership transfer to a finalizer (docs/02 §7) is not modeled.",
    ],
}


def check_fixture(system: Any) -> list[dict[str, str]]:
    findings, _report = tsys.check(system)
    return [f.as_json() for f in findings]


# ---------------------------------------------------------------------------
# Mutants
# ---------------------------------------------------------------------------


def _conjuncts(guard: dict) -> list[dict]:
    if guard.get("op") == "and":
        return list(guard["args"])
    return [guard]


def _join(conds: list[dict]) -> dict:
    return conds[0] if len(conds) == 1 else {"op": "and", "args": conds}


def _rules(findings: list[tsys.Finding]) -> set[str]:
    return {f.rule for f in findings}


class Tally:
    def __init__(self) -> None:
        self.applied: dict[str, int] = {}
        self.detected: dict[str, int] = {}
        self.failures: list[str] = []

    def record(self, name: str, detected: bool, expected: bool, subject: str) -> None:
        self.applied[name] = self.applied.get(name, 0) + 1
        if detected:
            self.detected[name] = self.detected.get(name, 0) + 1
        if detected != expected:
            want = "detected" if expected else "not detected"
            self.failures.append(f"mutant {name} on {subject}: expected {want}, got the opposite")

    def as_json(self, name: str) -> dict[str, int]:
        return {"applied": self.applied.get(name, 0), "detected": self.detected.get(name, 0)}


def _mutate(system: dict, report: tsys.OracleReport, tally: Tally) -> None:
    name = system["name"]
    transitions = system["transitions"]

    # TEST-2-01: an initial value outside its type is always a static finding.
    m = copy.deepcopy(system)
    var = m["variables"][0]
    decl = var["type"]
    m["init"][var["name"]] = {"bool": 7, "int": decl.get("max", 0) + 1, "enum": "zz"}[decl["kind"]]
    tally.record("init-out-of-type", "init-typed" in _rules(tsys.check_static(m)), True, name)

    # TEST-2-01: drop the bound conjunct that keeps an increment in range. The
    # oracle must report value-in-type exactly when the bound is reachable at
    # the maximum, which this mutant cannot know statically; it is recorded,
    # and the corpus must show at least one detection.
    for i, t in enumerate(transitions):
        for target, expr in t["updates"].items():
            if expr.get("op") != "add" or target.startswith("pc_"):
                continue
            conds = _conjuncts(t["guard"])
            bound = {"op": "lt", "args": [{"var": target}, {"const": _max(system, target)}]}
            if bound not in conds:
                continue
            m = copy.deepcopy(system)
            m["transitions"][i]["guard"] = _join([c for c in conds if c != bound])
            findings, _ = tsys.check(m)
            detected = "value-in-type" in _rules(findings)
            tally.applied["unbounded-increment"] = tally.applied.get("unbounded-increment", 0) + 1
            if detected:
                tally.detected["unbounded-increment"] = tally.detected.get("unbounded-increment", 0) + 1

    t0 = transitions[0]
    # TEST-2-02: remove the guard; make the guard an int.
    m = copy.deepcopy(system)
    del m["transitions"][0]["guard"]
    tally.record("guard-removed", "guard-declared" in _rules(tsys.check_static(m)), True, f"{name}/{t0['name']}")
    m = copy.deepcopy(system)
    m["transitions"][0]["guard"] = {"op": "add", "args": [{"var": t0["reads"][0]}, {"const": 1}]} if _int_read(system, t0) else {"const": 3}
    tally.record("guard-not-boolean", _rules(tsys.check_static(m)) & {"guard-boolean", "expr-typed"} != set(), True, f"{name}/{t0['name']}")

    # TEST-2-03: drop a declared write; drop a declared read.
    for i, t in enumerate(transitions):
        m = copy.deepcopy(system)
        dropped = m["transitions"][i]["writes"].pop(0)
        tally.record("write-undeclared", "footprint-covers-writes" in _rules(tsys.check_static(m)), True, f"{name}/{t['name']}/{dropped}")
        m = copy.deepcopy(system)
        dropped = m["transitions"][i]["reads"].pop(0)
        tally.record("read-undeclared", "footprint-covers-reads" in _rules(tsys.check_static(m)), True, f"{name}/{t['name']}/{dropped}")

    # TEST-2-04: declare a co-enabled dependent pair independent (docs/19 §4
    # "declare conflicting events independent"). The static rule always fires;
    # the oracle's diamond check and the reduction comparison run on the
    # mutant with the static rules bypassed, as they would for an independence
    # claim justified by something other than footprints.
    for a, b in sorted(report.coenabled_dependent):
        m = copy.deepcopy(system)
        m["independent"].append([a, b])
        m["conflicts"] = [p for p in m["conflicts"] if tsys.pair_key(*p) != (a, b)]
        tally.record("dependent-declared-independent", "independence-static" in _rules(tsys.check_static(m)), True, f"{name}/{a}|{b}")
        oracle = tsys.explore(m)
        rules = _rules(oracle.findings)
        diamond = "independence-diamond" in rules
        for key in ("diamond-on-mutant", "reduction-disagrees-on-mutant"):
            tally.applied[key] = tally.applied.get(key, 0) + 1
        if diamond:
            tally.detected["diamond-on-mutant"] = tally.detected.get("diamond-on-mutant", 0) + 1
        if "reduction-agrees" in rules:
            tally.detected["reduction-disagrees-on-mutant"] = tally.detected.get("reduction-disagrees-on-mutant", 0) + 1
            if not diamond:
                tally.failures.append(f"{name}/{a}|{b}: the reduction disagreed with the oracle but no diamond violation was reported")

    # TEST-2-05: drop a witnessed conflict (must be reported), declare a
    # declared conflict independent (always a static finding).
    for a, b in sorted(report.witnessed_conflicts):
        m = copy.deepcopy(system)
        m["conflicts"] = [p for p in m["conflicts"] if tsys.pair_key(*p) != (a, b)]
        tally.record("conflict-undeclared", "conflict-complete" in _rules(tsys.explore(m).findings), True, f"{name}/{a}|{b}")
    for a, b in system["conflicts"][:1]:
        m = copy.deepcopy(system)
        m["independent"].append([a, b])
        tally.record("conflict-declared-independent", "conflict-not-independent" in _rules(tsys.check_static(m)), True, f"{name}/{a}|{b}")

    # TEST-2-06: skip the discharge (docs/19 §4 "skip obligation discharge"):
    # a static finding always, and an oracle leak exactly when the acquire is
    # reachable. Acquire at the discharge step instead: a linearity violation
    # exactly when the discharge step is reachable.
    for obl in system["obligations"]:
        o = obl["name"]
        acq = next(t["name"] for t in transitions if o in t.get("acquire", []))
        dis_i = next(i for i, t in enumerate(transitions) if o in t.get("discharge", []))
        dis = transitions[dis_i]["name"]
        m = copy.deepcopy(system)
        del m["transitions"][dis_i]["discharge"]
        tally.record("discharge-skipped-static", "obligation-declared" in _rules(tsys.check_static(m)), True, f"{name}/{o}")
        tally.record("discharge-skipped-leak", "obligation-leak" in _rules(tsys.explore(m).findings), acq in report.fired, f"{name}/{o}")
        m = copy.deepcopy(system)
        m["transitions"][dis_i]["acquire"] = m["transitions"][dis_i].pop("discharge")
        tally.record("double-acquire", "obligation-linear" in _rules(tsys.explore(m).findings), dis in report.fired, f"{name}/{o}")


def _max(system: dict, var: str) -> int:
    return next(v["type"]["max"] for v in system["variables"] if v["name"] == var)


def _int_read(system: dict, t: dict) -> bool:
    kinds = {v["name"]: v["type"]["kind"] for v in system["variables"]}
    return bool(t["reads"]) and kinds.get(t["reads"][0]) == "int"


# ---------------------------------------------------------------------------
# The real run
# ---------------------------------------------------------------------------


def real_run() -> dict[str, Any]:
    systems = [tsys.generate(seed) for seed in SEEDS]
    digest = hashlib.sha256("\n".join(tsys.canonical(s) for s in systems).encode()).hexdigest()
    failures: dict[str, list[str]] = {rid: [] for rid in OBLIGATIONS}
    tally = Tally()
    agg: dict[str, int] = {
        "systems": len(systems),
        "states": 0,
        "interleavings": 0,
        "trace_classes": 0,
        "reduced_runs": 0,
        "variables_bool": 0,
        "variables_int": 0,
        "variables_enum": 0,
        "transitions": 0,
        "guards_blocking_a_ready_process": 0,
        "footprint_shared_reads": 0,
        "footprint_shared_writes": 0,
        "coenabled_independent_pairs": 0,
        "coenabled_dependent_pairs": 0,
        "declared_conflicts": 0,
        "witnessed_conflicts": 0,
        "obligations": 0,
        "obligations_acquired_in_some_run": 0,
    }
    for system in systems:
        findings, report = tsys.check(system)
        for f in findings:
            failures[f.requirement].append(f"{system['name']}: {f.rule} {f.subject}: {f.message}")
        if report is None:
            continue
        for inc in report.inconclusive:
            for rid in OBLIGATIONS:
                failures[rid].append(f"{system['name']}: inconclusive {inc.reason}: {inc.detail}")
        agg["states"] += report.states
        agg["interleavings"] += report.interleavings
        agg["trace_classes"] += report.trace_classes
        agg["reduced_runs"] += report.reduced_runs
        for v in system["variables"]:
            if not v["name"].startswith("pc_"):
                agg[f"variables_{v['type']['kind']}"] += 1
        agg["transitions"] += len(system["transitions"])
        agg["guards_blocking_a_ready_process"] += _blocking_guards(system, report)
        for t in system["transitions"]:
            shared_r = [r for r in t["reads"] if not r.startswith(("pc_", "obligation:"))]
            shared_w = [w for w in t["writes"] if not w.startswith(("pc_", "obligation:"))]
            agg["footprint_shared_reads"] += bool(shared_r)
            agg["footprint_shared_writes"] += bool(shared_w)
        agg["coenabled_independent_pairs"] += len(report.coenabled_independent)
        agg["coenabled_dependent_pairs"] += len(report.coenabled_dependent)
        agg["declared_conflicts"] += len(system["conflicts"])
        agg["witnessed_conflicts"] += len(report.witnessed_conflicts)
        agg["obligations"] += len(system["obligations"])
        agg["obligations_acquired_in_some_run"] += sum(
            1 for t in system["transitions"] if t.get("acquire") and t["name"] in report.fired
        )
        _mutate(system, report, tally)

    for msg in tally.failures:
        rid = _mutant_requirement(msg)
        failures[rid].append(msg)

    def need(rid: str, key: str, what: str) -> None:
        if agg[key] == 0:
            failures[rid].append(f"corpus exercises no {what} ({key} = 0): the pass would be vacuous")

    def need_detected(rid: str, mutant: str) -> None:
        if tally.detected.get(mutant, 0) == 0:
            failures[rid].append(f"mutant {mutant} was never detected across the corpus")

    for kind in ("bool", "int", "enum"):
        need("TEST-2-01", f"variables_{kind}", f"{kind} state variable")
    need_detected("TEST-2-01", "unbounded-increment")
    need("TEST-2-02", "guards_blocking_a_ready_process", "guard that blocks a ready process")
    need("TEST-2-03", "footprint_shared_reads", "shared-variable read")
    need("TEST-2-03", "footprint_shared_writes", "shared-variable write")
    need("TEST-2-04", "coenabled_independent_pairs", "co-enabled independent pair")
    need("TEST-2-04", "coenabled_dependent_pairs", "co-enabled dependent pair")
    need_detected("TEST-2-04", "diamond-on-mutant")
    need_detected("TEST-2-04", "reduction-disagrees-on-mutant")
    if agg["reduced_runs"] >= agg["interleavings"]:
        failures["TEST-2-04"].append("the sleep-set reduction explored no fewer runs than the oracle: nothing was reduced")
    need("TEST-2-05", "witnessed_conflicts", "reachable conflict")
    need("TEST-2-06", "obligations_acquired_in_some_run", "reachable obligation acquire")
    need_detected("TEST-2-06", "discharge-skipped-leak")
    need_detected("TEST-2-06", "double-acquire")

    mutants = {
        "TEST-2-01": ["init-out-of-type", "unbounded-increment"],
        "TEST-2-02": ["guard-removed", "guard-not-boolean"],
        "TEST-2-03": ["write-undeclared", "read-undeclared"],
        "TEST-2-04": ["dependent-declared-independent", "diamond-on-mutant", "reduction-disagrees-on-mutant"],
        "TEST-2-05": ["conflict-undeclared", "conflict-declared-independent"],
        "TEST-2-06": ["discharge-skipped-static", "discharge-skipped-leak", "double-acquire"],
    }
    corpus_keys = {
        "TEST-2-01": ["variables_bool", "variables_int", "variables_enum"],
        "TEST-2-02": ["transitions", "guards_blocking_a_ready_process"],
        "TEST-2-03": ["footprint_shared_reads", "footprint_shared_writes"],
        "TEST-2-04": ["coenabled_independent_pairs", "coenabled_dependent_pairs", "interleavings", "trace_classes", "reduced_runs"],
        "TEST-2-05": ["declared_conflicts", "witnessed_conflicts"],
        "TEST-2-06": ["obligations", "obligations_acquired_in_some_run"],
    }
    results = {}
    for rid in OBLIGATIONS:
        results[rid] = {
            "status": "enforced",
            "rules": sorted(r for r, q in RULES.items() if q == rid),
            "failures": failures[rid],
            "corpus": {k: agg[k] for k in corpus_keys[rid]},
            "mutants": {m: tally.as_json(m) for m in mutants[rid]},
            "boundaries": BOUNDARIES[rid],
        }
    return {
        "generator": {
            "module": "tools/test-policy/tsys.py",
            "function": "generate",
            "seeds": f"{SEEDS.start}..{SEEDS.stop - 1}",
            "corpus_sha256": digest,
            "totals": {k: agg[k] for k in ("systems", "states", "interleavings", "trace_classes", "reduced_runs")},
        },
        "requirements": results,
        "unclaimed": ["TEST-2-07", "TEST-2-08"],
    }


def _mutant_requirement(msg: str) -> str:
    table = {
        "init-out-of-type": "TEST-2-01",
        "guard-": "TEST-2-02",
        "write-undeclared": "TEST-2-03",
        "read-undeclared": "TEST-2-03",
        "dependent-declared-independent": "TEST-2-04",
        "conflict-": "TEST-2-05",
        "discharge-": "TEST-2-06",
        "double-acquire": "TEST-2-06",
    }
    for prefix, rid in table.items():
        if f"mutant {prefix}" in msg:
            return rid
    return "TEST-2-04"  # the reduction/diamond consistency failure


def _blocking_guards(system: dict, report: tsys.OracleReport) -> int:
    """Transitions whose guard is false in some reachable state where their
    process's program counter is at them: the guard, not the sequencing,
    blocks a ready process."""
    c = tsys.compile_system(system)
    count = 0
    for t in c.transitions:
        pc = f"pc_{t['process']}"
        step = int(t["name"].rsplit(".s", 1)[1])
        pos = c.order.index(pc)
        if any(s[0][pos] == step and not tsys.enabled(c, s, t) for s in report.reachable):
            count += 1
    return count
