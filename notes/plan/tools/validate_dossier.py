#!/usr/bin/env python3
"""Mechanical validation for the Continuum architecture/research dossier."""
from __future__ import annotations

import ast
import os
import csv
import json
import re
import subprocess
import sys
import tomllib
from collections import Counter
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]

SCHEMA_PAIRS = {
    "schemas/cir.schema.json": "schemas/examples/minimal.cir.json",
    "schemas/crashpack.schema.json": "schemas/examples/counterexample.crashpack.json",
    "schemas/assurance-result.schema.json": "schemas/examples/finite-proof.assurance.json",
    "schemas/domain-pack.schema.json": "schemas/examples/storage-pack.manifest.json",
    "schemas/corpus-port.schema.json": "schemas/examples/diehard.corpus-port.json",
    "schemas/proof-receipt.schema.json": "schemas/examples/finite-closure.proof-receipt.json",
    "schemas/intent-contract.schema.json": "schemas/examples/intent-contract.example.json",
    "schemas/workspace-snapshot.schema.json": "schemas/examples/workspace-snapshot.example.json",
    "schemas/verification-task.schema.json": "schemas/examples/verification-task.example.json",
    "schemas/context-pack.schema.json": "schemas/examples/context-pack.example.json",
    "schemas/semantic-diff.schema.json": "schemas/examples/semantic-diff.example.json",
    "schemas/repair-transaction.schema.json": "schemas/examples/repair-transaction.example.json",
    "schemas/synthesis-candidate.schema.json": "schemas/examples/synthesis-candidate.example.json",
    "schemas/benchmark-task.schema.json": "schemas/examples/benchmark-task.example.json",
    "schemas/evidence-graph-node.schema.json": "schemas/examples/evidence-graph-node.example.json",
}

LINK_RE = re.compile(r"(?<!!)\[[^\]]*\]\(([^)]+)\)")
BIB_ID_RE = re.compile(r"^### \[(S[^\]]+)\]", re.MULTILINE)


def run(cmd: list[str], cwd: Path = ROOT) -> str:
    return subprocess.check_output(
        cmd, cwd=cwd, text=True, stderr=subprocess.STDOUT,
        env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"},
    )


def check_json() -> dict[str, Any]:
    paths = sorted(ROOT.rglob("*.json"))
    for path in paths:
        json.loads(path.read_text(encoding="utf-8"))
    return {"files": len(paths)}


def check_schemas() -> dict[str, Any]:
    for schema_rel, instance_rel in SCHEMA_PAIRS.items():
        schema = json.loads((ROOT / schema_rel).read_text(encoding="utf-8"))
        instance = json.loads((ROOT / instance_rel).read_text(encoding="utf-8"))
        Draft202012Validator.check_schema(schema)
        errors = sorted(Draft202012Validator(schema).iter_errors(instance), key=lambda e: list(e.path))
        if errors:
            formatted = "\n".join(f"{instance_rel}:{list(e.path)}: {e.message}" for e in errors)
            raise AssertionError(formatted)
    return {"pairs": len(SCHEMA_PAIRS)}


def check_toml() -> dict[str, Any]:
    paths = sorted(ROOT.rglob("*.toml"))
    for path in paths:
        tomllib.loads(path.read_text(encoding="utf-8"))
    return {"files": len(paths)}


def check_python() -> dict[str, Any]:
    paths = sorted(ROOT.rglob("*.py"))
    for path in paths:
        ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    return {"files": len(paths)}


