#!/usr/bin/env python3
"""Mechanical validation for the Continuum architecture/research dossier."""
from __future__ import annotations

import ast
import inspect
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
from traceability import strip_delivered, validate_checked_traceability
import run_config_parity

ROOT = Path(__file__).resolve().parents[1]
PROJECT_ROOT = ROOT.parents[1]

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
    "schemas/intent-registry-record.schema.json": "schemas/examples/intent-registry-record.example.json",
    "schemas/redacted.schema.json": "schemas/examples/redacted.example.json",
    "schemas/promotion-receipt.schema.json": "schemas/examples/promotion-receipt.example.json",
    "schemas/whiteboard-note.schema.json": "schemas/examples/whiteboard-note.example.json",
    "schemas/run-config.schema.json": "schemas/examples/replicated-register.run-config.json",
}

# Instances that live outside the dossier but are still governed by a dossier
# schema. Paths are relative to the repository root, not to `notes/plan/`.
# The `continuum-intent` fixtures are the contracts the Rust suite parses, and
# INV-003 makes `schemas/` — not the parser — normative for their shape, so
# their conformance belongs in this gate rather than in a hand-run command.
#
# The `continuum-evidence` whiteboard fixtures are the other direction of the
# same rule: they are the records the plan §11.5 whiteboard compiler *emits*,
# and `tests/whiteboard_compiler.rs` asserts the library renders exactly these
# bytes. Validating them here is what makes "the compiler emits schema-conforming
# proposals" a checked claim rather than a Rust assertion about itself; the note
# the same suite compiles is `schemas/examples/whiteboard-note.example.json`,
# already validated above.
EXTERNAL_SCHEMA_PAIRS = {
    "schemas/intent-contract.schema.json": (
        "crates/continuum-intent/tests/fixtures/die-hard-contract.json",
        "crates/continuum-intent/tests/fixtures/replicated-register-contract.json",
    ),
    "schemas/evidence-graph-node.schema.json": (
        "crates/continuum-evidence/tests/fixtures/whiteboard/node-goal.json",
        "crates/continuum-evidence/tests/fixtures/whiteboard/node-known-fact.json",
        "crates/continuum-evidence/tests/fixtures/whiteboard/node-candidate-invariant.json",
        "crates/continuum-evidence/tests/fixtures/whiteboard/node-counterexample.json",
        "crates/continuum-evidence/tests/fixtures/whiteboard/node-unresolved-obligation.json",
        "crates/continuum-evidence/tests/fixtures/whiteboard/node-decision.json",
    ),
    "schemas/evidence-graph-edge.schema.json": (
        "crates/continuum-evidence/tests/fixtures/whiteboard/edge-supports-crash.json",
        "crates/continuum-evidence/tests/fixtures/whiteboard/edge-supports-model.json",
    ),
    # The run configurations `continuum-cml-elab`'s configured-lowering suite reads
    # (bn-3a9sr): the schema, not the Rust reader, is normative for their shape.
    "schemas/run-config.schema.json": (
        "crates/continuum-cml-elab/tests/configs/ring.run-config.json",
    ),
    # The transaction versions `continuum-repair`'s PR-20 / IMPL-01 suite emits
    # (bn-2d70): `repair.begin`'s draft and `repair.apply`'s applied version.
    # `tests/pr20_impl01_hypothesis_evidence.rs` asserts the library renders
    # exactly these bytes; this gate checks them against the schema.
    "schemas/repair-transaction.schema.json": (
        "crates/continuum-repair/tests/fixtures/pr20-impl01-ack-after-sync-draft.json",
        "crates/continuum-repair/tests/fixtures/pr20-impl01-ack-after-sync-applied.json",
        # PR-20 / IMPL-03 (bn-2pla): the versions exact replay records on M01's
        # crashpack, gates 1 and 4 with evidence: the ack-after-sync repair (both
        # passed), a repair that does not fix it (gate 4 failed) and one the recorded
        # choices do not determine (gate 4 inconclusive).
        # `crates/continuum-asupersync/tests/pr20_impl03_exact_replay.rs` asserts the
        # library renders exactly these bytes.
        "crates/continuum-asupersync/tests/golden/pr20-impl03-m01-ack-after-sync-evaluated.json",
        "crates/continuum-asupersync/tests/golden/pr20-impl03-m01-no-fix-evaluated.json",
        "crates/continuum-asupersync/tests/golden/pr20-impl03-m01-double-ack-evaluated.json",
    ),
}

# Receipt fragments `continuum-repair`'s PR-22 suites emit (bn-1ebx, bn-cps6, bn-1cec;
# bn-9r5e and bn-yzu1 in `tests/pr22_impl05_impl06_evidence.rs`). A skeleton is not a
# whole receipt: the schema requires fields that other PR-22 bones own, so no fragment
# is checked against a derived schema. Each is checked against the schema's own
# subschemas, with the schema's `$defs` in scope:
#
# - `properties`: the fragment is an object, every key is a declared top-level property,
#   and each value validates against that property's subschema;
# - `gates.items`: the fragment is an array, each entry validates against the subschema
#   of one `gates[]` entry, and each is `not_yet_enforced` (the NotYetEnforced list).
#
# `tests/pr22_receipt_skeleton_evidence.rs` asserts the library renders exactly these
# bytes.
EXTERNAL_SCHEMA_FRAGMENTS = {
    "schemas/promotion-receipt.schema.json": (
        ("crates/continuum-repair/tests/fixtures/pr22-impl01-ack-after-sync-intent.json", "properties"),
        ("crates/continuum-repair/tests/fixtures/pr22-impl02-ack-after-sync-snapshots.json", "properties"),
        ("crates/continuum-repair/tests/fixtures/pr22-impl07-ack-after-sync-gate-profile.json", "properties"),
        ("crates/continuum-repair/tests/fixtures/pr22-impl07-phase-b-not-yet-enforced.json", "gates.items"),
        ("crates/continuum-repair/tests/fixtures/pr22-impl05-ack-after-sync-certificate-status.json", "properties"),
        ("crates/continuum-repair/tests/fixtures/pr22-impl06-ack-after-sync-unknowns.json", "properties"),
    ),
}

