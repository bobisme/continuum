#!/usr/bin/env python3
"""Executable plan-to-Bones traceability for the Continuum program.

The source documents remain authoritative.  This module extracts their
stable, implementation-relevant obligations into a generated registry and
projects the append-only Bones event log so coverage can be checked without
depending on a local Bones database.
"""
from __future__ import annotations

import csv
import json
import re
from collections import Counter, defaultdict, deque
from pathlib import Path
from typing import Any, Iterable

ROOT = Path(__file__).resolve().parents[1]
PROJECT_ROOT = ROOT.parents[1]
REGISTRY_PATH = ROOT / "notes/PLAN_REQUIREMENTS.json"
REPORT_PATH = ROOT / "notes/PLAN_BONE_TRACEABILITY.md"
ACTIVE = "active"

# These edges are the semantic prerequisites for the initial dispatch frontier.
# Requirement coverage proves that work exists; this contract additionally
# proves that the work is executable in the order it is offered to agents.
INITIAL_READINESS_EDGES: tuple[tuple[str, str], ...] = (
    # PR 1: establish the workspace/crate skeleton before adding its policies
    # and shared semantic types.
    ("bn-147t", "bn-11se"),
    ("bn-147t", "bn-17cw"),
    ("bn-147t", "bn-2b8w"),
    ("bn-147t", "bn-2es0"),
    # PR 4a: pin the Lean environment, check the seed, then extend the theorem
    # ladder.
    ("bn-31mq", "bn-fak5"),
    ("bn-fak5", "bn-3qsa"),
    # Phase A constitutional regressions must wait for the implementation they
    # exercise.
    ("bn-1r9", "bn-34je"),
    ("bn-yx2", "bn-1aqq"),
    ("bn-yx2", "bn-1eqt"),
    ("bn-6tv", "bn-11yx"),
    ("bn-3af", "bn-11yx"),
    ("bn-19u", "bn-jme9"),
    ("bn-2ge", "bn-2eeg"),
    ("bn-8mx", "bn-kh8b"),
    ("bn-yx2", "bn-n9a1"),
    ("bn-19u", "bn-2lq8"),
    ("bn-20s", "bn-2lq8"),
    ("bn-37d", "bn-2z0b"),
    ("bn-6tv", "bn-dw81"),
    ("bn-3af", "bn-dw81"),
    ("bn-20s", "bn-3mjd"),
    ("bn-yx2", "bn-1604"),
    # Phase A abuse cases likewise need their real parser, CAS, evidence, and
    # kernel boundaries before their hostile fixtures can be meaningful.
    ("bn-yx2", "bn-3nfy"),
    ("bn-6tv", "bn-3nfy"),
    ("bn-2y0", "bn-2res"),
    ("bn-2y0", "bn-1ptb"),
    ("bn-20s", "bn-1ptb"),
    ("bn-djj", "bn-nio3"),
    ("bn-6tv", "bn-nio3"),
    # The ACI-vs-CLI kill assay consumes PR 10's benchmark harness.
    ("bn-38i", "bn-3ety"),
)