def check_spikes() -> dict[str, Any]:
    output_r2 = run([sys.executable, "spikes/run_spikes.py"])
    results = json.loads((ROOT / "spikes/results/spike-results.json").read_text(encoding="utf-8"))
    assert results["diehard"]["states"] == 16
    assert results["diehard"]["shortest_solution_depth"] == 6
    assert results["dining_philosophers_5"]["states"] == 573
    assert results["dining_philosophers_5"]["shortest_deadlock_depth"] == 10
    assert results["advanced"]["cyclic_symmetry"]["quotient_states"] == 117
    assert results["advanced"]["cyclic_symmetry"]["automorphism_check_valid"] is True
    assert results["stuttering_refinement"]["valid"] is True
    assert results["advanced"]["assumption_safety_game"]["minimum_removed_move_count"] == 1
    assert [m["action"] for m in results["advanced"]["assumption_safety_game"]["synthesized_forbidden_environment_moves"]] == ["AckBeforeSync"]

    output_r3 = run([sys.executable, "spikes/run_r3_spikes.py"])
    r3 = json.loads((ROOT / "spikes/results/r3-spike-results.json").read_text(encoding="utf-8"))
    assert r3["context_pack"]["replay_preserving"] is True
    assert r3["context_pack"]["one_minimal"] is True
    assert r3["context_pack"]["core_event_count"] == 4
    assert r3["semantic_diff"]["all_detected"] is True
    assert r3["agent_protocol"]["idempotent"] is True
    assert r3["agent_protocol"]["stale_snapshot_rejected"] is True
    assert r3["agent_protocol"]["idempotency_mismatch_rejected"] is True
    assert r3["forge_cegis"]["solution"] == "synced"
    assert r3["forge_cegis"]["safety"] is True
    assert r3["forge_cegis"]["progress_nonvacuity"] is True
    assert r3["incremental"]["all_clean_parity"] is True
    assert r3["causal_debugger"]["failing_branch"]["violation"] is True
    assert r3["causal_debugger"]["safe_branch"]["violation"] is False
    assert r3["proof_lens"]["silent_choice_made"] is False
    assert r3["proof_lens"]["unambiguous_ack_roundtrip"] is True
    assert r3["evidence_graph"]["duplicate_proposal_deduplicated"] is True
    assert r3["evidence_graph"]["unauthorized_promotion_rejected"] is True
    assert r3["evidence_graph"]["stale_evidence_rejected"] is True
    assert r3["evidence_graph"]["accepted_after_complete_fresh_envelope"] is True
    assert r3["evidence_graph"]["intent_gaming_blocked"] is True
    assert r3["evidence_graph"]["conflicts_surfaced"] is True

    return {
        "revision_2_runner": output_r2.strip().splitlines()[-1] if output_r2.strip() else "",
        "revision_3_runner": output_r3.strip().splitlines()[-1] if output_r3.strip() else "",
        "assertions": 30,
    }


def check_inventory() -> dict[str, Any]:
    output = run([sys.executable, "corpus/tla-examples/check_inventory.py"])
    data = json.loads(output)
    assert data["status"] == "pass"
    return {"validated": data["validated_count"], "extended": data["other_count"]}



def check_benchmark_tasks() -> dict[str, Any]:
    schema = json.loads((ROOT / "schemas/benchmark-task.schema.json").read_text(encoding="utf-8"))
    validator = Draft202012Validator(schema)
    tasks = sorted((ROOT / "benchmarks/continuum-bench/tasks").glob("*.json"))
    for path in tasks:
        instance = json.loads(path.read_text(encoding="utf-8"))
        errors = sorted(validator.iter_errors(instance), key=lambda e: list(e.path))
        if errors:
            formatted = "\n".join(
                f"{path.relative_to(ROOT)}:{list(e.path)}: {e.message}" for e in errors
            )
            raise AssertionError(formatted)
    return {"tasks": len(tasks)}

