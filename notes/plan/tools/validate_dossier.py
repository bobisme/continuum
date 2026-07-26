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
    "schemas/evidence-graph-edge.schema.json": "schemas/examples/evidence-graph-edge.example.json",
}

LINK_RE = re.compile(r"(?<!!)\[[^\]]*\]\(([^)]+)\)")
BIB_ID_RE = re.compile(r"^### \[(S[^\]]+)\]", re.MULTILINE)

# Retired terminology (plan §25 / review-2 Appendix A). The optional
# "differential" captures the pre-rename long form of the same retired gate name.
RETIRED_NAME_RES = [
    re.compile(r"clean[-\s]build\s+(?:differential\s+)?tribunal", re.IGNORECASE),
    re.compile(r"clean\s+recomputation\s+(?:differential\s+)?tribunal", re.IGNORECASE),
]
CP_HANDLE_RE = re.compile(r"[\"'`]cp_[A-Za-z0-9]")
GATE_HEADING_RE = re.compile(r"^#{2,3}\s+G(\d+)\s*[—–-]", re.MULTILINE)
GATE_SUFFIX_RE = re.compile(r"\bG\d+-(?:Corpus|Proof)\b")
PR_HEADING_RE = re.compile(r"^### PR (\d+)\b.*$", re.MULTILINE)
PR_GATE_ANNOTATION_RE = re.compile(r"\[G[0-9]")
PREFIX_LINE_RE = re.compile(r"^([a-z]+)_\*", re.MULTILINE)


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


def _is_excluded_from_retired_scan(path: Path) -> bool:
    rel = path.relative_to(ROOT)
    if "archive" in rel.parts or "__pycache__" in rel.parts or ".git" in rel.parts:
        return True
    if rel.name.startswith("plan.review.") and rel.suffix == ".md":
        return True
    if rel.as_posix() == "docs/26_RELEASE_GATES_REV2.md":
        return True
    # The validator defines the retired phrases; generated reports may quote them.
    if rel.as_posix() in ("tools/validate_dossier.py", "validation-results.json"):
        return True
    return False


def check_retired_names() -> dict[str, Any]:
    phrase_hits: list[str] = []
    handle_hits: list[str] = []
    scanned = 0
    for path in sorted(ROOT.rglob("*")):
        if not path.is_file() or _is_excluded_from_retired_scan(path):
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        scanned += 1
        rel = path.relative_to(ROOT)
        for number, line in enumerate(text.splitlines(), start=1):
            for pattern in RETIRED_NAME_RES:
                match = pattern.search(line)
                if not match:
                    continue
                # Historical mentions documenting the rename itself are allowed:
                # e.g. (Renamed from "clean-build Tribunal"; ...) or
                # 'formerly called the "clean-build Tribunal"'.
                if re.search(r"renamed from|formerly called", line[: match.start()], re.IGNORECASE):
                    continue
                phrase_hits.append(f"{rel}:{number}: retired Tribunal name: {line.strip()}")
                break
        # cp_ crashpack handles (retired in favor of crash_*): docs/, examples/,
        # schemas/, notes/ only; the synthesis-candidate example legitimately
        # uses cp_reject_true-style counterexample labels.
        if rel.parts[0] in ("docs", "examples", "schemas", "notes") and rel.as_posix() != (
            "schemas/examples/synthesis-candidate.example.json"
        ):
            for number, line in enumerate(text.splitlines(), start=1):
                if CP_HANDLE_RE.search(line):
                    handle_hits.append(f"{rel}:{number}: retired cp_ handle: {line.strip()}")
    if phrase_hits or handle_hits:
        raise AssertionError("retired names present:\n" + "\n".join(phrase_hits + handle_hits))
    return {"files_scanned": scanned}


def _normalize_tokens(text: str) -> frozenset[str]:
    return frozenset(re.sub(r"[^a-z0-9]+", " ", text.lower()).split())