def _text(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def _clean(value: str) -> str:
    value = re.sub(r"`([^`]*)`", r"\1", value)
    value = re.sub(r"\[([^\]]+)\]\([^)]+\)", r"\1", value)
    value = re.sub(r"[*_]+", "", value)
    return re.sub(r"\s+", " ", value).strip(" \n;:")


def _line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def _blocks(pattern: str, text: str) -> Iterable[tuple[re.Match[str], str]]:
    matches = list(re.finditer(pattern, text, re.MULTILINE))
    for index, match in enumerate(matches):
        end = matches[index + 1].start() if index + 1 < len(matches) else len(text)
        yield match, text[match.end() : end]


def _top_level_bullets(text: str) -> list[str]:
    """Return non-nested Markdown bullet blocks, including continuations."""
    bullets: list[str] = []
    current: list[str] = []
    in_fence = False
    for line in text.splitlines():
        if line.startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        if line.startswith("- "):
            if current:
                bullets.append(_clean(" ".join(current)))
            current = [line[2:]]
        elif current and (line.startswith("  ") or not line.strip()):
            if line.strip() and not re.match(r"\s*[-*]\s", line):
                current.append(line.strip())
        elif current:
            bullets.append(_clean(" ".join(current)))
            current = []
    if current:
        bullets.append(_clean(" ".join(current)))
    return [bullet for bullet in bullets if bullet]


def _requirement(
    req_id: str,
    category: str,
    summary: str,
    path: str,
    line: int,
    *,
    status: str = ACTIVE,
    coverage: str = "leaf",
    parent: str | None = None,
    metadata: dict[str, Any] | None = None,
) -> dict[str, Any]:
    result: dict[str, Any] = {
        "id": req_id.upper(),
        "category": category,
        "summary": _clean(summary),
        "status": status,
        "coverage": coverage,
        "source": {"path": path, "line": line},
    }
    if parent:
        result["parent"] = parent.upper()
    if metadata:
        result["metadata"] = metadata
    return result


def _extract_phases(requirements: list[dict[str, Any]]) -> None:
    path = "plan.md"
    text = _text(path)
    section = re.search(r"^## 21\. .*?(?=^## 22\.)", text, re.MULTILINE | re.DOTALL)
    if not section:
        raise AssertionError("plan.md: implementation program section missing")
    section_text = section.group(0)
    for match, body in _blocks(
        r"^### Phase ([A-F])\s+[—–-]\s+(.+)$", section_text
    ):
        phase, title = match.groups()
        phase_id = f"PHASE-{phase}"
        source_line = _line_number(text, section.start() + match.start())
        requirements.append(
            _requirement(
                phase_id, "phase", title, path, source_line, coverage="any"
            )
        )
        deliver_match = re.search(
            r"\nDeliver:\s*(.*?)(?=\nExit:)", body, re.DOTALL
        )
        if not deliver_match:
            raise AssertionError(f"{phase_id}: Deliver block missing")
        for ordinal, summary in enumerate(
            _top_level_bullets(deliver_match.group(1)), 1
        ):
            requirements.append(
                _requirement(
                    f"{phase_id}-DEL-{ordinal:02d}",
                    "phase-deliverable",
                    summary,
                    path,
                    source_line,
                    parent=phase_id,
                    # Same living-document completion record as PR bullets:
                    # "(delivered: bn-…)" on the Deliver bullet.
                    status="satisfied" if "(delivered:" in summary else ACTIVE,
                )
            )
        exit_match = re.search(r"\nExit:\s*(.*?)(?=\n\n|\Z)", body, re.DOTALL)
        if not exit_match:
            raise AssertionError(f"{phase_id}: Exit block missing")
        requirements.append(
            _requirement(
                f"{phase_id}-EXIT",
                "phase-exit",
                exit_match.group(1),
                path,
                source_line,
                parent=phase_id,
            )
        )


def _normalize_pr_id(raw: str) -> str:
    return f"PR-{raw.upper()}"


def _extract_prs(requirements: list[dict[str, Any]]) -> None:
    path = "notes/START_HERE_IMPLEMENTATION.md"
    text = _text(path)
    pattern = r"^### PR (\d+[a-z]?)\s+[—–-]\s+(.+)$"
    for match, body in _blocks(pattern, text):
        raw_id, title = match.groups()
        pr_id = _normalize_pr_id(raw_id)
        line = _line_number(text, match.start())
        pr_requirement = _requirement(pr_id, "pr", title, path, line, coverage="any")
        requirements.append(pr_requirement)
        before_exit, marker, after_exit = body.partition("**Exit:**")
        for ordinal, summary in enumerate(_top_level_bullets(before_exit), 1):
            requirements.append(
                _requirement(
                    f"{pr_id}-IMPL-{ordinal:02d}",
                    "pr-deliverable",
                    summary,
                    path,
                    line,
                    parent=pr_id,
                    # A "(delivered: bn-…)" annotation on the bullet is the
                    # living-document completion record: the deliverable's
                    # evidence-owning Bone is closed and the label retired.
                    status="satisfied" if "(delivered:" in summary else ACTIVE,
                )
            )
        if marker:
            exit_summary = _clean(after_exit.split("\n\n", 1)[0])
            requirements.append(
                _requirement(
                    f"{pr_id}-EXIT",
                    "pr-exit",
                    exit_summary,
                    path,
                    line,
                    parent=pr_id,
                    # Same living-document completion record as deliverable
                    # bullets: "(delivered: bn-…)" on the Exit line.
                    status="satisfied" if "(delivered:" in exit_summary else ACTIVE,
                )
            )
            if "(delivered:" in exit_summary:
                # A delivered exit completes the PR itself: the exit criterion
                # is the PR's definition of done.
                pr_requirement["status"] = "satisfied"


def _extract_gates(requirements: list[dict[str, Any]]) -> None:
    path = "docs/52_RELEASE_GATES_REV3.md"
    text = _text(path)
    matches = list(re.finditer(r"^## G(\d+)\s+[—–-]\s+(.+)$", text, re.MULTILINE))
    for match in matches:
        gate, title = match.groups()
        next_heading = re.search(r"^## ", text[match.end() :], re.MULTILINE)
        end = match.end() + next_heading.start() if next_heading else len(text)
        body = text[match.end() : end]
        line = _line_number(text, match.start())
        gate_id = f"G{gate}"
        requirements.append(
            _requirement(gate_id, "gate", title, path, line, coverage="any")
        )
        bullets = _top_level_bullets(body)
        if not bullets:
            paragraphs = [
                _clean(paragraph)
                for paragraph in re.split(r"\n\s*\n", body)
                if _clean(paragraph)
            ]
            bullets = [" ".join(paragraphs)]
        for ordinal, summary in enumerate(bullets, 1):
            requirements.append(
                _requirement(
                    f"{gate_id}-{ordinal:02d}",
                    "gate-criterion",
                    summary,
                    path,
                    line,
                    parent=gate_id,
                )
            )


def _extract_g0_matrix(requirements: list[dict[str, Any]]) -> None:
    path = "notes/G0_SPIKE_MATRIX.md"
    text = _text(path)
    for line_number, line in enumerate(text.splitlines(), 1):
        if not re.match(r"^\| G0-DX-\d{2} \|", line):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        req_id, question, experiment, passed, failure, status = cells[:6]
        requirements.append(
            _requirement(
                req_id,
                "experiment",
                question,
                path,
                line_number,
                # The matrix's own Status column is the living-document
                # completion record for an experiment, the way "(delivered:
                # bn-…)" is for a bullet: "Evidence (reference implementation)"
                # means the campaign ran against production code and its
                # retained artifacts are named in the Evidence column, so the
                # requirement no longer needs an active carrier Bone. Spike
                # evidence and re-homed rows stay active — a finite spike is
                # explicitly insufficient, and a re-homed row's obligation
                # lives at its target gate.
                status=(
                    "satisfied"
                    if status.startswith("Evidence (reference implementation)")
                    else ACTIVE
                ),
                metadata={
                    "experiment": experiment,
                    "pass_condition": passed,
                    "failure_consequence": failure,
                    "matrix_status": status,
                },
            )
        )


def _extract_headings(
    requirements: list[dict[str, Any]],
    *,
    path: str,
    pattern: str,
    category: str,
    deferred_prefixes: tuple[str, ...] = (),
) -> None:
    text = _text(path)
    matches = list(re.finditer(pattern, text, re.MULTILINE))
    for match in matches:
        req_id, summary = match.groups()
        heading = re.match(r"^(#{1,6})\s", match.group(0))
        if not heading:
            raise AssertionError(f"{path}: requirement pattern must match a Markdown heading")
        level = len(heading.group(1))
        boundary = re.search(
            rf"^#{{1,{level}}}\s",
            text[match.end() :],
            re.MULTILINE,
        )
        end = match.end() + boundary.start() if boundary else len(text)
        body = text[match.end() : end]
        paragraphs = [
            _clean(paragraph)
            for paragraph in re.split(r"\n\s*\n", body)
            if _clean(paragraph) and not paragraph.lstrip().startswith("#")
        ]
        if any(req_id.startswith(prefix) for prefix in deferred_prefixes):
            status = "deferred"
        elif "(delivered:" in summary:
            status = "satisfied"
            summary = re.sub(r"\s*\(delivered:[^)]*\)", "", summary)
        else:
            status = ACTIVE
        metadata: dict[str, Any] = {}
        if paragraphs:
            metadata["contract"] = paragraphs[0]
        bullets = _top_level_bullets(body)
        if bullets:
            metadata["controls"] = bullets
        requirements.append(
            _requirement(
                req_id,
                category,
                summary,
                path,
                _line_number(text, match.start()),
                status=status,
                metadata=metadata or None,
            )
        )


def _extract_claims(requirements: list[dict[str, Any]]) -> None:
    path = "docs/18_CLAIMS_MATRIX.md"
    text = _text(path)
    for line_number, line in enumerate(text.splitlines(), 1):
        if not re.match(r"^\| C\d{3} \|", line):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        req_id, claim, evidence, initial_state = cells[:4]
        if initial_state.startswith("OBSERVED"):
            status = "satisfied"
        elif req_id == "C030":
            status = "deferred"
        else:
            status = ACTIVE
        requirements.append(
            _requirement(
                req_id,
                "claim",
                claim,
                path,
                line_number,
                status=status,
                metadata={
                    "required_evidence": evidence,
                    "initial_state": initial_state,
                },
            )
        )


def _extract_frontier(requirements: list[dict[str, Any]]) -> None:
    path = "plan.md"
    text = _text(path)
    section = re.search(r"^## 24\.5 .*?(?=^## 25\.)", text, re.MULTILINE | re.DOTALL)
    if not section:
        raise AssertionError("plan.md: frontier register missing")
    ordinal = 0
    for offset, line in enumerate(section.group(0).splitlines()):
        if not line.startswith("| ") or line.startswith("| Capability") or line.startswith("|---"):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) != 4:
            continue
        ordinal += 1
        capability, lane, threshold, fallback = cells
        if capability.startswith(("Full TLA+", "Weak-memory", "Timed/probabilistic")):
            status = "deferred"
        elif "quote-id=" in threshold:
            # A marked quote means the row is ratified. The FR requirement —
            # fix the threshold before the consuming phase opens — is
            # discharged; the lane's promote/kill decision is owned by its
            # gate, not by this register row.
            status = "satisfied"
        else:
            status = ACTIVE
        requirements.append(
            _requirement(
                f"FR-{ordinal:02d}",
                "frontier",
                capability,
                path,
                _line_number(text, section.start()) + offset,
                status=status,
                metadata={"lane": lane, "threshold": threshold, "fallback": fallback},
            )
        )


