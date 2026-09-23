"""docs/19 §2 "Generated transition systems" — TEST-2-07 … TEST-2-08.

The section text: "... cancellation phases; fairness annotations. For small
sizes, enumerate all interleavings and configurations. Compare every
optimized engine and reduction against this oracle."

`sections/s2_generated_systems.py` (bn-1fqu) claims the first six bullets and
explicitly defers these two (README-test.md, its own module docstring). This
module claims them without editing a line of that module, `tsys.py`, or the
`Justfile` (README-test.md "Adding a section (the extension point)").

Extension point: rather than widen `tsys`'s own system shape (`tsys.py`'s
`_SYSTEM_KEYS` is bn-1fqu's line, not touched here), a payload for this
module is `{"base": <tsys system>, "cancel": {...}, "fairness": [...]}`.
`base` is checked by `tsys.check` unmodified; the two extension fields are
checked by the functions below, which reuse `tsys.compile_system`,
`tsys.enabled`, `tsys.fire`, `tsys.explore`, `tsys.Finding` and
`tsys.SplitMix64` — the shared substrate the README asks a new section to
import rather than re-define.

docs/02 §7 "Cancellation calculus" lifecycle:

    Active -> request -> Cancelling -> drain* -> finalize* ->
                          (obligations == ∅) -> Cancelled

TEST-2-07 models this per process: `cancel["<process>"]` names one `request`
transition and `drain`/`finalize` transition lists, all owned by that
process. `request` must precede every `drain`/`finalize` step (by the
process's own program-counter index, the module's step-numbering
convention), every declared `drain` step must precede every declared
`finalize` step, and every `finalize` step must discharge an obligation the
process owns — finalize is how a Cancelling process clears its obligations.
The one rule that needs the oracle, not just the declaration: no state
reachable after a process's last declared `finalize` step may still hold an
obligation that process owns. That is "finalize only after obligations
discharge" / the "obligations == ∅ -> Cancelled" guard of docs/02 §7,
checked dynamically because reachability, not syntax, decides it.

TEST-2-08 models a weak-fairness annotation: `fairness` names transitions
that must not stay permanently ready and never taken. Two rules use the
oracle report tsys already computes: a declared-fair transition must be
co-enabled with some other transition at some reachable state (fairness
over a transition nothing ever contends for demonstrates nothing — the same
non-vacuity convention bn-1fqu applies to its own corpus: "a zero count
fails as a vacuous pass"); and no declared-fair transition may be
guard-enabled at a state with no successor. In an acyclic, terminating
system (docs/14 glossary: "Fairness — assumption restricting infinite
executions, such as weak fairness of an action"), a state with no
successor stutters forever once reached, so a transition still enabled
there is "continuously enabled" without ever firing — a genuine weak
fairness violation, and the only kind an acyclic bounded oracle can decide.

Strong fairness (infinitely-often enabled implies infinitely-often taken;
docs/25_SEMANTIC_FEATURE_MATRIX.md "weak/strong fairness") needs
infinite/cyclic behavior to mean anything: this harness's oracle only
enumerates interleavings on an acyclic state graph and types a cyclic one
`Inconclusive("cyclic-state-graph", ...)` (`tsys.explore`), never a pass. No
fixture or corpus system here can demonstrate a strong-fairness violation,
so strong fairness is NOT covered by this module. See BOUNDARIES.
"""

from __future__ import annotations

import copy
import hashlib
from typing import Any

import tsys

SECTION = 2
TITLE = "Generated transition systems"
OBLIGATIONS = {
    "TEST-2-07": "cancellation phases",
    "TEST-2-08": "fairness annotations.",
}
RULES: dict[str, str] = {
    "cancel-declared": "TEST-2-07",
    "cancel-phase-order": "TEST-2-07",
    "cancel-finalize-discharges": "TEST-2-07",
    "cancel-cancelled-clears-obligations": "TEST-2-07",
    "fairness-declared": "TEST-2-08",
    "fairness-witnessed-choice": "TEST-2-08",
    "fairness-not-stuck": "TEST-2-08",
}
SEEDS = range(64)