def _parse_gate_sections(text: str) -> dict[int, list[str]]:
    """Map gate number -> list of bullet texts (wrapped bullets joined)."""
    gates: dict[int, list[str]] = {}
    current: int | None = None
    bullet: list[str] = []

    def flush() -> None:
        if current is not None and bullet:
            gates[current].append(" ".join(bullet))
        bullet.clear()

    for line in text.splitlines():
        heading = GATE_HEADING_RE.match(line)
        if heading:
            flush()
            current = int(heading.group(1))
            gates.setdefault(current, [])
            continue
        if line.startswith("#"):
            flush()
            current = None
            continue
        if current is None:
            continue
        stripped = line.strip()
        if stripped.startswith("- "):
            flush()
            bullet.append(stripped[2:].strip())
        elif bullet and stripped and line[:1].isspace():
            bullet.append(stripped)
        elif not stripped:
            flush()
    flush()
    return gates


def check_gate_scheme_correspondence() -> dict[str, Any]:
    plan_text = (ROOT / "plan.md").read_text(encoding="utf-8")
    match = re.search(r"^## 22\..*?(?=^## \d)", plan_text, re.MULTILINE | re.DOTALL)
    assert match, "plan.md section 22 not found"
    plan_gates = _parse_gate_sections(match.group(0))
    docs_gates = _parse_gate_sections(
        (ROOT / "docs/52_RELEASE_GATES_REV3.md").read_text(encoding="utf-8")
    )
    expected = set(range(11))
    assert set(plan_gates) == expected, f"plan.md section 22 gates: {sorted(plan_gates)}"
    assert set(docs_gates) == expected, f"docs/52 gates: {sorted(docs_gates)}"

    def _orphans(
        source_gates: dict[int, list[str]],
        target_gates: dict[int, list[str]],
        source_name: str,
        target_name: str,
    ) -> tuple[list[str], int]:
        unmatched: list[str] = []
        compared = 0
        for gate in sorted(source_gates):
            target_token_sets = [_normalize_tokens(b) for b in target_gates[gate]]
            for bullet in source_gates[gate]:
                compared += 1
                tokens = _normalize_tokens(bullet)
                best = max(
                    (
                        len(tokens & target_tokens) / len(tokens | target_tokens)
                        for target_tokens in target_token_sets
                        if tokens | target_tokens
                    ),
                    default=0.0,
                )
                if best < 0.6:
                    unmatched.append(
                        f"G{gate}: no {target_name} bullet with >=0.6 overlap "
                        f"(best {best:.2f}) for {source_name} bullet: {bullet}"
                    )
        return unmatched, compared

    # Bidirectional: a bullet dropped from either document is a failure
    # (plan section 22 claims "no docs/52 criterion is dropped here").
    plan_orphans, plan_compared = _orphans(plan_gates, docs_gates, "plan", "docs/52")
    docs_orphans, docs_compared = _orphans(docs_gates, plan_gates, "docs/52", "plan")
    unmatched = plan_orphans + docs_orphans
    if unmatched:
        raise AssertionError("plan section 22 <-> docs/52 mismatch:\n" + "\n".join(unmatched))
    return {
        "gates": len(expected),
        "plan_bullets": plan_compared,
        "docs_bullets": docs_compared,
    }


def _numbered_markdown(directory: str, low: int, high: int) -> list[Path]:
    paths = []
    for path in sorted((ROOT / directory).glob("*.md")):
        match = re.match(r"(\d+)[_-]", path.name)
        if match and low <= int(match.group(1)) <= high:
            paths.append(path)
    return paths


def check_gate_citation_hygiene() -> dict[str, Any]:
    # All non-archived numbered documents: Rev-2 files may describe the
    # Rev-2 scheme (docs/26) but may not carry the retired suffixed gate
    # names anywhere (plan §22 translation sweep).
    paths = (
        _numbered_markdown("docs", 0, 999)
        + _numbered_markdown("adr", 0, 999)
        + _numbered_markdown("rfcs", 0, 999)
    )
    violations: list[str] = []
    for path in paths:
        rel = path.relative_to(ROOT)
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
            for token in GATE_SUFFIX_RE.findall(line):
                violations.append(f"{rel}:{number}: retired gate citation {token}: {line.strip()}")
    if violations:
        raise AssertionError("suffixed gate citations:\n" + "\n".join(violations))
    return {"files": len(paths)}