def _extract_spec_debt(requirements: list[dict[str, Any]]) -> None:
    path = "plan.md"
    text = _text(path)
    pattern = r"^- (SD-\d{2}) \((open|paid[^)]*)\):\s*(.*(?:\n  .*)*)"
    for match in re.finditer(pattern, text, re.MULTILINE):
        req_id, raw_status, summary = match.groups()
        requirements.append(
            _requirement(
                req_id,
                "spec-debt",
                summary,
                path,
                _line_number(text, match.start()),
                status=ACTIVE if raw_status == "open" else "satisfied",
                metadata={"ledger_status": raw_status},
            )
        )


def _extract_corpus(requirements: list[dict[str, Any]]) -> None:
    path = "corpus/tla-examples/validated-examples.csv"
    with (ROOT / path).open(encoding="utf-8", newline="") as handle:
        rows = list(csv.DictReader(handle))
    for line_number, row in enumerate(rows, 2):
        requirements.append(
            _requirement(
                row["id"],
                "corpus-family",
                row["title"],
                path,
                line_number,
                metadata={
                    "wave": int(row["porting_wave"]),
                    "required_parity": row["required_parity"],
                    "runtime_refinement_target": row["runtime_refinement_target"],
                    "p5_exemplar": row["p5_exemplar"],
                    "source_path": row["source_path"],
                    "source_commit": row["source_commit"],
                },
            )
        )


def _extract_bullet_policy(
    requirements: list[dict[str, Any]],
    *,
    path: str,
    category: str,
    prefix: str,
    section_pattern: str = r"^## (\d+)\.\s+(.+)$",
) -> None:
    text = _text(path)
    for match, body in _blocks(section_pattern, text):
        section_id, _title = match.groups()
        line = _line_number(text, match.start())
        for ordinal, summary in enumerate(_top_level_bullets(body), 1):
            status = "satisfied" if "(delivered:" in summary else ACTIVE
            summary = re.sub(r"\s*\(delivered:[^)]*\)", "", summary)
            requirements.append(
                _requirement(
                    f"{prefix}-{section_id}-{ordinal:02d}",
                    category,
                    summary,
                    path,
                    line,
                    status=status,
                )
            )


def _extract_metrics_and_kills(requirements: list[dict[str, Any]]) -> None:
    path = "plan.md"
    text = _text(path)
    metrics = re.search(r"^## 23\. .*?(?=^## 24\.)", text, re.MULTILINE | re.DOTALL)
    if not metrics:
        raise AssertionError("plan.md: success metrics missing")
    for match, body in _blocks(r"^### ([A-Za-z]+)$", metrics.group(0)):
        name = match.group(1).upper()
        line = _line_number(text, metrics.start() + match.start())
        for ordinal, summary in enumerate(_top_level_bullets(body), 1):
            requirements.append(
                _requirement(
                    f"METRIC-{name}-{ordinal:02d}",
                    "success-metric",
                    summary,
                    path,
                    line,
                )
            )
    kills = re.search(r"^## 24\. Kill criteria(.*?)(?=^## 24\.5)", text, re.MULTILINE | re.DOTALL)
    if not kills:
        raise AssertionError("plan.md: kill criteria missing")
    line = _line_number(text, kills.start())
    for ordinal, summary in enumerate(_top_level_bullets(kills.group(1)), 1):
        requirements.append(
            _requirement(
                f"KILL-{ordinal:02d}",
                "kill-criterion",
                summary,
                path,
                line,
            )
        )