BOUNDARIES = {
    "TEST-2-07": [
        "A process's cancellation state is derived from its declared request/drain/finalize "
        "transitions and the obligations it owns; there is no separate effect-protocol state "
        "(Idle/Reserved/Committed/Aborted, docs/02 §7) and no finalizer ownership transfer "
        "(also not modeled by tsys.py / TEST-2-06, see s2_generated_systems.py).",
    ],
    "TEST-2-08": [
        "Only weak fairness is checked, and only within one finite maximal run: a "
        "declared-fair transition must (a) be co-enabled with another transition somewhere "
        "reachable, and (b) never be guard-enabled at a state with no successor. Strong "
        "fairness is NOT covered: it is a property of infinitely-often-enabled behaviour over "
        "an infinite or cyclic run, and this harness's oracle only enumerates interleavings on "
        "an acyclic state graph (a cyclic one is a typed Inconclusive, tsys.explore). No "
        "fixture or corpus system here can demonstrate a strong-fairness violation.",
    ],
}


def _finding(rule: str, subject: str, message: str) -> tsys.Finding:
    return tsys.Finding(rule, RULES[rule], subject, message)


def _step_index(tname: str) -> int | None:
    if ".s" not in tname:
        return None
    tail = tname.rsplit(".s", 1)[1]
    return int(tail) if tail.isdigit() else None


# ---------------------------------------------------------------------------
# TEST-2-07: cancellation phases
# ---------------------------------------------------------------------------


def _cancel_static(base: Any, cancel: Any) -> tuple[list[tsys.Finding], dict[str, dict]]:
    """Well-formedness of the `cancel` map. Returns (findings, resolved), where
    resolved[process] = {"finalize": [step indices], "finalize_names": [...]},
    for processes whose declaration was well formed enough to resolve."""
    findings: list[tsys.Finding] = []
    resolved: dict[str, dict] = {}
    if not isinstance(base, dict):
        return findings, resolved  # tsys.check already reports this on `base`
    by_name = {t["name"]: t for t in base.get("transitions", []) if isinstance(t, dict) and isinstance(t.get("name"), str)}
    obligations = {o["name"]: o["owner"] for o in base.get("obligations", []) if isinstance(o, dict)}
    if not isinstance(cancel, dict):
        findings.append(_finding("cancel-declared", "<cancel>", "cancel must be an object keyed by process name"))
        return findings, resolved
    for process, decl in sorted(cancel.items()):
        if not (isinstance(decl, dict) and set(decl) == {"request", "drain", "finalize"}):
            findings.append(_finding("cancel-declared", process, "cancel entry must have exactly request/drain/finalize"))
            continue
        request, drain, finalize = decl["request"], decl["drain"], decl["finalize"]
        shape_ok = (
            isinstance(request, str)
            and isinstance(drain, list) and all(isinstance(x, str) for x in drain)
            and isinstance(finalize, list) and all(isinstance(x, str) for x in finalize) and finalize
        )
        if not shape_ok:
            findings.append(_finding("cancel-declared", process, "request must name one transition, drain/finalize must be lists of names, finalize non-empty"))
            continue
        names = [request, *drain, *finalize]
        if len(set(names)) != len(names):
            findings.append(_finding("cancel-declared", process, "request/drain/finalize name a transition more than once"))
            continue
        idx: dict[str, int] = {}
        bad = False
        for tname in names:
            t = by_name.get(tname)
            if t is None or t.get("process") != process:
                findings.append(_finding("cancel-declared", process, f"{tname!r} is not a transition owned by {process!r}"))
                bad = True
                continue
            step = _step_index(tname)
            if step is None:
                findings.append(_finding("cancel-declared", process, f"{tname!r} has no step index (expected '<process>.s<k>')"))
                bad = True
                continue
            idx[tname] = step
        if bad:
            continue
        request_idx = idx[request]
        drain_idx = [idx[d] for d in drain]
        finalize_idx = [idx[f] for f in finalize]
        if any(request_idx >= i for i in drain_idx) or any(request_idx >= i for i in finalize_idx):
            findings.append(_finding("cancel-phase-order", process, "request does not precede every drain/finalize step"))
        if drain_idx and finalize_idx and max(drain_idx) >= min(finalize_idx):
            findings.append(_finding("cancel-phase-order", process, "a drain step does not precede every finalize step"))
        for fname in finalize:
            t = by_name[fname]
            discharged = t.get("discharge", []) if isinstance(t.get("discharge", []), list) else []
            owned = [o for o in discharged if obligations.get(o) == process]
            if not owned:
                findings.append(_finding("cancel-finalize-discharges", f"{process}/{fname}", "finalize step discharges no obligation it owns"))
        resolved[process] = {"finalize": finalize_idx, "finalize_names": list(finalize)}
    return findings, resolved