def check_start_here_gate_annotations() -> dict[str, Any]:
    text = (ROOT / "notes/START_HERE_IMPLEMENTATION.md").read_text(encoding="utf-8")
    headings = PR_HEADING_RE.findall(text)
    missing = [
        f"PR {match.group(1)}: {match.group(0).strip()}"
        for match in PR_HEADING_RE.finditer(text)
        if not PR_GATE_ANNOTATION_RE.search(match.group(0))
    ]
    assert headings, "no '### PR <n>' headings found in notes/START_HERE_IMPLEMENTATION.md"
    if missing:
        raise AssertionError("PR headings missing [G<n>] gate annotation:\n" + "\n".join(missing))
    return {"pr_headings": len(headings)}


def _collect_pattern_values(node: Any) -> list[str]:
    patterns: list[str] = []
    if isinstance(node, dict):
        for key, value in node.items():
            if key == "pattern" and isinstance(value, str):
                patterns.append(value)
            else:
                patterns.extend(_collect_pattern_values(value))
    elif isinstance(node, list):
        for item in node:
            patterns.extend(_collect_pattern_values(item))
    return patterns


def check_handle_prefix_registry() -> dict[str, Any]:
    plan_text = (ROOT / "plan.md").read_text(encoding="utf-8")
    section = re.search(r"^### 4\.4 .*?(?=^### )", plan_text, re.MULTILINE | re.DOTALL)
    assert section, "plan.md section 4.4 not found"
    fence = re.search(r"```text\n(.*?)```", section.group(0), re.DOTALL)
    assert fence, "plan.md section 4.4 prefix code block not found"
    registry = set(PREFIX_LINE_RE.findall(fence.group(1)))
    assert registry, "no prefixes parsed from plan.md section 4.4"
    for required in ("cap", "diff"):
        assert required in registry, f"section 4.4 registry missing {required}_*: {sorted(registry)}"

    unknown: list[str] = []
    anchored = 0
    for path in sorted((ROOT / "schemas").glob("*.schema.json")):
        schema = json.loads(path.read_text(encoding="utf-8"))
        for pattern in _collect_pattern_values(schema):
            group = re.match(r"\^\((\w+(?:\|\w+)+)\)_", pattern)
            single = re.match(r"\^([A-Za-z][A-Za-z0-9]*)_", pattern)
            prefixes = group.group(1).split("|") if group else (
                [single.group(1)] if single else []
            )
            for prefix in prefixes:
                anchored += 1
                if prefix not in registry:
                    unknown.append(
                        f"{path.relative_to(ROOT)}: pattern {pattern!r} uses "
                        f"prefix {prefix!r} not in plan section 4.4 registry"
                    )
    if unknown:
        raise AssertionError("unregistered handle prefixes:\n" + "\n".join(unknown))
    return {"registry_prefixes": len(registry), "anchored_patterns": anchored}


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
        ("retired_names", check_retired_names),
        ("gate_scheme_correspondence", check_gate_scheme_correspondence),
        ("gate_citation_hygiene", check_gate_citation_hygiene),
        ("start_here_gate_annotations", check_start_here_gate_annotations),
        ("handle_prefix_registry", check_handle_prefix_registry),
    ]
    report: dict[str, Any] = {"status": "pass", "checks": {}}
    failed: list[str] = []
    for name, fn in checks:
        try:
            report["checks"][name] = fn()
        except Exception as exc:  # noqa: BLE001 — report every check, then fail
            report["status"] = "fail"
            report["checks"][name] = {"error": f"{type(exc).__name__}: {exc}"}
            failed.append(name)
    print(json.dumps(report, indent=2, sort_keys=True))
    if failed:
        sys.exit(1)


if __name__ == "__main__":
    main()