def build_registry() -> dict[str, Any]:
    requirements: list[dict[str, Any]] = []
    plan_text = _text("plan.md")
    plan_title = re.search(r"^#\s+(.+)$", plan_text, re.MULTILINE)
    if not plan_title:
        raise AssertionError("plan.md: title missing")
    requirements.append(
        _requirement(
            "PROGRAM",
            "program",
            plan_title.group(1),
            "plan.md",
            _line_number(plan_text, plan_title.start()),
            coverage="any",
        )
    )
    _extract_phases(requirements)
    _extract_prs(requirements)
    _extract_gates(requirements)
    _extract_g0_matrix(requirements)
    _extract_headings(
        requirements,
        path="plan.md",
        pattern=r"^### (INV-\d{3})\s+[—–-]\s+(.+)$",
        category="invariant",
    )
    _extract_headings(
        requirements,
        path="docs/16_PROOF_OBLIGATIONS.md",
        pattern=r"^### (PO-[A-Z]+-\d{3})\s+[—–-]\s+(.+)$",
        category="proof-obligation",
        deferred_prefixes=("PO-TIME-", "PO-PROB-"),
    )
    _extract_claims(requirements)
    _extract_headings(
        requirements,
        path="docs/09_THREAT_MODEL.md",
        pattern=r"^### (T\d{2})\s+[—–-]\s+(.+)$",
        category="threat",
    )
    _extract_headings(
        requirements,
        path="docs/08_RISK_REGISTER.md",
        pattern=r"^## (R\d{2})\s+[—–-]\s+(.+)$",
        category="risk",
    )
    _extract_frontier(requirements)
    _extract_spec_debt(requirements)
    _extract_corpus(requirements)
    _extract_bullet_policy(
        requirements,
        path="docs/19_TEST_STRATEGY.md",
        category="test-policy",
        prefix="TEST",
    )
    _extract_bullet_policy(
        requirements,
        path="docs/12_GOVERNANCE_AND_ENGINEERING.md",
        category="governance-policy",
        prefix="GOV",
    )
    _extract_metrics_and_kills(requirements)

    invariant_spill = [
        item["id"]
        for item in requirements
        if item["category"] == "invariant"
        and item.get("metadata", {}).get("controls")
    ]
    if invariant_spill:
        raise AssertionError(
            "invariant extraction crossed its heading boundary: "
            f"{invariant_spill}"
        )
    threat_control_failures = [
        (item["id"], len(item.get("metadata", {}).get("controls", [])))
        for item in requirements
        if item["category"] == "threat"
        and not 1 <= len(item.get("metadata", {}).get("controls", [])) <= 8
    ]
    if threat_control_failures:
        raise AssertionError(
            "threat control extraction crossed its heading boundary: "
            f"{threat_control_failures}"
        )

    identifiers = [item["id"] for item in requirements]
    duplicates = sorted(key for key, count in Counter(identifiers).items() if count > 1)
    if duplicates:
        raise AssertionError(f"duplicate requirement IDs: {duplicates}")
    return {
        "schema_version": 1,
        "authority": "Generated from normative plan sources; do not edit by hand.",
        "label_contract": "Each active requirement is covered by req:<ID> on an active Bone.",
        "sources": sorted({item["source"]["path"] for item in requirements}),
        "requirements": requirements,
    }


def project_bones() -> dict[str, dict[str, Any]]:
    bones: dict[str, dict[str, Any]] = {}
    event_paths = sorted((PROJECT_ROOT / ".bones/events").glob("*.events"))
    if not event_paths:
        raise AssertionError(f"no Bones events under {PROJECT_ROOT / '.bones/events'}")
    for path in event_paths:
        for line_number, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if not raw or raw.startswith("#"):
                continue
            fields = raw.split("\t")
            if len(fields) < 7:
                raise AssertionError(f"{path}:{line_number}: malformed Bones event")
            event_type, item_id = fields[4], fields[5]
            try:
                data = json.loads(fields[6])
            except json.JSONDecodeError as exc:
                raise AssertionError(f"{path}:{line_number}: invalid event JSON") from exc
            if event_type == "item.create":
                bones[item_id] = {
                    "id": item_id,
                    "title": data.get("title", ""),
                    "description": data.get("description", ""),
                    "kind": data.get("kind", "task"),
                    "parent": data.get("parent"),
                    "size": data.get("size"),
                    "labels": set(data.get("labels", [])),
                    "state": data.get("state", "open"),
                    "deleted": False,
                }
                continue
            if item_id not in bones:
                continue
            bone = bones[item_id]
            if event_type == "item.update":
                field, value = data.get("field"), data.get("value")
                if field == "labels" and isinstance(value, dict):
                    if value.get("action") == "add":
                        bone["labels"].add(value["label"])
                    elif value.get("action") == "remove":
                        bone["labels"].discard(value["label"])
                elif field in {"title", "description", "kind", "parent", "size", "state"}:
                    bone[field] = value
            elif event_type == "item.move":
                if "state" in data:
                    bone["state"] = data["state"]
                if "parent" in data:
                    bone["parent"] = data["parent"]
            elif event_type == "item.delete":
                bone["deleted"] = True
            elif event_type in {"item.close", "item.done"}:
                bone["state"] = "done"
            elif event_type in {"item.reopen", "item.open"}:
                bone["state"] = "open"
    return bones


def project_blocking_links() -> set[tuple[str, str]]:
    """Project current (blocker, blocked) dependency pairs from Bones events."""
    links: set[tuple[str, str]] = set()
    for path in sorted((PROJECT_ROOT / ".bones/events").glob("*.events")):
        for line_number, raw in enumerate(
            path.read_text(encoding="utf-8").splitlines(), 1
        ):
            if not raw or raw.startswith("#"):
                continue
            fields = raw.split("\t")
            if len(fields) < 7:
                raise AssertionError(f"{path}:{line_number}: malformed Bones event")
            event_type, blocked = fields[4], fields[5]
            data = json.loads(fields[6])
            blocker = data.get("target")
            if not blocker:
                continue
            pair = (blocker, blocked)
            if event_type == "item.link" and data.get("link_type") == "blocks":
                links.add(pair)
            elif event_type == "item.unlink":
                links.discard(pair)
    return links