def _cancel_dynamic(base: dict, resolved: dict[str, dict], report: tsys.OracleReport) -> list[tsys.Finding]:
    findings: list[tsys.Finding] = []
    c = tsys.compile_system(base)
    obligations = {o["name"]: o["owner"] for o in base.get("obligations", [])}
    for process, phases in sorted(resolved.items()):
        if not phases["finalize"]:
            continue
        pc = f"pc_{process}"
        if pc not in c.order:
            continue
        pos = c.order.index(pc)
        last_finalize = max(phases["finalize"])
        owned = {o for o, owner in obligations.items() if owner == process}
        if not owned:
            continue
        for s in report.reachable:
            held = set(s[1]) & owned
            if s[0][pos] > last_finalize and held:
                findings.append(_finding(
                    "cancel-cancelled-clears-obligations",
                    process,
                    f"state past {process}'s last finalize step still holds {sorted(held)}",
                ))
                break
    return findings


# ---------------------------------------------------------------------------
# TEST-2-08: fairness annotations
# ---------------------------------------------------------------------------


def _fairness_static(base: Any, fairness: Any) -> list[tsys.Finding]:
    findings: list[tsys.Finding] = []
    if not isinstance(base, dict):
        return findings
    names = {t["name"] for t in base.get("transitions", []) if isinstance(t, dict) and isinstance(t.get("name"), str)}
    if not (isinstance(fairness, list) and all(isinstance(x, str) for x in fairness)):
        findings.append(_finding("fairness-declared", "<fairness>", "fairness must be a list of transition names"))
        return findings
    if len(set(fairness)) != len(fairness):
        findings.append(_finding("fairness-declared", "<fairness>", "fairness list declares a transition more than once"))
    for tname in fairness:
        if tname not in names:
            findings.append(_finding("fairness-declared", tname, "fairness names an undeclared transition"))
    return findings


def _fairness_analysis(base: dict, fairness: list[str], report: tsys.OracleReport) -> dict[str, dict[str, bool]]:
    """For every name in `fairness` that resolves to a real transition: whether
    it was ever co-enabled with a different transition, and whether it was
    ever guard-enabled at a state with no successor."""
    c = tsys.compile_system(base)
    valid = {t["name"]: t for t in c.transitions}
    terminal = set(report.terminal_states)
    out: dict[str, dict[str, bool]] = {}
    for tname in fairness:
        t = valid.get(tname)
        if t is None or tname in out:
            continue
        witnessed = False
        stuck = False
        for s in report.reachable:
            en = [tr["name"] for tr in c.transitions if tsys.enabled(c, s, tr)]
            if tname not in en:
                continue
            if len(en) > 1:
                witnessed = True
            if s in terminal:
                stuck = True
        out[tname] = {"witnessed_choice": witnessed, "stuck": stuck}
    return out


def _fairness_dynamic(base: dict, fairness: list[str], report: tsys.OracleReport) -> list[tsys.Finding]:
    findings: list[tsys.Finding] = []
    for tname, info in sorted(_fairness_analysis(base, fairness, report).items()):
        if not info["witnessed_choice"]:
            findings.append(_finding("fairness-witnessed-choice", tname, "never co-enabled with another transition in a reachable state"))
        if info["stuck"]:
            findings.append(_finding("fairness-not-stuck", tname, "guard-enabled at a state with no successor: it can never actually fire"))
    return findings


# ---------------------------------------------------------------------------
# Driving both extensions over one payload
# ---------------------------------------------------------------------------