# The normative protocol artifacts `rule conformance.registry_agreement` binds
# to one another: the IDL declares the operations, plan §10.2 registers them,
# and the RFC 0027 table carries one authority row per registered operation.
PROTOCOL_IDL_REL = "schemas/continuumd-native-protocol.idl"
PROTOCOL_RFC_REL = "rfcs/0027-agent-tool-protocol.md"
IDL_OPERATION_RE = re.compile(
    r"^operation\s+([a-z_][a-z0-9_]*\.[a-z_][a-z0-9_]*)\s*\{(.*?)^\}",
    re.MULTILINE | re.DOTALL,
)
IDL_AUTHORITY_RE = re.compile(r"^\s*authority\s+([A-Za-z][A-Za-z_-]*)\s*;", re.MULTILINE)
PLAN_OPERATION_FENCE_RE = re.compile(
    r"^### 10\.2 .*?^```text\n(.*?)^```", re.MULTILINE | re.DOTALL
)
PLAN_OPERATION_LINE_RE = re.compile(r"^[a-z_][a-z0-9_]*\.")
RFC_REGISTRY_ROW_RE = re.compile(
    r"^\|\s*`([a-z_][a-z0-9_]*\.[a-z_][a-z0-9_]*)`\s*\|\s*([a-z][a-z_-]*)\s*\|",
    re.MULTILINE,
)
RFC_REGISTRY_COUNT_RE = re.compile(r"\*\*(\d+) operations in (\d+) namespaces\*\*")

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
# \d+[a-z]? — lettered insert PRs (4a/15a/25a) must be matched, not skipped.
PR_HEADING_RE = re.compile(r"^### PR (\d+[a-z]?)\b.*$", re.MULTILINE)
PR_GATE_ANNOTATION_RE = re.compile(r"\[G[0-9]")
TARGET_GATE_RE = re.compile(r"^\*\*Target gate")
GATE_SCHEME_QUALIFIER_RE = re.compile(
    r"Rev(?:ision)?[\s-]?[23]\b|docs/26|docs/52|translat", re.IGNORECASE
)
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
    pairs: list[tuple[str, str, Path]] = [
        (schema_rel, instance_rel, ROOT / instance_rel)
        for schema_rel, instance_rel in SCHEMA_PAIRS.items()
    ]
    pairs += [
        (schema_rel, instance_rel, PROJECT_ROOT / instance_rel)
        for schema_rel, instances in EXTERNAL_SCHEMA_PAIRS.items()
        for instance_rel in instances
    ]
    for schema_rel, instance_rel, instance_path in pairs:
        assert instance_path.exists(), (
            f"{instance_rel}: schema instance is registered against {schema_rel} but does "
            "not exist; a moved or deleted instance loses its conformance check silently"
        )
        schema = json.loads((ROOT / schema_rel).read_text(encoding="utf-8"))
        instance = json.loads(instance_path.read_text(encoding="utf-8"))
        Draft202012Validator.check_schema(schema)
        errors = sorted(Draft202012Validator(schema).iter_errors(instance), key=lambda e: list(e.path))
        if errors:
            formatted = "\n".join(f"{instance_rel}:{list(e.path)}: {e.message}" for e in errors)
            raise AssertionError(formatted)
    fragments = check_schema_fragments()
    return {"pairs": len(pairs), "external_instances": sum(
        len(instances) for instances in EXTERNAL_SCHEMA_PAIRS.values()
    ), "external_fragments": fragments}