def traceability_state(
    registry: dict[str, Any] | None = None,
    bones: dict[str, dict[str, Any]] | None = None,
) -> dict[str, Any]:
    registry = registry or build_registry()
    bones = bones or project_bones()
    requirements = {item["id"]: item for item in registry["requirements"]}
    active_bones = {
        item_id: item
        for item_id, item in bones.items()
        if not item["deleted"] and item["state"] not in {"done", "closed", "archived"}
    }
    children: dict[str, set[str]] = defaultdict(set)
    for item in active_bones.values():
        if item["parent"] in active_bones:
            children[item["parent"]].add(item["id"])
    leaves = {item_id for item_id in active_bones if not children[item_id]}
    coverage: dict[str, set[str]] = defaultdict(set)
    unknown_labels: list[dict[str, str]] = []
    untraced: list[dict[str, str]] = []
    overbundled: list[dict[str, Any]] = []
    for item_id, bone in active_bones.items():
        req_labels = sorted(
            label[4:].upper() for label in bone["labels"] if label.startswith("req:")
        )
        for req_id in req_labels:
            if req_id not in requirements:
                unknown_labels.append({"bone": item_id, "requirement": req_id})
            else:
                coverage[req_id].add(item_id)
        if not req_labels and "trace:meta" not in bone["labels"]:
            untraced.append({"bone": item_id, "title": bone["title"]})
        if item_id in leaves and len(req_labels) > 8:
            overbundled.append(
                {"bone": item_id, "title": bone["title"], "requirements": req_labels}
            )

    uncovered: list[dict[str, str]] = []
    nonleaf_only: list[dict[str, Any]] = []
    for req_id, requirement in requirements.items():
        if requirement["status"] != ACTIVE:
            continue
        mapped = coverage[req_id]
        if not mapped:
            uncovered.append(
                {
                    "requirement": req_id,
                    "category": requirement["category"],
                    "summary": requirement["summary"],
                }
            )
        elif requirement["coverage"] == "leaf" and not (mapped & leaves):
            nonleaf_only.append(
                {
                    "requirement": req_id,
                    "bones": sorted(mapped),
                    "summary": requirement["summary"],
                }
            )
    return {
        "requirements": requirements,
        "bones": active_bones,
        "leaves": leaves,
        "coverage": coverage,
        "uncovered": uncovered,
        "nonleaf_only": nonleaf_only,
        "untraced": untraced,
        "unknown_labels": unknown_labels,
        "overbundled": overbundled,
    }