def _check(payload: Any) -> tuple[list[tsys.Finding], tsys.OracleReport | None]:
    if not isinstance(payload, dict) or set(payload) != {"base", "cancel", "fairness"}:
        return [_finding("cancel-declared", "<system>", "payload must have exactly base/cancel/fairness")], None
    base = payload["base"]
    base_findings, report = tsys.check(base)
    findings: list[tsys.Finding] = list(base_findings)
    cancel_findings, resolved = _cancel_static(base, payload["cancel"])
    findings.extend(cancel_findings)
    fairness = payload["fairness"]
    findings.extend(_fairness_static(base, fairness))
    if report is not None:
        findings.extend(_cancel_dynamic(base, resolved, report))
        if isinstance(fairness, list) and all(isinstance(x, str) for x in fairness):
            findings.extend(_fairness_dynamic(base, fairness, report))
    return sorted(set(findings)), report


def check_fixture(system: Any) -> list[dict[str, str]]:
    findings, _report = _check(system)
    return [f.as_json() for f in findings]


# ---------------------------------------------------------------------------
# The generator
# ---------------------------------------------------------------------------


def generate(seed: int) -> dict:
    """One payload, a pure function of `seed` (INV-005: explicit `tsys.SplitMix64`).

    Two processes. Each is a straight line: `work` (acquires its own
    obligation), `request` (enters Cancelling), 1-2 `drain` steps, `finalize`
    (discharges the obligation). `p1`'s drain step(s) read a boolean flag
    that only `p0`'s `work` step sets, so the two processes are genuinely
    interleaved rather than independent throughout — enough for the corpus
    to witness real cross-process co-enablement (TEST-2-08's non-vacuity
    rule) without relying on the generator ever producing a stuck run.
    """
    rng = tsys.SplitMix64(seed)
    ndrain = 1 + rng.below(2)
    variables: list[dict] = [{"name": "flag", "type": {"kind": "bool"}}]
    init: dict[str, Any] = {"flag": False}
    obligations: list[dict] = []
    transitions: list[dict] = []
    cancel: dict[str, dict] = {}
    fairness: list[str] = []

    for p in range(2):
        proc = f"p{p}"
        pc = f"pc_{proc}"
        total = 3 + ndrain  # work(0), request(1), drain(2..1+ndrain), finalize(2+ndrain)
        variables.append({"name": pc, "type": {"kind": "int", "min": 0, "max": total}})
        init[pc] = 0
        obl = f"o_{proc}"
        obligations.append({"name": obl, "owner": proc})
        res = tsys.obligation_resource(obl)

        work = {
            "name": f"{proc}.s0",
            "process": proc,
            "guard": {"op": "eq", "args": [{"var": pc}, {"const": 0}]},
            "updates": {pc: {"op": "add", "args": [{"var": pc}, {"const": 1}]}},
            "acquire": [obl],
            "reads": [pc, res],
            "writes": [pc, res],
        }
        if p == 0:
            work["updates"]["flag"] = {"const": True}
            work["reads"] = work["reads"] + ["flag"]
            work["writes"] = work["writes"] + ["flag"]
        transitions.append(work)

        request = {
            "name": f"{proc}.s1",
            "process": proc,
            "guard": {"op": "eq", "args": [{"var": pc}, {"const": 1}]},
            "updates": {pc: {"op": "add", "args": [{"var": pc}, {"const": 1}]}},
            "reads": [pc],
            "writes": [pc],
        }
        transitions.append(request)

        drain_names: list[str] = []
        for k in range(ndrain):
            step = 2 + k
            guard: dict = {"op": "eq", "args": [{"var": pc}, {"const": step}]}
            reads = [pc]
            if p == 1:
                guard = {"op": "and", "args": [guard, {"op": "eq", "args": [{"var": "flag"}, {"const": True}]}]}
                reads = reads + ["flag"]
            drain = {
                "name": f"{proc}.s{step}",
                "process": proc,
                "guard": guard,
                "updates": {pc: {"op": "add", "args": [{"var": pc}, {"const": 1}]}},
                "reads": reads,
                "writes": [pc],
            }
            transitions.append(drain)
            drain_names.append(drain["name"])

        fin_step = 2 + ndrain
        finalize = {
            "name": f"{proc}.s{fin_step}",
            "process": proc,
            "guard": {"op": "eq", "args": [{"var": pc}, {"const": fin_step}]},
            "updates": {pc: {"op": "add", "args": [{"var": pc}, {"const": 1}]}},
            "discharge": [obl],
            "reads": [pc, res],
            "writes": [pc, res],
        }
        transitions.append(finalize)

        cancel[proc] = {"request": request["name"], "drain": drain_names, "finalize": [finalize["name"]]}
        fairness.append(finalize["name"])

    independent: list[list[str]] = []
    conflicts: list[list[str]] = []
    for i, a in enumerate(transitions):
        for b in transitions[i + 1 :]:
            if a["process"] == b["process"]:
                continue
            ra, wa, rb, wb = set(a["reads"]), set(a["writes"]), set(b["reads"]), set(b["writes"])
            if not ((wa & wb) | (wa & rb) | (ra & wb)):
                independent.append([a["name"], b["name"]])
            if (wa & tsys.expr_vars(b["guard"])) or (wb & tsys.expr_vars(a["guard"])):
                conflicts.append([a["name"], b["name"]])

    base = {
        "name": f"cf-{seed}",
        "variables": variables,
        "init": init,
        "obligations": obligations,
        "transitions": transitions,
        "independent": independent,
        "conflicts": conflicts,
    }
    return {"base": base, "cancel": cancel, "fairness": fairness}