def check_schema_fragments() -> int:
    count = 0
    for schema_rel, fragments in EXTERNAL_SCHEMA_FRAGMENTS.items():
        schema = json.loads((ROOT / schema_rel).read_text(encoding="utf-8"))
        Draft202012Validator.check_schema(schema)

        def subschema(body: dict[str, Any]) -> dict[str, Any]:
            return {"$schema": schema["$schema"], "$defs": schema.get("$defs", {}), **body}

        for instance_rel, mode in fragments:
            instance_path = PROJECT_ROOT / instance_rel
            assert instance_path.exists(), (
                f"{instance_rel}: schema fragment is registered against {schema_rel} but "
                "does not exist; a moved or deleted fragment loses its conformance check silently"
            )
            instance = json.loads(instance_path.read_text(encoding="utf-8"))
            checks: list[tuple[list[Any], Any, dict[str, Any]]] = []
            if mode == "properties":
                assert isinstance(instance, dict) and instance, (
                    f"{instance_rel}: a `properties` fragment is a non-empty object"
                )
                for key, value in instance.items():
                    assert key in schema["properties"], (
                        f"{instance_rel}: `{key}` is not a property {schema_rel} declares"
                    )
                    checks.append(([key], value, subschema(schema["properties"][key])))
            elif mode == "gates.items":
                assert isinstance(instance, list) and instance, (
                    f"{instance_rel}: a `gates.items` fragment is a non-empty array"
                )
                items = schema["properties"]["gates"]["items"]
                for index, entry in enumerate(instance):
                    checks.append(([index], entry, subschema(items)))
                    # These fragments are the NotYetEnforced list: an entry listed
                    # `passed` is schema-valid and exactly what the list must never say.
                    assert isinstance(entry, dict) and entry.get("status") == "not_yet_enforced", (
                        f"{instance_rel}:[{index}]: a NotYetEnforced entry is never rendered passed"
                    )
            else:
                raise AssertionError(f"{instance_rel}: unknown fragment mode {mode!r}")
            for path, value, body in checks:
                errors = list(Draft202012Validator(body).iter_errors(value))
                if errors:
                    formatted = "\n".join(
                        f"{instance_rel}:{path + list(e.path)}: {e.message}" for e in errors
                    )
                    raise AssertionError(formatted)
            count += 1
    registered = sum(len(fragments) for fragments in EXTERNAL_SCHEMA_FRAGMENTS.values())
    assert registered == 6 and count == registered, (
        f"{count} of {registered} schema fragments checked; the PR-22 suites register six"
    )
    return count


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

    assertion_count = sum(
        1
        for line in inspect.getsource(check_spikes).splitlines()
        if line.strip().startswith("assert ")
    )
    return {
        "revision_2_runner": output_r2.strip().splitlines()[-1] if output_r2.strip() else "",
        "revision_3_runner": output_r3.strip().splitlines()[-1] if output_r3.strip() else "",
        "assertions": assertion_count,
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
    # Key name is deliberate: this is the number of files *scanned*, not empty.
    return {"scanned_files": sum(
        1 for p in ROOT.rglob("*") if p.is_file() and "__pycache__" not in p.parts
    ), "empty_files": 0}


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
    # Generated renders (e.g. Typst output of the markdown sources) re-wrap
    # lines, which can strip the same-line "renamed from" allowance; the
    # markdown sources themselves are scanned.
    if rel.suffix == ".typ":
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
            # Correspondence is over the normative text: a "(delivered: bn-…)"
            # annotation is a completion record, stripped exactly as the
            # traceability extractors strip it. The strip stops at the first
            # ")", so an annotation with nested parentheses is not supported.
            gates[current].append(strip_delivered(" ".join(bullet)))
        bullet.clear()

    for line in text.splitlines():
        # A delivered annotation on a gate heading ("## G1 — title
        # (delivered: bn-…)") is a completion record, not a heading change.
        heading = GATE_HEADING_RE.match(strip_delivered(line))
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
    missing: list[str] = []
    invalid: list[str] = []
    heading_by_pr: dict[str, str] = {}
    for match in PR_HEADING_RE.finditer(text):
        heading = match.group(0)
        heading_by_pr[match.group(1)] = heading
        if not PR_GATE_ANNOTATION_RE.search(heading):
            missing.append(f"PR {match.group(1)}: {heading.strip()}")
            continue
        bracket = re.search(r"\[(G[^\]]*)\]", heading)
        gates = re.findall(r"G(\d+)", bracket.group(1)) if bracket else []
        if not gates or any(int(g) > 10 for g in gates):
            invalid.append(f"PR {match.group(1)}: gate numbers out of range G0-G10: {heading.strip()}")
    assert headings, "no '### PR <n>' headings found in notes/START_HERE_IMPLEMENTATION.md"
    for expected in ("4a", "15a", "25a", "15b", "22a", "26a", "27a", "27b"):
        assert expected in headings, f"lettered PR heading {expected} not matched"
    # Every open G0 item whose Decision names a closing PR must have that PR
    # annotated [G0 ...] so the freeze-blocking linkage is visible in the plan.
    matrix_text = (ROOT / "notes/G0_SPIKE_MATRIX.md").read_text(encoding="utf-8")
    for line in matrix_text.splitlines():
        row = re.match(r"^\|\s*G0-(DX-\d+)\s*\|", line)
        if not row:
            continue
        cells = [c.strip() for c in line.split("|")]
        status, decision = cells[6], cells[8]
        if not status.startswith("Open"):
            continue
        pr_ref = re.search(r"pending PR (\d+[a-z]?)", decision)
        if not pr_ref:
            continue
        heading = heading_by_pr.get(pr_ref.group(1), "")
        if "G0" not in heading:
            invalid.append(
                f"{row.group(1)}: closing PR {pr_ref.group(1)} heading lacks a G0 "
                f"annotation: {heading.strip() or '(missing heading)'}"
            )
    if missing or invalid:
        raise AssertionError(
            "PR heading gate-annotation violations:\n" + "\n".join(missing + invalid)
        )
    return {"pr_headings": len(headings)}


def check_program_status() -> dict[str, Any]:
    """Validate the section 21.1 swarm-execution posture and phase map."""
    text = (ROOT / "notes/START_HERE_IMPLEMENTATION.md").read_text(encoding="utf-8")
    execution = re.search(
        r"^## Swarm execution map .*?(?=^## |\Z)",
        text,
        re.MULTILINE | re.DOTALL,
    )
    assert execution, "START_HERE lacks the section 21.1 swarm execution map"
    execution_text = execution.group(0)
    assert "autonomous agents" in execution_text
    assert "Bones" in execution_text
    assert "named owner" in execution_text
    for retired in ("Minimum viable team", "unassigned", "owner/team row"):
        assert retired not in execution_text, (
            f"retired staffing gate remains in swarm execution map: {retired}"
        )

    phases_seen: set[str] = set()
    for line in execution_text.splitlines():
        match = re.match(r"^\|\s*(?:Phase\s+)?([A-F])\s*\|(.+)\|\s*$", line)
        if not match:
            continue
        phases_seen.add(match.group(1))
    assert phases_seen == set("ABCDEF"), (
        f"swarm execution map must cover phases A-F, found {sorted(phases_seen)}"
    )

    # Plan section 0.3 must state the same status (its Program status bullet).
    plan_text = (ROOT / "plan.md").read_text(encoding="utf-8")
    section = re.search(r"^## 0\.3 .*?(?=^## |\Z)", plan_text, re.MULTILINE | re.DOTALL)
    assert section, "plan.md section 0.3 not found"
    norm = re.sub(r"\s+", " ", section.group(0))
    assert "Program status:" in norm, "plan section 0.3 lacks a Program status bullet"
    assert "`READY`" in section.group(0), (
        "swarm execution map exists but plan section 0.3 does not say READY"
    )
    return {
        "program_status": "ready",
        "execution_model": "autonomous-agent-swarm",
        "mapped_phases": sorted(phases_seen),
    }


def _deep_get(node: Any, *path: str) -> Any:
    for key in path:
        if not isinstance(node, dict) or key not in node:
            return None
        node = node[key]
    return node


def _schema_defs_named(schema: dict[str, Any], needle: str) -> list[Any]:
    defs = schema.get("$defs", {}) or {}
    return [value for key, value in defs.items() if needle in key.lower()]


def _strip_comments(node: Any) -> Any:
    if isinstance(node, dict):
        return {k: _strip_comments(v) for k, v in node.items() if k != "$comment"}
    if isinstance(node, list):
        return [_strip_comments(v) for v in node]
    return node


def _authority_token(value: str) -> str:
    """Normalize an authority token to its comparable form.

    RFC 0027 spells one level `revise-intent` where the IDL spells it
    `revise_intent`, and the RFC says so explicitly ("allowing for the
    `revise-intent`/`revise_intent` spelling of one token"). That single
    licensed difference is the only one folded away here; every other
    disagreement is a disagreement.
    """
    return value.replace("-", "_")


def _idl_operations() -> dict[str, str]:
    """Every operation the normative IDL declares, mapped to its authority token."""
    text = (ROOT / PROTOCOL_IDL_REL).read_text(encoding="utf-8")
    operations: dict[str, str] = {}
    for match in IDL_OPERATION_RE.finditer(text):
        name, body = match.group(1), match.group(2)
        assert name not in operations, f"{PROTOCOL_IDL_REL}: operation {name} declared twice"
        authority = IDL_AUTHORITY_RE.search(body)
        assert authority, f"{PROTOCOL_IDL_REL}: operation {name} declares no authority clause"
        operations[name] = authority.group(1)
    assert operations, f"{PROTOCOL_IDL_REL}: no operation declarations parsed"
    return operations


def _plan_operations() -> list[str]:
    """The plan §10.2 registry, expanded from its `namespace.verb / verb` shorthand.

    A registry entry may wrap across lines; a continuation is any line inside
    the fence that does not itself open a `namespace.` group.
    """
    plan_text = (ROOT / "plan.md").read_text(encoding="utf-8")
    fence = PLAN_OPERATION_FENCE_RE.search(plan_text)
    assert fence, "plan.md section 10.2 operation registry fence not found"
    entries: list[str] = []
    for raw in fence.group(1).splitlines():
        line = raw.strip()
        if not line:
            continue
        if PLAN_OPERATION_LINE_RE.match(line) or not entries:
            entries.append(line)
        else:
            entries[-1] = f"{entries[-1]} {line}"
    operations: list[str] = []
    for entry in entries:
        namespace, separator, verbs = entry.partition(".")
        assert separator, f"plan.md section 10.2: registry entry {entry!r} names no namespace"
        for verb in verbs.split("/"):
            verb = verb.strip()
            assert verb, f"plan.md section 10.2: registry entry {entry!r} has an empty verb"
            operations.append(f"{namespace.strip()}.{verb}")
    assert operations, "plan.md section 10.2: no operations parsed"
    return operations


def _rfc_operations() -> dict[str, str]:
    """The RFC 0027 authority registry: one row per operation, one level per row."""
    text = (ROOT / PROTOCOL_RFC_REL).read_text(encoding="utf-8")
    rows: dict[str, str] = {}
    for name, authority in RFC_REGISTRY_ROW_RE.findall(text):
        assert name not in rows, f"{PROTOCOL_RFC_REL}: registry lists {name} twice"
        rows[name] = authority
    assert rows, f"{PROTOCOL_RFC_REL}: no registry rows parsed"
    return rows


def check_protocol_registry_agreement() -> dict[str, Any]:
    """`rule conformance.registry_agreement`, enforced rather than asserted.

    The IDL states that its operation set MUST equal the plan §10.2 registry
    exactly, that each operation's `authority` clause MUST equal the RFC 0027
    registry row for the same operation, and that "a generator or validator
    MUST fail closed on any disagreement rather than preferring either
    source". Three independent parses and no precedence between them: every
    disagreement is reported by name, in both directions, and the count RFC
    0027 states in prose must be the count its own table carries.
    """
    idl = _idl_operations()
    plan_list = _plan_operations()
    rfc = _rfc_operations()

    problems: list[str] = []

    repeated = sorted(name for name, count in Counter(plan_list).items() if count > 1)
    if repeated:
        problems.append("plan §10.2 registers an operation more than once: " + ", ".join(repeated))
    plan = set(plan_list)

    for label, other in (
        ("plan.md §10.2", plan),
        (f"{PROTOCOL_RFC_REL} authority registry", set(rfc)),
    ):
        absent = sorted(set(idl) - other)
        unknown = sorted(other - set(idl))
        if absent:
            problems.append(
                f"declared in {PROTOCOL_IDL_REL} but absent from {label}: " + ", ".join(absent)
            )
        if unknown:
            problems.append(
                f"listed in {label} but not declared in {PROTOCOL_IDL_REL}: " + ", ".join(unknown)
            )

    for name in sorted(set(idl) & set(rfc)):
        if _authority_token(idl[name]) != _authority_token(rfc[name]):
            problems.append(
                f"{name}: authority {idl[name]!r} in {PROTOCOL_IDL_REL}, "
                f"{rfc[name]!r} in {PROTOCOL_RFC_REL}"
            )

    namespaces = {name.split(".", 1)[0] for name in idl}
    stated = RFC_REGISTRY_COUNT_RE.search((ROOT / PROTOCOL_RFC_REL).read_text(encoding="utf-8"))
    if not stated:
        problems.append(
            f"{PROTOCOL_RFC_REL}: no '**N operations in M namespaces**' statement to check "
            "the derived registry against"
        )
    else:
        if int(stated.group(1)) != len(idl):
            problems.append(
                f"{PROTOCOL_RFC_REL} says {stated.group(1)} operations, "
                f"{PROTOCOL_IDL_REL} declares {len(idl)}"
            )
        if int(stated.group(2)) != len(namespaces):
            problems.append(
                f"{PROTOCOL_RFC_REL} says {stated.group(2)} namespaces, "
                f"{PROTOCOL_IDL_REL} declares {len(namespaces)}: {sorted(namespaces)}"
            )

    if problems:
        raise AssertionError(
            "protocol registry disagreement (IDL `rule conformance.registry_agreement`; "
            "no source is preferred — fix whichever artifact moved alone):\n"
            + "\n".join(f"  {problem}" for problem in problems)
        )
    return {
        "operations": len(idl),
        "namespaces": len(namespaces),
        "authority_levels": dict(sorted(Counter(idl.values()).items())),
        "sources": [PROTOCOL_IDL_REL, "plan.md §10.2", PROTOCOL_RFC_REL],
    }


def check_spec_debt() -> dict[str, Any]:
    """Derive the section 25 specification-debt ledger; plan text must agree."""

    def text_of(rel: str) -> str:
        return (ROOT / rel).read_text(encoding="utf-8")

    def schema_of(rel: str) -> dict[str, Any]:
        return json.loads(text_of(rel))

    def sd01() -> bool:
        """SD-01 is paid when the protocol IDL exists *and* its registry agrees.

        A glob for a file named `*idl*` only ever proved that something had
        been written down. The debt §25 records is the drift the IDL's own
        `rule conformance.registry_agreement` forbids — between the IDL, the
        plan §10.2 registry, and the RFC 0027 authority table — so the
        predicate is that agreement. `check_protocol_registry_agreement` runs
        as a check of its own and names every disagreeing operation when it
        fails; §25 needs only the boolean, and an unevaluable predicate is
        open debt by the loop below.
        """
        if not (ROOT / PROTOCOL_IDL_REL).exists():
            return False
        check_protocol_registry_agreement()
        return True

    def sd02() -> bool:
        t = text_of("rfcs/0026-continuumd-native-protocol.md")
        return (
            re.search(r"N[−-]1", t) is not None
            and "task.update_budget" in t
            and "evidence.subscribe" in t
            and re.search(r"readab", t, re.IGNORECASE) is not None
        )

    def sd03() -> bool:
        t = text_of("rfcs/0030-incremental-semantic-query-engine.md")
        return re.search(r"auditab", t, re.IGNORECASE) is not None and "bootstrap" in t.lower()

    def sd04() -> bool:
        t = text_of("rfcs/0032-repair-transaction-protocol.md")
        return "BudgetExhausted" in t and "ceiling" in t.lower()

    def sd05() -> bool:
        t = text_of("rfcs/0037-intent-contract.md")
        return "inb_" in t and "three-way" in t.lower()

    def sd06() -> bool:
        t = text_of("rfcs/0038-multi-agent-evidence-graph.md")
        return "append-only" in t.lower() and "compare-and-set" in t.lower()

    def sd07() -> bool:
        schema = schema_of("schemas/intent-contract.schema.json")
        expr = _deep_get(
            schema, "properties", "claims", "items", "properties", "expression"
        )
        if expr is None:
            expr = _deep_get(
                schema, "properties", "properties", "items", "properties", "expression"
            )
        return isinstance(expr, dict) and expr.get("type") not in (None, "string")

    def sd08() -> bool:
        """One identity/versioning convention across every schema (schemas/README.md).

        Derived, never asserted: each document's `$id` is recomputed from its file
        name and declared `schema_epoch`, every artifact schema must require the
        `schema_id`/`schema_epoch` instance header with the consts the identity
        rules imply, and no retired version field may survive.
        """
        retired = ("format", "version", "schema_version", "encoding_version")
        for path in sorted((ROOT / "schemas").glob("*.schema.json")):
            schema = schema_of(f"schemas/{path.name}")
            name = path.name[: -len(".schema.json")]
            epoch = schema.get("schema_epoch")
            kind = schema.get("schema_kind")
            if not isinstance(epoch, int) or isinstance(epoch, bool) or epoch < 1:
                return False
            if kind not in ("artifact", "value"):
                return False
            if schema.get("$id") != f"https://continuum.dev/schema/v{epoch}/{name}.json":
                return False
            properties = schema.get("properties") or {}
            required = set(schema.get("required") or [])
            for field in retired:
                # `repair-transaction.version` is transaction lineage (RFC 0032),
                # not schema identity; every other retired spelling is gone.
                if field in properties and (name, field) != ("repair-transaction", "version"):
                    return False
            if kind == "value":
                # Embedded values inherit their enclosing artifact's header.
                if {"schema_id", "schema_epoch"} & (set(properties) | required):
                    return False
                continue
            class_id = f"https://continuum.dev/schema/{name}.json"
            if properties.get("schema_id", {}).get("const") != class_id:
                return False
            if properties.get("schema_epoch", {}).get("const") != epoch:
                return False
            if not {"schema_id", "schema_epoch"} <= required:
                return False
        return True

    def sd09() -> bool:
        return "has not yet absorbed" not in text_of(
            "docs/35_CONTINUUMD_WORKBENCH_DAEMON.md"
        )

    def sd10() -> bool:
        return "<!-- regenerated: rfc-0032 -->" in text_of(
            "docs/41_REPAIR_TRANSACTIONS.md"
        )

    def _find_key(node: Any, key: str) -> list[Any]:
        found: list[Any] = []
        if isinstance(node, dict):
            for k, v in node.items():
                if k == key:
                    found.append(v)
                found.extend(_find_key(v, key))
        elif isinstance(node, list):
            for item in node:
                found.extend(_find_key(item, key))
        return found

    def sd11() -> bool:
        rt_text = text_of("schemas/repair-transaction.schema.json")
        pr_text = text_of("schemas/promotion-receipt.schema.json")
        node_text = text_of("schemas/evidence-graph-node.schema.json")
        edge_text = text_of("schemas/evidence-graph-edge.schema.json")
        protected_defs = [
            v
            for props in _find_key(schema_of("schemas/semantic-diff.schema.json"), "properties")
            if isinstance(props, dict)
            for k, v in props.items()
            if k == "protected" and isinstance(v, dict)
        ]
        return (
            any(v.get("const") is True for v in protected_defs)
            and "not_applicable" not in rt_text
            and '"phase-c"' in rt_text
            and "uniqueItems" in rt_text
            and "uniqueItems" in pr_text
            and "service_identity" in node_text
            and "CHECKED_BY" in edge_text
            and '"checker"' in edge_text
        )

    def sd12() -> bool:
        cp_text = text_of("schemas/context-pack.schema.json")
        dims = (
            "bounds", "faults", "fairness", "values", "schedules",
            "memory_model", "observer", "proof_status", "unknowns",
        )
        return all(f'"{d}"' in cp_text for d in dims) and "engine_error" not in cp_text

    def sd13() -> bool:
        vt_text = text_of("schemas/verification-task.schema.json")
        ws = schema_of("schemas/workspace-snapshot.schema.json")
        epochs = _deep_get(ws, "properties", "epochs", "properties") or {}
        return (
            "non_resumable_reason" in vt_text
            and "proof_environment" in json.dumps(ws)
            and "protocol" not in epochs
        )

    def _stub_core(node: Any) -> Any:
        """Comparable core of a Redacted stub: its properties and required set."""
        if not isinstance(node, dict):
            return None
        core = {
            "properties": _strip_comments(node.get("properties")),
            "required": sorted(node.get("required", [])),
        }
        return core if core["properties"] else None

    def sd14() -> bool:
        canonical_schema = schema_of("schemas/redacted.schema.json")
        canonical = _schema_defs_named(canonical_schema, "redact") or [canonical_schema]
        want = _stub_core(canonical[0])
        if want is None:
            return False
        targets = (
            "schemas/context-pack.schema.json",
            "schemas/crashpack.schema.json",
            "schemas/proof-receipt.schema.json",
            "schemas/promotion-receipt.schema.json",
            "schemas/repair-transaction.schema.json",
            "schemas/assurance-result.schema.json",
            "schemas/evidence-graph-node.schema.json",
            "schemas/verification-task.schema.json",
            "schemas/cir.schema.json",
        )
        for rel in targets:
            copies = _schema_defs_named(schema_of(rel), "redact")
            if not copies or _stub_core(copies[0]) != want:
                return False
        return True

    predicates = {
        "SD-01": sd01, "SD-02": sd02, "SD-03": sd03, "SD-04": sd04,
        "SD-05": sd05, "SD-06": sd06, "SD-07": sd07, "SD-08": sd08,
        "SD-09": sd09, "SD-10": sd10, "SD-11": sd11, "SD-12": sd12,
        "SD-13": sd13, "SD-14": sd14,
    }
    paid: list[str] = []
    open_items: list[str] = []
    for item, predicate in predicates.items():
        try:
            (paid if predicate() else open_items).append(item)
        except Exception:  # noqa: BLE001 — an unevaluable predicate is open debt
            open_items.append(item)

    # The section 25 ledger must agree with the derived status.
    plan_text = (ROOT / "plan.md").read_text(encoding="utf-8")
    section = re.search(r"^## 25\..*?(?=^## |\Z)", plan_text, re.MULTILINE | re.DOTALL)
    assert section, "plan.md section 25 not found"
    mismatches: list[str] = []
    for item_id, state in re.findall(r"(SD-\d+) \((open|paid)", section.group(0)):
        derived = "paid" if item_id in paid else "open"
        if state != derived:
            mismatches.append(f"{item_id}: plan says {state}, validator derives {derived}")
    listed = set(re.findall(r"SD-\d+", section.group(0)))
    for item_id in predicates:
        if item_id not in listed:
            mismatches.append(f"{item_id}: has a predicate but is not listed in section 25")
    if mismatches:
        raise AssertionError("spec-debt ledger mismatch:\n" + "\n".join(mismatches))
    return {"open": sorted(open_items), "paid": sorted(paid)}


def check_g0_matrix_counts() -> dict[str, Any]:
    """Plan section 0.3's G0 counts must derive from the matrix, not sit beside it."""
    matrix_text = (ROOT / "notes/G0_SPIKE_MATRIX.md").read_text(encoding="utf-8")
    statuses: dict[str, str] = {}
    for line in matrix_text.splitlines():
        match = re.match(r"^\|\s*G0-(DX-\d+)\s*\|", line)
        if not match:
            continue
        cells = [c.strip() for c in line.split("|")]
        # Columns: '' ID Question Experiment Pass Failure Status Evidence Decision ''
        assert len(cells) >= 9, f"matrix row has too few cells: {line.strip()}"
        statuses[match.group(1)] = cells[6]
    assert len(statuses) == 15, f"expected 15 matrix rows, found {len(statuses)}"
    evidence = {d for d, s in statuses.items() if s.startswith("Evidence")}
    open_blocking = {d for d, s in statuses.items() if s.startswith("Open")}
    rehomed = {d for d, s in statuses.items() if s.startswith("Re-homed")}
    # "Closed" is the terminal state of a freeze-blocking row that ran and did
    # not pass: the experiment is finished and the failure consequence the row
    # names has been carried out and adjudicated, so the row is neither open
    # nor carrying evidence of a pass (G0-DX-10, bn-762i).
    closed = {d for d, s in statuses.items() if s.startswith("Closed")}
    classified = evidence | open_blocking | rehomed | closed
    assert classified == set(statuses), (
        f"unclassified statuses: { {d: s for d, s in statuses.items() if d not in classified} }"
    )

    plan_text = (ROOT / "plan.md").read_text(encoding="utf-8")
    section = re.search(r"^## 0\.3 .*?(?=^## |\Z)", plan_text, re.MULTILINE | re.DOTALL)
    assert section, "plan.md section 0.3 not found"
    norm = re.sub(r"\s+", " ", section.group(0))
    evidence_match = re.search(r"G0 status: (.*?) carry spike evidence", norm)
    assert evidence_match, "plan section 0.3 spike-evidence sentence not found"
    plan_evidence = set(re.findall(r"DX-\d+", evidence_match.group(1)))
    assert plan_evidence == evidence, (
        f"plan evidence set {sorted(plan_evidence)} != matrix {sorted(evidence)}"
    )
    # `is`/`are` because the open freeze-blocking set shrinks as G0 items close and
    # English does not let the verb stay plural for one item: G0-DX-14's flip to
    # Evidence (bn-2zy) left DX-10 alone in that sentence. The tolerance is exactly the
    # verb — the item list, the sentence order, and the equality against the matrix are
    # untouched, so this loosens the grammar and enforces the same fact.
    open_match = re.search(r"carry spike evidence\. (.*?) (?:are|is) open and freeze-blocking", norm)
    assert open_match, "plan section 0.3 open/freeze-blocking sentence not found"
    plan_open = set(re.findall(r"DX-\d+", open_match.group(1)))
    assert plan_open == open_blocking, f"plan open set {sorted(plan_open)} != matrix {sorted(open_blocking)}"
    rehomed_match = re.search(r"freeze-blocking \(Phase A\)\. (.*?) are re-homed", norm)
    assert rehomed_match, "plan section 0.3 re-homed sentence not found"
    plan_rehomed = set(re.findall(r"DX-\d+", rehomed_match.group(1)))
    assert plan_rehomed == rehomed, f"plan re-homed set {sorted(plan_rehomed)} != matrix {sorted(rehomed)}"
    # The closed set is reconciled the same way, and in both directions: the
    # sentence is required only when the matrix has a closed row, and a row
    # named closed in the plan but not in the matrix fails too. `[^.]*?` keeps
    # the item list inside its own sentence.
    closed_match = re.search(r"([^.]*?) (?:are|is) closed as failed", norm)
    plan_closed = (
        set(re.findall(r"DX-\d+", closed_match.group(1))) if closed_match else set()
    )
    assert plan_closed == closed, f"plan closed set {sorted(plan_closed)} != matrix {sorted(closed)}"

    # The freeze-blocking subset line must be identical across the three sources.
    subset = "DX-01–03, 10, 12, 13, 14"
    for rel in ("plan.md", "docs/52_RELEASE_GATES_REV3.md", "notes/G0_SPIKE_MATRIX.md"):
        assert subset in (ROOT / rel).read_text(encoding="utf-8"), (
            f"freeze-blocking subset string missing from {rel}"
        )
    return {
        "evidence": len(evidence),
        "open_freeze_blocking": len(open_blocking),
        "rehomed": len(rehomed),
        "closed": len(closed),
    }


def _phase_gate_table(text: str) -> dict[str, frozenset[str]]:
    rows: dict[str, frozenset[str]] = {}
    for line in text.splitlines():
        match = re.match(r"^\|\s*(?:Phase\s+)?([A-F])\s*\|(.+)\|\s*$", line)
        if match:
            gates = frozenset(re.findall(r"G\d+", match.group(2)))
            if gates:
                rows[match.group(1)] = gates
    return rows


def check_phase_gate_tables() -> dict[str, Any]:
    """The phase-to-gate tables in plan section 22 and docs/52 must agree."""
    plan_text = (ROOT / "plan.md").read_text(encoding="utf-8")
    match = re.search(r"^## 22\..*?(?=^## \d)", plan_text, re.MULTILINE | re.DOTALL)
    assert match, "plan.md section 22 not found"
    # Delivered annotations are completion records; the tables compare the
    # normative text only (nested parentheses inside an annotation are not
    # supported).
    plan_table = _phase_gate_table(strip_delivered(match.group(0)))
    docs_table = _phase_gate_table(
        strip_delivered(
            (ROOT / "docs/52_RELEASE_GATES_REV3.md").read_text(encoding="utf-8")
        )
    )
    assert set(plan_table) == set("ABCDEF"), f"plan phase table rows: {sorted(plan_table)}"
    assert set(docs_table) == set("ABCDEF"), f"docs/52 phase table rows: {sorted(docs_table)}"
    mismatches = [
        f"phase {phase}: plan {sorted(plan_table[phase])} != docs/52 {sorted(docs_table[phase])}"
        for phase in sorted(plan_table)
        if plan_table[phase] != docs_table[phase]
    ]
    if mismatches:
        raise AssertionError("phase/gate table mismatch:\n" + "\n".join(mismatches))
    return {"phases": len(plan_table)}


def check_g10_two_system() -> dict[str, Any]:
    """Plan section 21, section 22, and docs/52 must agree on the G10 criterion."""
    needle = "at least two migrated projects stop requiring a separate TLA+ workflow"
    legacy = "at least one migrated project stops requiring"
    for rel in ("plan.md", "docs/52_RELEASE_GATES_REV3.md"):
        # Compare the normative text: a delivered annotation on a plan §21
        # Exit line or a docs/52 heading is a completion record (nested
        # parentheses inside an annotation are not supported).
        norm = re.sub(
            r"\s+", " ", strip_delivered((ROOT / rel).read_text(encoding="utf-8"))
        )
        assert needle in norm, f"{rel}: two-system G10 criterion missing"
        assert legacy not in norm, f"{rel}: superseded one-system G10 criterion still present"
    return {"criterion": "two-system"}


def check_rev2_target_gate_qualifiers() -> dict[str, Any]:
    """Rev-2 RFC 'Target gate' metadata must carry a gate-scheme qualifier."""
    violations: list[str] = []
    checked = 0
    for path in _numbered_markdown("rfcs", 1, 25):
        lines = path.read_text(encoding="utf-8").splitlines()
        for index, line in enumerate(lines):
            if not TARGET_GATE_RE.match(line):
                continue
            checked += 1
            window = " ".join(lines[index : index + 4])
            if not GATE_SCHEME_QUALIFIER_RE.search(window):
                violations.append(
                    f"{path.relative_to(ROOT)}:{index + 1}: bare gate citation without "
                    f"scheme qualifier: {line.strip()}"
                )
    if violations:
        raise AssertionError("untranslated Rev-2 gate metadata:\n" + "\n".join(violations))
    return {"target_gate_lines": checked}


def check_register_rows() -> dict[str, Any]:
    """Every section 24.5 register row names a resolvable lane and a kill/defer/draft.

    Ratified rows carry `quote-id=<slug>` markers in both the register cell and
    the owning research note; marked quotes are compared for identity.
    """
    plan_text = (ROOT / "plan.md").read_text(encoding="utf-8")
    section = re.search(r"^## 24\.5 .*?(?=^## |\Z)", plan_text, re.MULTILINE | re.DOTALL)
    assert section, "plan.md section 24.5 not found"
    rows = 0
    problems: list[str] = []
    for line in section.group(0).splitlines():
        if not line.startswith("| ") or line.startswith("| Capability") or line.startswith("|--"):
            continue
        cells = [c.strip() for c in line.split("|")]
        if len(cells) < 6:
            continue
        rows += 1
        capability, lane, threshold = cells[1], cells[2], cells[3]
        if not any(k in threshold.lower() for k in ("kill", "defer", "draft", "fallback")):
            problems.append(f"{capability}: threshold cell has no kill/defer/draft marker")
        for number in re.findall(r"research/(\d+)", lane):
            if not list((ROOT / "research").glob(f"{int(number):02d}-*.md")):
                problems.append(f"{capability}: research/{number} does not resolve")
        for adr in re.findall(r"ADR-(\d{4})", lane):
            if not list((ROOT / "adr").glob(f"{adr}-*.md")):
                problems.append(f"{capability}: ADR-{adr} does not resolve")
        if "docs/31" in lane and not list((ROOT / "docs").glob("31_*.md")):
            problems.append(f"{capability}: docs/31 does not resolve")
    assert rows >= 13, f"expected at least 13 register rows, found {rows}"

    # Marked-quote identity: quote-id=<slug> "<text>" must match between the
    # register and exactly one research note. Zero markers = nothing ratified.
    marker_re = re.compile(r'quote-id=([\w-]+)\s+"([^"]+)"')
    register_quotes = dict(marker_re.findall(section.group(0)))
    note_quotes: dict[str, str] = {}
    for path in sorted((ROOT / "research").glob("*.md")):
        for slug, quote in marker_re.findall(path.read_text(encoding="utf-8")):
            note_quotes[slug] = quote
    for slug, quote in register_quotes.items():
        if slug not in note_quotes:
            problems.append(f"quote-id={slug}: marked in the register, absent from research/")
        elif re.sub(r"\s+", " ", note_quotes[slug]) != re.sub(r"\s+", " ", quote):
            problems.append(f"quote-id={slug}: register and note quotes differ")
    if problems:
        raise AssertionError("register row violations:\n" + "\n".join(problems))
    return {"rows": rows, "ratified_quotes_checked": len(register_quotes)}


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
        # The run-config reader/schema differential's schema half (bn-3a9sr): the
        # committed parity corpus must be exactly what the schema decides today; the
        # Rust reader is held to the same verdicts by continuum-cml-elab's test suite.
        ("run_config_parity", run_config_parity.check),
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
        ("g0_matrix_counts", check_g0_matrix_counts),
        ("phase_gate_tables", check_phase_gate_tables),
        ("g10_two_system", check_g10_two_system),
        ("rev2_target_gate_qualifiers", check_rev2_target_gate_qualifiers),
        ("register_rows", check_register_rows),
        ("program_status", check_program_status),
        ("protocol_registry_agreement", check_protocol_registry_agreement),
        ("spec_debt", check_spec_debt),
        ("plan_bones_traceability", validate_checked_traceability),
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
    status_check = report["checks"].get("program_status", {})
    report["program_status"] = status_check.get("program_status", "unknown")
    print(json.dumps(report, indent=2, sort_keys=True))
    if failed:
        sys.exit(1)


if __name__ == "__main__":
    main()