def graph_contract_state(
    registry: dict[str, Any],
    state: dict[str, Any],
) -> dict[str, Any]:
    bones = state["bones"]
    leaves = state["leaves"]
    links = {
        (blocker, blocked)
        for blocker, blocked in project_blocking_links()
        if blocker in bones and blocked in bones
    }
    all_bones = project_bones()
    deleted_dependency_links = sorted(
        (blocker, blocked)
        for blocker, blocked in project_blocking_links()
        if (
            blocker in all_bones
            and all_bones[blocker]["deleted"]
            or blocked in all_bones
            and all_bones[blocked]["deleted"]
        )
    )
    children: dict[str, set[str]] = defaultdict(set)
    for item in bones.values():
        if item["parent"] in bones:
            children[item["parent"]].add(item["id"])

    dispatch_readiness_failures: list[str] = []
    completed = {
        item_id
        for item_id, item in project_bones().items()
        if not item["deleted"] and item["state"] in {"done", "closed", "archived"}
    }
    for blocker, blocked in INITIAL_READINESS_EDGES:
        if completed & {blocker, blocked}:
            # A completed endpoint has discharged the ordering constraint;
            # these edges only guard the initial dispatch window.
            continue
        missing = [item_id for item_id in (blocker, blocked) if item_id not in bones]
        if missing:
            dispatch_readiness_failures.append(
                f"{blocker} -> {blocked}: inactive or missing endpoint(s) {missing}"
            )
        elif (blocker, blocked) not in links:
            dispatch_readiness_failures.append(
                f"{blocker} does not block {blocked}"
            )

    risk_routing_failures: list[str] = []
    for item_id, item in bones.items():
        risk_labels = sorted(
            label for label in item["labels"] if label.startswith("risk:")
        )
        if len(risk_labels) > 1:
            risk_routing_failures.append(
                f"{item_id}: multiple risk labels {risk_labels}"
            )
        if (
            {"security", "threat", "invariant"} & item["labels"]
            and not {"risk:high", "risk:critical"} & item["labels"]
        ):
            risk_routing_failures.append(
                f"{item_id}: security-sensitive work lacks risk:high routing"
            )

    # Directed cycle and layer audit.
    indegree = {item_id: 0 for item_id in bones}
    outgoing: dict[str, set[str]] = defaultdict(set)
    for blocker, blocked in links:
        if blocked not in outgoing[blocker]:
            outgoing[blocker].add(blocked)
            indegree[blocked] += 1
    queue = deque(item_id for item_id, degree in indegree.items() if degree == 0)
    depth = {item_id: 0 for item_id in queue}
    visited: list[str] = []
    while queue:
        item_id = queue.popleft()
        visited.append(item_id)
        for dependent in outgoing[item_id]:
            depth[dependent] = max(depth.get(dependent, 0), depth[item_id] + 1)
            indegree[dependent] -= 1
            if indegree[dependent] == 0:
                queue.append(dependent)
    cycle_nodes = sorted(item_id for item_id, degree in indegree.items() if degree)

    plan_keys: dict[str, list[str]] = defaultdict(list)
    for item_id, item in bones.items():
        for label in item["labels"]:
            if label.startswith("plan-key:"):
                plan_keys[label].append(item_id)
    duplicate_plan_keys = {
        key: sorted(item_ids)
        for key, item_ids in plan_keys.items()
        if len(item_ids) > 1
    }
    empty_goals = sorted(
        item_id
        for item_id, item in bones.items()
        if item["kind"] == "goal" and not children[item_id]
    )
    large_leaf_tasks = sorted(
        item_id
        for item_id in leaves
        if bones[item_id]["kind"] == "task" and bones[item_id].get("size") in {"l", "xl"}
    )
    undersized_goals = sorted(
        item_id
        for item_id, item in bones.items()
        if item["kind"] == "goal" and item.get("size") not in {"l", "xl"}
    )

    phase_ids: dict[str, str] = {}
    phase_failures: list[str] = []
    for phase in "ABCDEF":
        matches = [
            item_id
            for item_id, item in bones.items()
            if item["title"].startswith(f"Phase {phase} —")
            and item["kind"] == "goal"
        ]
        if len(matches) != 1:
            phase_failures.append(f"Phase {phase}: expected one goal, found {matches}")
            continue
        phase_ids[phase] = matches[0]
        if "goal:manual" not in bones[matches[0]]["labels"]:
            phase_failures.append(f"Phase {phase}: goal:manual missing")

    def descendants(parent: str) -> set[str]:
        result: set[str] = set()
        pending = list(children[parent])
        while pending:
            item_id = pending.pop()
            if item_id in result:
                continue
            result.add(item_id)
            pending.extend(children[item_id])
        return result

    phase_barrier_failures: list[str] = []
    if len(phase_ids) == 6:
        for previous, current in zip("ABCDE", "BCDEF", strict=True):
            required = phase_ids[previous]
            for leaf in sorted(leaves & descendants(phase_ids[current])):
                if (required, leaf) not in links:
                    phase_barrier_failures.append(
                        f"{leaf} ({current}) lacks predecessor Phase {previous} barrier"
                    )

    phase_exit_failures: list[str] = []
    if len(phase_ids) == 6:
        for phase, phase_id in phase_ids.items():
            matches = [
                item_id
                for item_id, item in bones.items()
                if item["kind"] == "goal"
                and item["parent"] == phase_id
                and f"req:phase-{phase.lower()}-exit" in item["labels"]
            ]
            if len(matches) != 1:
                phase_exit_failures.append(
                    f"Phase {phase}: expected one exit goal, found {matches}"
                )
                continue
            if "goal:manual" not in bones[matches[0]]["labels"]:
                phase_exit_failures.append(
                    f"Phase {phase}: exit goal {matches[0]} lacks goal:manual"
                )

    requirement_map = {item["id"]: item for item in registry["requirements"]}
    pr_exit_failures: list[str] = []
    for requirement in registry["requirements"]:
        if requirement["category"] != "pr":
            continue
        if requirement["status"] != ACTIVE:
            # A completed PR's goal bone is closed; its exit wiring is history.
            continue
        pr_id = requirement["id"]
        parent_matches = [
            item_id
            for item_id, item in bones.items()
            if item["title"].lower().startswith(
                (pr_id.replace("-", " ") + " ").lower()
            )
            and f"req:{pr_id.lower()}" in item["labels"]
        ]
        if len(parent_matches) != 1:
            pr_exit_failures.append(
                f"{pr_id}: expected one PR goal, found {parent_matches}"
            )
            continue
        parent = parent_matches[0]
        pr_leaves = leaves & descendants(parent)
        exit_id = f"{pr_id}-EXIT"
        exit_matches = [
            item_id
            for item_id in pr_leaves
            if f"req:{exit_id.lower()}" in bones[item_id]["labels"]
            and f"plan-key:pr-exit-{pr_id.lower()}" in bones[item_id]["labels"]
        ]
        if len(exit_matches) != 1:
            pr_exit_failures.append(
                f"{pr_id}: expected one dedicated exit task, found {exit_matches}"
            )
            continue
        exit_task = exit_matches[0]
        for leaf in sorted(pr_leaves - {exit_task}):
            if (leaf, exit_task) not in links:
                pr_exit_failures.append(f"{pr_id}: {leaf} does not block {exit_task}")

    gate_failures: list[str] = []
    for requirement in registry["requirements"]:
        if (
            requirement["category"] != "gate-criterion"
            or requirement["status"] != ACTIVE
        ):
            continue
        req_id = requirement["id"]
        gate = requirement["parent"]
        phase = next(
            phase
            for phase, gates in {
                "A": {"G0", "G1", "G2"},
                "B": {"G3", "G4"},
                "C": {"G5"},
                "D": {"G6"},
                "E": {"G7"},
                "F": {"G8", "G9", "G10"},
            }.items()
            if gate in gates
        )
        criterion = [
            item_id
            for item_id in state["coverage"][req_id]
            if f"plan-key:gate-{req_id.lower()}" in bones[item_id]["labels"]
        ]
        integration = [
            item_id
            for item_id, item in bones.items()
            if f"plan-key:phase-exit-{phase.lower()}-integration" in item["labels"]
        ]
        if len(criterion) != 1 or len(integration) != 1:
            gate_failures.append(
                f"{req_id}: criterion={criterion}, integration={integration}"
            )
        elif (integration[0], criterion[0]) not in links:
            gate_failures.append(
                f"{req_id}: {integration[0]} does not block {criterion[0]}"
            )

    risk_checkpoint_failures: list[str] = []
    for item_id, item in bones.items():
        if item["kind"] != "task" or not (
            "risk" in item["labels"] or "kill-criterion" in item["labels"]
        ):
            continue
        phase_labels = sorted(
            label
            for label in item["labels"]
            if re.fullmatch(r"phase-[a-f]", label)
        )
        if len(phase_labels) != 1:
            risk_checkpoint_failures.append(
                f"{item_id}: expected one phase label, found {phase_labels}"
            )
            continue
        phase = phase_labels[0][-1].upper()
        integration = [
            candidate
            for candidate, candidate_item in bones.items()
            if f"plan-key:phase-exit-{phase.lower()}-integration"
            in candidate_item["labels"]
        ]
        if len(integration) != 1:
            risk_checkpoint_failures.append(
                f"{item_id}: Phase {phase} integration={integration}"
            )
        elif (item_id, integration[0]) not in links:
            risk_checkpoint_failures.append(
                f"{item_id}: does not block Phase {phase} integration"
            )
        if phase != "A":
            previous = chr(ord(phase) - 1)
            if previous not in phase_ids or (phase_ids[previous], item_id) not in links:
                risk_checkpoint_failures.append(
                    f"{item_id}: lacks predecessor Phase {previous} barrier"
                )
        for other_phase in "ABCDEF":
            if other_phase == phase:
                continue
            other_integration = [
                candidate
                for candidate, candidate_item in bones.items()
                if f"plan-key:phase-exit-{other_phase.lower()}-integration"
                in candidate_item["labels"]
            ]
            if (
                len(other_integration) == 1
                and (item_id, other_integration[0]) in links
            ):
                risk_checkpoint_failures.append(
                    f"{item_id}: unexpectedly blocks Phase {other_phase} integration"
                )

    frontier_wiring_failures: list[str] = []
    active_frontiers = {
        item["id"]
        for item in registry["requirements"]
        if item["category"] == "frontier" and item["status"] == ACTIVE
    }
    frontier_tasks: dict[str, list[str]] = defaultdict(list)
    for item_id, item in bones.items():
        for label in item["labels"]:
            match = re.fullmatch(r"plan-key:frontier-(fr-\d+)", label)
            if match:
                frontier_tasks[match.group(1).upper()].append(item_id)
    satisfied_frontiers = {
        item["id"]
        for item in registry["requirements"]
        if item["category"] == "frontier" and item["status"] == "satisfied"
    }
    missing_frontiers = active_frontiers - set(frontier_tasks)
    stray_frontiers = set(frontier_tasks) - active_frontiers - satisfied_frontiers
    if missing_frontiers or stray_frontiers:
        frontier_wiring_failures.append(
            "frontier task keys drift: "
            f"missing={sorted(missing_frontiers)}, "
            f"extra={sorted(stray_frontiers)}"
        )
    for req_id, matches in sorted(frontier_tasks.items()):
        if req_id not in active_frontiers:
            # A ratified (satisfied) lane's task may still be open; its wiring
            # obligations ended with ratification.
            continue
        if len(matches) != 1:
            frontier_wiring_failures.append(
                f"{req_id}: expected one frontier task, found {matches}"
            )
            continue
        item_id = matches[0]
        item = bones[item_id]
        if item_id not in leaves or item["kind"] != "task":
            frontier_wiring_failures.append(
                f"{req_id}: {item_id} is not a dispatchable leaf task"
            )
        phase_labels = sorted(
            label
            for label in item["labels"]
            if re.fullmatch(r"phase-[a-f]", label)
        )
        if len(phase_labels) != 1:
            frontier_wiring_failures.append(
                f"{req_id}: expected one deadline phase, found {phase_labels}"
            )
            continue
        deadline = phase_labels[0][-1].upper()
        if deadline >= "C":
            barrier_phase = chr(ord(deadline) - 2)
            if (
                barrier_phase not in phase_ids
                or (phase_ids[barrier_phase], item_id) not in links
            ):
                frontier_wiring_failures.append(
                    f"{req_id}: lacks Phase {barrier_phase} just-in-time barrier"
                )
        if not any(blocker == item_id for blocker, _blocked in links):
            frontier_wiring_failures.append(
                f"{req_id}: ratification does not block a consuming Bone"
            )

    corpus_failures: list[str] = []
    for requirement in registry["requirements"]:
        if requirement["category"] != "corpus-family":
            continue
        req_id = requirement["id"]
        mapped_leaves = state["coverage"][req_id] & leaves
        interactions = [
            item_id
            for item_id in mapped_leaves
            if f"plan-key:corpus-{req_id.lower()}-interaction"
            in bones[item_id]["labels"]
        ]
        if len(mapped_leaves) < 2 or len(interactions) != 1:
            corpus_failures.append(
                f"{req_id}: leaves={len(mapped_leaves)}, interactions={interactions}"
            )

    directly_blocked = {blocked for _blocker, blocked in links}

    def has_blocked_or_punted_ancestor(item_id: str) -> bool:
        seen: set[str] = set()
        parent = bones[item_id]["parent"]
        while parent in bones and parent not in seen:
            seen.add(parent)
            if (
                parent in directly_blocked
                or bones[parent].get("urgency") == "punt"
            ):
                return True
            parent = bones[parent]["parent"]
        return False

    inherited_blocked = {
        item_id
        for item_id in bones
        if item_id not in directly_blocked
        and has_blocked_or_punted_ancestor(item_id)
    }
    dispatch_ready_leaves = sorted(
        item_id
        for item_id in leaves
        if bones[item_id]["kind"] != "goal"
        and bones[item_id]["state"] != "doing"
        and bones[item_id].get("urgency") != "punt"
        and item_id not in directly_blocked
        and item_id not in inherited_blocked
    )
    return {
        "active_edges": len(links),
        "blocked_bones": len(directly_blocked | inherited_blocked),
        "dispatch_ready_leaves": dispatch_ready_leaves,
        "layers": max(depth.values(), default=0) + 1,
        "cycle_nodes": cycle_nodes,
        "deleted_dependency_links": deleted_dependency_links,
        "duplicate_plan_keys": duplicate_plan_keys,
        "empty_goals": empty_goals,
        "large_leaf_tasks": large_leaf_tasks,
        "undersized_goals": undersized_goals,
        "dispatch_readiness_failures": dispatch_readiness_failures,
        "risk_routing_failures": risk_routing_failures,
        "phase_failures": phase_failures,
        "phase_barrier_failures": phase_barrier_failures,
        "phase_exit_failures": phase_exit_failures,
        "pr_exit_failures": pr_exit_failures,
        "gate_failures": gate_failures,
        "risk_checkpoint_failures": risk_checkpoint_failures,
        "frontier_wiring_failures": frontier_wiring_failures,
        "corpus_failures": corpus_failures,
    }