# ---------------------------------------------------------------------------
# The real run: corpus over SEEDS, plus a small structural mutation check
# ---------------------------------------------------------------------------


def _rules_of(findings: list[tsys.Finding]) -> set[str]:
    return {f.rule for f in findings}


def real_run() -> dict[str, Any]:
    systems = [generate(seed) for seed in SEEDS]
    digest = hashlib.sha256("\n".join(tsys.canonical(s) for s in systems).encode()).hexdigest()
    failures: dict[str, list[str]] = {rid: [] for rid in OBLIGATIONS}
    agg = {
        "systems": len(systems),
        "states": 0,
        "interleavings": 0,
        "trace_classes": 0,
        "reduced_runs": 0,
        "cancel_processes": 0,
        "finalize_transitions": 0,
        "fairness_transitions": 0,
        "fairness_choice_witnessed": 0,
    }
    applied: dict[str, int] = {}
    detected: dict[str, int] = {}
    mutant_failures: list[str] = []

    def record(name: str, ok: bool, subject: str) -> None:
        applied[name] = applied.get(name, 0) + 1
        if ok:
            detected[name] = detected.get(name, 0) + 1
        else:
            mutant_failures.append(f"mutant {name} on {subject}: expected detection, got none")

    for payload in systems:
        base = payload["base"]
        findings, report = _check(payload)
        for f in findings:
            failures[f.requirement].append(f"{base['name']}: {f.rule} {f.subject}: {f.message}")
        if report is None:
            continue
        for inc in report.inconclusive:
            for rid in OBLIGATIONS:
                failures[rid].append(f"{base['name']}: inconclusive {inc.reason}: {inc.detail}")
        agg["states"] += report.states
        agg["interleavings"] += report.interleavings
        agg["trace_classes"] += report.trace_classes
        agg["reduced_runs"] += report.reduced_runs
        agg["cancel_processes"] += len(payload["cancel"])
        agg["finalize_transitions"] += sum(len(d["finalize"]) for d in payload["cancel"].values())
        agg["fairness_transitions"] += len(payload["fairness"])
        analysis = _fairness_analysis(base, payload["fairness"], report)
        agg["fairness_choice_witnessed"] += sum(1 for info in analysis.values() if info["witnessed_choice"])

        proc0 = sorted(payload["cancel"])[0]
        name = base["name"]

        # TEST-2-07 structural mutants: always statically detected.
        m = copy.deepcopy(payload)
        m["cancel"] = ["not", "a", "mapping"]
        mfindings, _ = _check(m)
        record("cancel-not-object", "cancel-declared" in _rules_of(mfindings), name)

        m = copy.deepcopy(payload)
        old_request = m["cancel"][proc0]["request"]
        old_finalize = m["cancel"][proc0]["finalize"][0]
        m["cancel"][proc0]["request"] = old_finalize
        m["cancel"][proc0]["finalize"] = [old_request]
        mfindings, _ = _check(m)
        record("phase-order-broken", "cancel-phase-order" in _rules_of(mfindings), f"{name}/{proc0}")

        m = copy.deepcopy(payload)
        fin_name = m["cancel"][proc0]["finalize"][0]
        for t in m["base"]["transitions"]:
            if t["name"] == fin_name:
                t.pop("discharge", None)
        mfindings, _ = _check(m)
        record("finalize-no-discharge", "cancel-finalize-discharges" in _rules_of(mfindings), f"{name}/{fin_name}")

        # TEST-2-07 dynamic mutant: shrink the finalize window so it no
        # longer covers the transition that actually discharges the
        # obligation (the drain step becomes the declared "finalize").
        m = copy.deepcopy(payload)
        drains = m["cancel"][proc0]["drain"]
        if drains:
            moved = drains[-1]
            m["cancel"][proc0]["drain"] = drains[:-1]
            m["cancel"][proc0]["finalize"] = [moved]
            mfindings, _ = _check(m)
            record("finalize-window-too-narrow", "cancel-cancelled-clears-obligations" in _rules_of(mfindings), f"{name}/{proc0}")

        # TEST-2-08 structural mutant: always statically detected.
        m = copy.deepcopy(payload)
        m["fairness"] = m["fairness"] + ["no-such-transition"]
        mfindings, _ = _check(m)
        record("fairness-undeclared-name", "fairness-declared" in _rules_of(mfindings), name)

    for msg in mutant_failures:
        rid = "TEST-2-07" if msg.startswith(("mutant cancel", "mutant phase", "mutant finalize")) else "TEST-2-08"
        failures[rid].append(msg)

    def need(rid: str, key: str, what: str) -> None:
        if agg[key] == 0:
            failures[rid].append(f"corpus exercises no {what} ({key} = 0): the pass would be vacuous")

    def need_detected(rid: str, mutant: str) -> None:
        if detected.get(mutant, 0) == 0:
            failures[rid].append(f"mutant {mutant} was never detected across the corpus")

    need("TEST-2-07", "cancel_processes", "declared cancellation phase set")
    need("TEST-2-07", "finalize_transitions", "finalize transition")
    need_detected("TEST-2-07", "cancel-not-object")
    need_detected("TEST-2-07", "phase-order-broken")
    need_detected("TEST-2-07", "finalize-no-discharge")
    need_detected("TEST-2-07", "finalize-window-too-narrow")

    need("TEST-2-08", "fairness_transitions", "declared-fair transition")
    need("TEST-2-08", "fairness_choice_witnessed", "reachable co-enabled fairness choice")
    need_detected("TEST-2-08", "fairness-undeclared-name")

    mutants = {
        "TEST-2-07": ["cancel-not-object", "phase-order-broken", "finalize-no-discharge", "finalize-window-too-narrow"],
        "TEST-2-08": ["fairness-undeclared-name"],
    }
    corpus_keys = {
        "TEST-2-07": ["cancel_processes", "finalize_transitions"],
        "TEST-2-08": ["fairness_transitions", "fairness_choice_witnessed"],
    }
    results = {}
    for rid in OBLIGATIONS:
        results[rid] = {
            "status": "enforced",
            "rules": sorted(r for r, q in RULES.items() if q == rid),
            "failures": failures[rid],
            "corpus": {k: agg[k] for k in corpus_keys[rid]},
            "mutants": {m: {"applied": applied.get(m, 0), "detected": detected.get(m, 0)} for m in mutants[rid]},
            "boundaries": BOUNDARIES[rid],
        }
    return {
        "generator": {
            "module": "tools/test-policy/sections/s2_cancellation_fairness.py",
            "function": "generate",
            "seeds": f"{SEEDS.start}..{SEEDS.stop - 1}",
            "corpus_sha256": digest,
            "totals": {k: agg[k] for k in ("systems", "states", "interleavings", "trace_classes", "reduced_runs")},
        },
        "requirements": results,
    }
