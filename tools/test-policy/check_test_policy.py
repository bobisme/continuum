#!/usr/bin/env python3
"""Executable test-policy obligations of docs/19 (`TEST-<section>-<nn>`).

Source of truth:

- `notes/plan/docs/19_TEST_STRATEGY.md` — the test strategy. Each top-level
  bullet of its section N is the requirement `TEST-N-<ordinal>`;
- `notes/plan/notes/PLAN_REQUIREMENTS.json` — the generated registry of those
  IDs (`category: test-policy`), including the `satisfied` status a
  "(delivered: bn-…)" annotation on the bullet produces.

This file is a driver. It enforces nothing about a section by itself. Each
section's obligations live in one module under `sections/`, discovered by file
name (`s<N>_<slug>.py`), so a new section is a new file and never an edit to a
line another section owns. See README-test.md for the module contract.

For every section module the driver enforces the shared conventions:

1. Binding. Every ID the module claims exists in the registry with the same
   summary, and no ID is claimed by two modules.
2. Violating fixtures. `fixtures/<module>/*.json` each name the rule they must
   trigger; `--self-test` fails if a fixture is not caught, if the fixture's
   requirement is not the rule's, if a claimed ID has no violating fixture, or
   if a `clean` control produces any finding.
3. Positive enforcement. The module's real run reports no failure for any
   claimed ID.
4. Retained output. `evidence/<module>.json` is the deterministic record of
   (2) and (3), keyed by requirement ID. The real run fails when the committed
   file differs from what this revision produces, so the retained output can
   never describe a different revision. `--evidence` rewrites it.
5. The delivered link. A requirement the registry marks `satisfied` must be
   claimed by a module with status `enforced` and a caught fixture. A
   delivered annotation without harness evidence fails here.

Exit 0 when every check holds, 1 otherwise. Python 3 stdlib only.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path
from types import ModuleType
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
REGISTRY = ROOT / "notes/plan/notes/PLAN_REQUIREMENTS.json"
SOURCE = "notes/plan/docs/19_TEST_STRATEGY.md"
SECTIONS = HERE / "sections"
FIXTURES = HERE / "fixtures"
EVIDENCE = HERE / "evidence"
EVIDENCE_SCHEMA = "continuum.test-policy.evidence/1"
STATUSES = {"enforced", "partial"}

sys.path.insert(0, str(HERE))


def load_modules() -> list[ModuleType]:
    modules = []
    for path in sorted(SECTIONS.glob("s[0-9]*_*.py")):
        spec = importlib.util.spec_from_file_location(f"test_policy_{path.stem}", path)
        assert spec and spec.loader
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        module.STEM = path.stem
        modules.append(module)
    return modules


def load_registry() -> dict[str, dict]:
    data = json.loads(REGISTRY.read_text(encoding="utf-8"))
    return {r["id"]: r for r in data["requirements"] if r["category"] == "test-policy"}


def check_binding(modules: list[ModuleType], registry: dict[str, dict]) -> list[str]:
    errors = []
    owner: dict[str, str] = {}
    for m in modules:
        for attr in ("SECTION", "TITLE", "OBLIGATIONS", "RULES", "check_fixture", "real_run"):
            if not hasattr(m, attr):
                errors.append(f"{m.STEM}: module contract: missing {attr}")
        if not m.STEM.startswith(f"s{getattr(m, 'SECTION', '?')}_"):
            errors.append(f"{m.STEM}: file name does not match SECTION = {getattr(m, 'SECTION', '?')}")
        for rid, summary in getattr(m, "OBLIGATIONS", {}).items():
            if not rid.startswith(f"TEST-{m.SECTION}-"):
                errors.append(f"{m.STEM}: claims {rid} outside section {m.SECTION}")
            if rid in owner:
                errors.append(f"{rid}: claimed by both {owner[rid]} and {m.STEM}")
            owner[rid] = m.STEM
            entry = registry.get(rid)
            if entry is None:
                errors.append(f"{m.STEM}: claims {rid}, which the registry does not contain")
            elif entry["summary"] != summary:
                errors.append(f"{m.STEM}: {rid} summary {summary!r} != registry {entry['summary']!r}")
            elif entry["source"]["path"] != SOURCE.removeprefix("notes/plan/"):
                errors.append(f"{rid}: registry source is {entry['source']['path']}, not docs/19")
        for rule, rid in getattr(m, "RULES", {}).items():
            if rid not in getattr(m, "OBLIGATIONS", {}):
                errors.append(f"{m.STEM}: rule {rule} serves {rid}, which the module does not claim")
    return errors


def load_fixtures(m: ModuleType) -> list[tuple[str, dict]]:
    out = []
    for path in sorted((FIXTURES / m.STEM).glob("*.json")):
        out.append((path.name, json.loads(path.read_text(encoding="utf-8"))))
    return out


def self_test(m: ModuleType) -> tuple[list[str], list[dict]]:
    errors: list[str] = []
    records: list[dict] = []
    fixtures = load_fixtures(m)
    covered: set[str] = set()
    controls = 0
    for fname, fx in fixtures:
        found = m.check_fixture(fx["system"])
        rules = sorted({f["rule"] for f in found})
        if fx.get("expect") == "clean":
            controls += 1
            ok = not found
            if not ok:
                errors.append(f"{m.STEM}/{fname}: negative control produced {rules}")
            records.append({"fixture": fname, "expect": "clean", "caught": rules, "ok": ok})
            continue
        rule, rid = fx.get("rule"), fx.get("requirement")
        if m.RULES.get(rule) != rid:
            errors.append(f"{m.STEM}/{fname}: rule {rule!r} does not serve {rid!r}")
        ok = rule in rules
        if not ok:
            errors.append(f"{m.STEM}/{fname}: expected {rule}, caught {rules or 'nothing'}")
        else:
            covered.add(rid)
        records.append({"fixture": fname, "requirement": rid, "rule": rule, "caught": rules, "ok": ok})
    for rid in sorted(m.OBLIGATIONS):
        if rid not in covered:
            errors.append(f"{m.STEM}: {rid} has no caught violating fixture")
    if controls == 0:
        errors.append(f"{m.STEM}: no negative control fixture (expect: clean)")
    uncaught_rules = sorted(set(m.RULES) - {r["rule"] for r in records if r.get("ok") and "rule" in r})
    if uncaught_rules:
        errors.append(f"{m.STEM}: rules with no caught fixture: {uncaught_rules}")
    return errors, records


def build_evidence(m: ModuleType, fixtures: list[dict], run: dict[str, Any], registry: dict[str, dict]) -> dict:
    requirements = {}
    for rid, summary in sorted(m.OBLIGATIONS.items()):
        entry = dict(run["requirements"][rid])
        entry["summary"] = summary
        entry["registry_status"] = registry.get(rid, {}).get("status", "missing")
        entry["fixtures"] = [f for f in fixtures if f.get("requirement") == rid]
        requirements[rid] = entry
    return {
        "schema": EVIDENCE_SCHEMA,
        "source": f"{SOURCE} §{m.SECTION} {m.TITLE}",
        "checker": "tools/test-policy/check_test_policy.py",
        "module": f"tools/test-policy/sections/{m.STEM}.py",
        "controls": [f for f in fixtures if f.get("expect") == "clean"],
        **{k: v for k, v in run.items() if k != "requirements"},
        "requirements": requirements,
    }


def render(evidence: dict) -> str:
    return json.dumps(evidence, indent=2, sort_keys=True, ensure_ascii=True) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--self-test", action="store_true", help="run the violating fixtures only")
    parser.add_argument("--evidence", action="store_true", help="rewrite evidence/<module>.json")
    parser.add_argument("--section", type=int, help="limit to one docs/19 section")
    parser.add_argument("--where", metavar="ID", help="print the evidence location of one requirement ID")
    args = parser.parse_args(argv)

    registry = load_registry()
    modules = [m for m in load_modules() if args.section is None or m.SECTION == args.section]
    errors = check_binding(modules, registry)

    if args.where:
        for m in modules:
            if args.where in m.OBLIGATIONS:
                print(f"{args.where}: tools/test-policy/evidence/{m.STEM}.json  (module sections/{m.STEM}.py, fixtures fixtures/{m.STEM}/)")
                return 0
        print(f"{args.where}: no section module claims it", file=sys.stderr)
        return 1

    for m in modules:
        fx_errors, fixtures = self_test(m)
        errors.extend(fx_errors)
        caught = sum(1 for f in fixtures if f["ok"])
        print(f"{m.STEM}: self-test {caught}/{len(fixtures)} fixtures behave as declared")
        if args.self_test:
            continue
        run = m.real_run()
        for rid, res in sorted(run["requirements"].items()):
            if res["status"] not in STATUSES:
                errors.append(f"{rid}: unknown status {res['status']!r}")
            for failure in res["failures"]:
                errors.append(f"{rid}: {failure}")
            print(f"{m.STEM}: {rid} {res['status']}, {len(res['failures'])} failure(s)")
        evidence = build_evidence(m, fixtures, run, registry)
        path = EVIDENCE / f"{m.STEM}.json"
        text = render(evidence)
        if args.evidence:
            EVIDENCE.mkdir(exist_ok=True)
            path.write_text(text, encoding="utf-8")
            print(f"{m.STEM}: wrote {path.relative_to(ROOT)}")
        elif not path.exists() or path.read_text(encoding="utf-8") != text:
            errors.append(
                f"{path.relative_to(ROOT)} is stale or missing: it does not record this revision's run "
                "(rerun with --evidence and commit the result)"
            )
        # The delivered link: registry 'satisfied' needs enforced evidence.
        for rid in sorted(m.OBLIGATIONS):
            res = run["requirements"][rid]
            if registry.get(rid, {}).get("status") == "satisfied":
                if res["status"] != "enforced" or res["failures"]:
                    errors.append(f"{rid}: docs/19 marks it delivered, but its evidence status is {res['status']} with {len(res['failures'])} failure(s)")

    if args.section is None and not args.self_test:
        claimed = {rid for m in modules for rid in m.OBLIGATIONS}
        for rid, entry in sorted(registry.items()):
            if entry["status"] == "satisfied" and rid not in claimed:
                errors.append(f"{rid}: docs/19 marks it delivered, but no section module claims it")

    for e in errors:
        print(f"FAIL {e}", file=sys.stderr)
    if errors:
        print(f"test-policy: {len(errors)} failure(s)", file=sys.stderr)
        return 1
    print("test-policy: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