def render_report(
    registry: dict[str, Any] | None = None,
    state: dict[str, Any] | None = None,
) -> str:
    registry = registry or build_registry()
    state = state or traceability_state(registry)
    graph = graph_contract_state(registry, state)
    by_category: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for item in registry["requirements"]:
        by_category[item["category"]].append(item)
    lines = [
        "# Plan ↔ Bones Traceability",
        "",
        "Generated by `tools/generate_traceability.py`; do not edit by hand.",
        "",
        "## Coverage",
        "",
        "| Category | Total | Active | Leaf-covered | Uncovered | Deferred | Satisfied |",
        "|---|---:|---:|---:|---:|---:|---:|",
    ]
    for category in sorted(by_category):
        items = by_category[category]
        active = [item for item in items if item["status"] == ACTIVE]
        leaf_covered = sum(
            bool(state["coverage"][item["id"]] & state["leaves"]) for item in active
        )
        uncovered = sum(not state["coverage"][item["id"]] for item in active)
        deferred = sum(item["status"] == "deferred" for item in items)
        satisfied = sum(item["status"] == "satisfied" for item in items)
        lines.append(
            f"| {category} | {len(items)} | {len(active)} | {leaf_covered} | "
            f"{uncovered} | {deferred} | {satisfied} |"
        )
    total = len(registry["requirements"])
    active = sum(item["status"] == ACTIVE for item in registry["requirements"])
    covered = sum(
        bool(state["coverage"][item["id"]])
        for item in registry["requirements"]
        if item["status"] == ACTIVE
    )
    lines.extend(
        [
            "",
            f"Registry: **{total}** requirements; **{active}** active; "
            f"**{covered}** active requirements mapped.",
            "",
            f"Graph: **{len(state['bones'])}** active Bones; "
            f"**{len(state['leaves'])}** active leaves; "
            f"**{len(state['untraced'])}** untraced Bones.",
            "",
            "## Graph contract",
            "",
            f"- Active blocking edges: **{graph['active_edges']}**.",
            f"- Dependency layers: **{graph['layers']}**.",
            f"- Dispatch-ready leaves: **{len(graph['dispatch_ready_leaves'])}**; "
            f"dependency-suppressed Bones: **{graph['blocked_bones']}**.",
            f"- Dependency cycles: **{len(graph['cycle_nodes'])}**.",
            f"- Dependencies incident to deleted Bones: "
            f"**{len(graph['deleted_dependency_links'])}**.",
            f"- Duplicate generated plan keys: **{len(graph['duplicate_plan_keys'])}**.",
            f"- Empty goals: **{len(graph['empty_goals'])}**.",
            f"- L/XL leaf tasks: **{len(graph['large_leaf_tasks'])}**.",
            f"- Undersized goals: **{len(graph['undersized_goals'])}**.",
            f"- Initial dispatch-readiness failures: "
            f"**{len(graph['dispatch_readiness_failures'])}**.",
            f"- Risk-routing failures: **{len(graph['risk_routing_failures'])}**.",
            f"- Missing phase barriers: **{len(graph['phase_barrier_failures'])}**.",
            f"- Phase exit-goal failures: **{len(graph['phase_exit_failures'])}**.",
            f"- PR exit wiring failures: **{len(graph['pr_exit_failures'])}**.",
            f"- Gate wiring failures: **{len(graph['gate_failures'])}**.",
            f"- Risk/kill checkpoint wiring failures: "
            f"**{len(graph['risk_checkpoint_failures'])}**.",
            f"- Frontier-lane wiring failures: "
            f"**{len(graph['frontier_wiring_failures'])}**.",
            f"- Corpus atomization failures: **{len(graph['corpus_failures'])}**.",
            "",
            "## Uncovered active requirements",
            "",
        ]
    )
    if state["uncovered"]:
        for item in state["uncovered"]:
            lines.append(
                f"- `{item['requirement']}` ({item['category']}): {item['summary']}"
            )
    else:
        lines.append("- None.")
    lines.extend(["", "## Requirements mapped only to non-leaf Bones", ""])
    if state["nonleaf_only"]:
        for item in state["nonleaf_only"]:
            lines.append(
                f"- `{item['requirement']}`: {', '.join(f'`{bone}`' for bone in item['bones'])}"
            )
    else:
        lines.append("- None.")
    lines.extend(["", "## Untraced active Bones", ""])
    if state["untraced"]:
        for item in state["untraced"]:
            lines.append(f"- `{item['bone']}`: {item['title']}")
    else:
        lines.append("- None.")
    lines.extend(["", "## Invalid or over-bundled mappings", ""])
    if state["unknown_labels"]:
        for item in state["unknown_labels"]:
            lines.append(
                f"- Unknown `{item['requirement']}` on `{item['bone']}`."
            )
    if state["overbundled"]:
        for item in state["overbundled"]:
            lines.append(
                f"- `{item['bone']}` maps {len(item['requirements'])} requirements."
            )
    if not state["unknown_labels"] and not state["overbundled"]:
        lines.append("- None.")
    return "\n".join(lines) + "\n"