def check_markdown_links() -> dict[str, Any]:
    broken: list[str] = []
    checked = 0
    for path in sorted(ROOT.rglob("*.md")):
        text = path.read_text(encoding="utf-8")
        for raw in LINK_RE.findall(text):
            target = raw.strip().split()[0].strip("<>")
            if not target or target.startswith(("http://", "https://", "mailto:", "#")):
                continue
            target = target.split("#", 1)[0]
            if not target:
                continue
            checked += 1
            resolved = (path.parent / target).resolve()
            try:
                resolved.relative_to(ROOT.resolve())
            except ValueError:
                broken.append(f"{path.relative_to(ROOT)} -> {raw} (escapes root)")
                continue
            if not resolved.exists():
                broken.append(f"{path.relative_to(ROOT)} -> {raw}")
    if broken:
        raise AssertionError("broken relative links:\n" + "\n".join(broken))
    return {"links": checked}


def check_fences() -> dict[str, Any]:
    bad: list[str] = []
    for path in sorted(ROOT.rglob("*.md")):
        count = sum(1 for line in path.read_text(encoding="utf-8").splitlines() if line.lstrip().startswith("```"))
        if count % 2:
            bad.append(str(path.relative_to(ROOT)))
    if bad:
        raise AssertionError(f"unbalanced code fences: {bad}")
    return {"files": len(list(ROOT.rglob('*.md')))}


def check_empty() -> dict[str, Any]:
    empty = [
        str(p.relative_to(ROOT)) for p in ROOT.rglob("*")
        if p.is_file() and "__pycache__" not in p.parts and p.stat().st_size == 0
    ]
    if empty:
        raise AssertionError(f"empty files: {empty}")
    return {"files": sum(
        1 for p in ROOT.rglob("*") if p.is_file() and "__pycache__" not in p.parts
    )}


def check_numbering() -> dict[str, Any]:
    result: dict[str, Any] = {}
    for directory in ("adr", "rfcs"):
        ids = [p.name[:4] for p in (ROOT / directory).glob("[0-9][0-9][0-9][0-9]-*.md")]
        duplicates = [item for item, count in Counter(ids).items() if count > 1]
        assert not duplicates, (directory, duplicates)
        result[directory] = len(ids)
    bib_text = (ROOT / "docs/13_BIBLIOGRAPHY.md").read_text(encoding="utf-8")
    bib_ids = BIB_ID_RE.findall(bib_text)
    duplicate_bib = [item for item, count in Counter(bib_ids).items() if count > 1]
    assert not duplicate_bib, duplicate_bib
    result["bibliography_entries"] = len(bib_ids)
    return result


def check_lean_source() -> dict[str, Any]:
    paths = sorted((ROOT / "lean").rglob("*.lean"))
    violations: list[str] = []
    for path in paths:
        text = path.read_text(encoding="utf-8")
        for token in (r"\bsorry\b", r"\badmit\b", r"(?m)^\s*axiom\b"):
            if re.search(token, text):
                violations.append(f"{path.relative_to(ROOT)}:{token}")
    if violations:
        raise AssertionError(f"Lean placeholder/axiom tokens: {violations}")
    return {"files": len(paths), "kernel_checked": False}


def check_csv() -> dict[str, Any]:
    result = {}
    for rel in ("corpus/tla-examples/validated-examples.csv", "corpus/tla-examples/other-examples.csv"):
        with (ROOT / rel).open(newline="", encoding="utf-8") as handle:
            rows = list(csv.DictReader(handle))
        assert rows
        result[rel] = len(rows)
    return result


def main() -> None:
    checks = [
        ("json", check_json),
        ("schemas", check_schemas),
        ("toml", check_toml),
        ("python", check_python),
        ("spikes", check_spikes),
        ("corpus_inventory", check_inventory),
        ("benchmark_tasks", check_benchmark_tasks),
        ("markdown_links", check_markdown_links),
        ("markdown_fences", check_fences),
        ("empty_files", check_empty),
        ("numbering", check_numbering),
        ("lean_source", check_lean_source),
        ("csv", check_csv),
    ]
    report: dict[str, Any] = {"status": "pass", "checks": {}}
    for name, fn in checks:
        report["checks"][name] = fn()
    print(json.dumps(report, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