def validate_checked_traceability() -> dict[str, Any]:
    expected = build_registry()
    if not REGISTRY_PATH.exists():
        raise AssertionError(f"missing generated registry: {REGISTRY_PATH.relative_to(ROOT)}")
    actual = json.loads(REGISTRY_PATH.read_text(encoding="utf-8"))
    if actual != expected:
        raise AssertionError(
            "PLAN_REQUIREMENTS.json is stale; run tools/generate_traceability.py"
        )
    state = traceability_state(expected)
    graph = graph_contract_state(expected, state)
    expected_report = render_report(expected, state)
    if not REPORT_PATH.exists() or REPORT_PATH.read_text(encoding="utf-8") != expected_report:
        raise AssertionError(
            "PLAN_BONE_TRACEABILITY.md is stale; run tools/generate_traceability.py"
        )
    failures = {
        key: state[key]
        for key in (
            "uncovered",
            "nonleaf_only",
            "untraced",
            "unknown_labels",
            "overbundled",
        )
        if state[key]
    }
    for key in (
        "cycle_nodes",
        "deleted_dependency_links",
        "duplicate_plan_keys",
        "empty_goals",
        "large_leaf_tasks",
        "undersized_goals",
        "dispatch_readiness_failures",
        "risk_routing_failures",
        "phase_failures",
        "phase_barrier_failures",
        "phase_exit_failures",
        "pr_exit_failures",
        "gate_failures",
        "risk_checkpoint_failures",
        "frontier_wiring_failures",
        "corpus_failures",
    ):
        if graph[key]:
            failures[key] = graph[key]
    if failures:
        summary = ", ".join(f"{key}={len(value)}" for key, value in failures.items())
        raise AssertionError(f"plan/Bones traceability incomplete: {summary}")
    active = sum(item["status"] == ACTIVE for item in expected["requirements"])
    return {
        "requirements": len(expected["requirements"]),
        "active_requirements": active,
        "active_bones": len(state["bones"]),
        "leaf_bones": len(state["leaves"]),
        "active_edges": graph["active_edges"],
        "dependency_layers": graph["layers"],
        "coverage": "complete",
    }
