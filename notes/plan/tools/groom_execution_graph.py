#!/usr/bin/env python3
"""Expand and label the Continuum Bones graph from the executable registry.

This is intentionally a Bones-CLI client rather than an event-log writer.
It is idempotent through ``plan-key:*`` labels and preserves existing IDs,
descriptions, dependencies, and user-created structure.
"""
from __future__ import annotations

import argparse
import csv
import json
import re
import subprocess
from collections import defaultdict
from pathlib import Path
from typing import Any, Iterable

from traceability import PROJECT_ROOT, ROOT, build_registry, project_bones, traceability_state

AGENT = "continuum-planner"
STOP_WORDS = {
    "a", "all", "an", "and", "are", "as", "at", "be", "before", "by", "can",
    "does", "every", "for", "from", "has", "have", "in", "into", "is", "it",
    "its", "no", "not", "of", "on", "or", "per", "that", "the", "their", "then",
    "this", "to", "under", "with", "without", "v0", "v1",
}


def _run(arguments: list[str]) -> str:
    command = ["bn", "--agent", AGENT, "--format", "json", *arguments]
    try:
        return subprocess.check_output(
            command, cwd=PROJECT_ROOT, text=True, stderr=subprocess.STDOUT
        )
    except subprocess.CalledProcessError as exc:
        raise RuntimeError(
            f"bn command failed ({exc.returncode}): {' '.join(command[:6])}\n{exc.output}"
        ) from exc


def _find_id(value: Any) -> str | None:
    if isinstance(value, dict):
        candidate = value.get("id")
        if isinstance(candidate, str) and candidate.startswith("bn-"):
            return candidate
        for child in value.values():
            found = _find_id(child)
            if found:
                return found
    if isinstance(value, list):
        for child in value:
            found = _find_id(child)
            if found:
                return found
    return None


def _tokens(text: str) -> set[str]:
    return {
        token
        for token in re.findall(r"[a-z0-9]+", text.lower())
        if token not in STOP_WORDS and (len(token) >= 3 or token.isdigit())
    }


def _score(requirement: dict[str, Any], bone: dict[str, Any]) -> float:
    wanted = _tokens(requirement["summary"])
    metadata = requirement.get("metadata", {})
    wanted |= _tokens(str(metadata.get("contract", "")))
    candidate = _tokens(f"{bone['title']} {bone['description']}")
    common = wanted & candidate
    rare_bonus = sum(1.5 for token in common if len(token) >= 9)
    return len(common) + rare_bonus


def _short(text: str, limit: int = 92) -> str:
    text = re.sub(r"\s+", " ", text).strip().rstrip(".;")
    return text if len(text) <= limit else text[: limit - 1].rstrip() + "…"


def _key_label(key: str) -> str:
    return (
        "plan-key:" + re.sub(r"[^A-Za-z0-9._-]+", "-", key).strip("-")
    ).lower()


def _requirement_description(requirement: dict[str, Any], purpose: str) -> str:
    source = requirement["source"]
    metadata = requirement.get("metadata", {})
    lines = [
        "# Authoritative requirement",
        f"`{requirement['id']}` — {requirement['summary']}",
        f"Source: `notes/plan/{source['path']}:{source['line']}`.",
    ]
    contract = metadata.get("contract")
    if contract and contract.lower() != "controls":
        lines.extend(["", "# Contract", str(contract)])
    controls = metadata.get("controls")
    if controls:
        lines.extend(["", "# Required controls"])
        lines.extend(f"- {control}" for control in controls)
    for key, heading in (
        ("required_evidence", "Required evidence"),
        ("experiment", "Experiment"),
        ("pass_condition", "Pass condition"),
        ("failure_consequence", "Failure consequence"),
        ("threshold", "Promotion threshold / kill"),
        ("fallback", "Fallback"),
    ):
        if metadata.get(key):
            lines.extend(["", f"# {heading}", str(metadata[key])])
    lines.extend(
        [
            "",
            "# Delivery and evidence",
            purpose,
            "",
            "- [ ] Implementation, policy, or experiment is complete at the cited scope.",
            "- [ ] Positive, negative, and boundary evidence is retained under stable artifact IDs.",
            "- [ ] Relevant clean, incremental, mutation, security, and documentation gates pass.",
            "- [ ] The requirement label remains attached to the evidence-owning Bone.",
        ]
    )
    return "\n".join(lines)


class Editor:
    def __init__(self) -> None:
        self.bones = project_bones()
        self.links = self._project_links()

    @staticmethod
    def _project_links() -> set[tuple[str, str]]:
        """Return (blocker, blocked) pairs from the event log."""
        links: set[tuple[str, str]] = set()
        for path in sorted((PROJECT_ROOT / ".bones/events").glob("*.events")):
            for raw in path.read_text(encoding="utf-8").splitlines():
                if not raw or raw.startswith("#"):
                    continue
                fields = raw.split("\t")
                if len(fields) < 7:
                    continue
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

    def active(self) -> dict[str, dict[str, Any]]:
        return {
            item_id: item
            for item_id, item in self.bones.items()
            if not item["deleted"] and item["state"] not in {"done", "closed", "archived"}
        }

    def by_title(self, title: str) -> str | None:
        return next(
            (
                item_id
                for item_id, item in self.active().items()
                if item["title"] == title
            ),
            None,
        )

    def by_key(self, key: str) -> str | None:
        label = _key_label(key)
        return next(
            (
                item_id
                for item_id, item in self.active().items()
                if label in item["labels"]
            ),
            None,
        )

    def children(self, parent: str) -> list[str]:
        return [
            item_id
            for item_id, item in self.active().items()
            if item["parent"] == parent
        ]

    def descendants(self, parent: str) -> set[str]:
        result: set[str] = set()
        pending = list(self.children(parent))
        while pending:
            item_id = pending.pop()
            if item_id in result:
                continue
            result.add(item_id)
            pending.extend(self.children(item_id))
        return result

    def leaves(self, parent: str | None = None) -> set[str]:
        active = self.active()
        parents = {item["parent"] for item in active.values() if item["parent"]}
        leaves = {item_id for item_id in active if item_id not in parents}
        return leaves if parent is None else leaves & self.descendants(parent)

    def tag(self, item_id: str, *labels: str) -> None:
        normalized = {label.lower() for label in labels}
        new = sorted(normalized - self.bones[item_id]["labels"])
        if not new:
            return
        _run(["bone", "tag", item_id, *new])
        self.bones[item_id]["labels"].update(new)

    def kind(self, item_id: str, kind: str) -> None:
        if self.bones[item_id]["kind"] != kind:
            _run(["update", item_id, "--kind", kind])
            self.bones[item_id]["kind"] = kind
        if kind == "goal" and self.bones[item_id].get("size") not in {"l", "xl"}:
            _run(["update", item_id, "--size", "l"])
            self.bones[item_id]["size"] = "l"

    def size(self, item_id: str, size: str) -> None:
        if self.bones[item_id].get("size") != size:
            _run(["update", item_id, "--size", size])
            self.bones[item_id]["size"] = size

    def ensure(
        self,
        *,
        key: str,
        title: str,
        kind: str,
        parent: str | None,
        labels: Iterable[str],
        description: str,
        size: str,
        existing_id: str | None = None,
    ) -> str:
        item_id = existing_id or self.by_key(key)
        if not item_id:
            arguments = [
                "create",
                "--force",
                "--title",
                title,
                "--kind",
                kind,
                "--size",
                size,
                "--description",
                description,
                "--label",
                _key_label(key),
            ]
            if parent:
                arguments.extend(["--parent", parent])
            for label in sorted(set(labels)):
                arguments.extend(["--label", label])
            output = _run(arguments)
            try:
                item_id = _find_id(json.loads(output))
            except json.JSONDecodeError:
                item_id = None
            if not item_id:
                matches = re.findall(r"\bbn-[a-z0-9]+\b", output)
                item_id = matches[0] if matches else None
            if not item_id:
                raise RuntimeError(f"could not parse created Bone ID from: {output}")
            self.bones[item_id] = {
                "id": item_id,
                "title": title,
                "description": description,
                "kind": kind,
                "parent": parent,
                "labels": {_key_label(key), *(label.lower() for label in labels)},
                "state": "open",
                "deleted": False,
            }
        else:
            self.kind(item_id, kind)
            self.tag(item_id, _key_label(key), *labels)
        return item_id

    def goal(
        self,
        key: str,
        title: str,
        parent: str,
        labels: Iterable[str],
        description: str,
        *,
        existing_id: str | None = None,
        manual: bool = False,
    ) -> str:
        goal_labels = set(labels)
        if manual:
            goal_labels.add("goal:manual")
        return self.ensure(
            key=key,
            title=title,
            kind="goal",
            parent=parent,
            labels=goal_labels,
            description=description,
            size="l",
            existing_id=existing_id,
        )

    def task(
        self,
        key: str,
        title: str,
        parent: str,
        labels: Iterable[str],
        description: str,
        *,
        size: str = "s",
        existing_id: str | None = None,
    ) -> str:
        return self.ensure(
            key=key,
            title=title,
            kind="task",
            parent=parent,
            labels=labels,
            description=description,
            size=size,
            existing_id=existing_id,
        )

    def dep(self, blocker: str, blocked: str) -> None:
        if blocker == blocked or (blocker, blocked) in self.links:
            return
        _run(["dep", "add", blocker, "--blocks", blocked])
        self.links.add((blocker, blocked))

    def undep(self, blocker: str, blocked: str) -> None:
        if (blocker, blocked) not in self.links:
            return
        _run(["dep", "rm", blocker, blocked])
        self.links.discard((blocker, blocked))


def _phase_index(editor: Editor) -> dict[str, str]:
    result: dict[str, str] = {}
    for phase in "ABCDEF":
        prefix = f"Phase {phase} —"
        matches = [
            item_id
            for item_id, item in editor.active().items()
            if item["kind"] == "goal" and item["title"].startswith(prefix)
        ]
        if len(matches) != 1:
            raise AssertionError(f"expected one {prefix!r} goal, found {matches}")
        result[phase] = matches[0]
    return result


def _pr_index(editor: Editor) -> dict[str, str]:
    result: dict[str, str] = {}
    for item_id, item in editor.active().items():
        match = re.match(r"^PR (\d+[a-z]?)\s+[—–-]", item["title"])
        if match:
            result[f"PR-{match.group(1).upper()}"] = item_id
    return result


def _req_labels(editor: Editor, item_id: str) -> set[str]:
    return {
        label[4:].upper()
        for label in editor.bones[item_id]["labels"]
        if label.startswith("req:")
    }


def _baseline_trace(
    editor: Editor, root: str, phases: dict[str, str], pr_parents: dict[str, str]
) -> None:
    editor.tag(root, "req:PROGRAM")
    for item_id in editor.children(root):
        if item_id not in phases.values():
            editor.tag(item_id, "req:PROGRAM")
    for phase, phase_id in phases.items():
        editor.tag(phase_id, f"req:PHASE-{phase}")
        for descendant in editor.descendants(phase_id):
            editor.tag(descendant, f"req:PHASE-{phase}")
    for pr_id, item_id in pr_parents.items():
        editor.tag(item_id, f"req:{pr_id}")
        for descendant in editor.descendants(item_id):
            editor.tag(descendant, f"req:{pr_id}")


def _normalize_goal_sizes(editor: Editor) -> None:
    for item_id, item in editor.active().items():
        if item["kind"] == "goal":
            editor.kind(item_id, "goal")


def _map_prs(
    editor: Editor,
    registry: dict[str, Any],
    pr_parents: dict[str, str],
    phase_for: dict[str, str],
) -> dict[str, str]:
    requirements = registry["requirements"]
    pr_exits: dict[str, str] = {}
    for pr_id, parent in sorted(pr_parents.items()):
        phase = phase_for[parent]
        phase_label = f"req:PHASE-{phase}"
        impl = [
            item
            for item in requirements
            if item.get("parent") == pr_id and item["category"] == "pr-deliverable"
        ]
        exits = [
            item
            for item in requirements
            if item.get("parent") == pr_id and item["category"] == "pr-exit"
        ]
        editor.kind(parent, "goal")
        owners: set[str] = set()
        for requirement in impl:
            covered = [
                item_id
                for item_id in editor.leaves(parent)
                if requirement["id"] in _req_labels(editor, item_id)
            ]
            if covered:
                owners.add(covered[0])
                continue
            candidates = [
                item_id
                for item_id in editor.leaves(parent)
                if "exit evidence" not in editor.bones[item_id]["title"].lower()
                and len(_req_labels(editor, item_id)) < 6
            ]
            ranked = sorted(
                ((_score(requirement, editor.bones[item_id]), item_id) for item_id in candidates),
                reverse=True,
            )
            if ranked and ranked[0][0] >= 2.0:
                owner = ranked[0][1]
                editor.tag(owner, f"req:{requirement['id']}", f"req:{pr_id}", phase_label)
            else:
                owner = editor.task(
                    f"pr:{requirement['id']}",
                    f"{pr_id} / {requirement['id'].split('-')[-2]}-{requirement['id'].split('-')[-1]} — "
                    f"{_short(requirement['summary'])}",
                    parent,
                    {f"req:{requirement['id']}", f"req:{pr_id}", phase_label, "implementation"},
                    _requirement_description(
                        requirement,
                        "Deliver this PR-scoped change as one reviewable semantic unit, with its "
                        "own tests and artifact evidence. Do not close it through the PR umbrella.",
                    ),
                )
            owners.add(owner)
        for requirement in exits:
            exit_task = editor.task(
                f"pr-exit:{pr_id}",
                f"{pr_id} exit evidence — {_short(requirement['summary'])}",
                parent,
                {f"req:{requirement['id']}", f"req:{pr_id}", phase_label, "integration", "quality"},
                _requirement_description(
                    requirement,
                    "Run the PR's clean integration path, retain the exact evidence named by "
                    "the exit contract, and fail closed on any unsupported or inconclusive result.",
                ),
            )
            pr_exits[pr_id] = exit_task
            for owner in owners:
                editor.dep(owner, exit_task)
    return pr_exits


def _frontier_and_debt(
    editor: Editor,
    registry: dict[str, Any],
    phases: dict[str, str],
) -> None:
    frontier = {
        "FR-01": "bn-hii",
        "FR-03": "bn-1cp",
        "FR-04": "bn-2bu",
        "FR-05": "bn-2ba",
        "FR-06": "bn-2tt",
        "FR-07": "bn-195",
        "FR-08": "bn-1pg",
        "FR-09": "bn-1ir",
        "FR-11": "bn-1y7",
        "FR-12": "bn-320",
        "FR-18": "bn-2fj",
        "FR-19": "bn-cs3",
        "FR-20": "bn-2oy",
        "FR-21": "bn-1o2",
    }
    for req_id, item_id in frontier.items():
        editor.tag(item_id, f"req:{req_id}", f"plan-key:frontier-{req_id.lower()}")

    # A deadline-phase lane should be ratified during the immediately preceding
    # phase, rather than every future research decision crowding the first
    # dispatch window.
    deadline_barriers = {
        "bn-2tt": "A",  # Phase C deadline; execute during Phase B.
        "bn-1cp": "B",  # Phase D deadline; execute during Phase C.
        "bn-1y7": "B",
        "bn-320": "B",
        "bn-195": "C",  # Phase E deadline; execute during Phase D.
        "bn-cs3": "C",
        "bn-1pg": "D",  # Phase F deadline; execute during Phase E.
    }
    for item_id, barrier_phase in deadline_barriers.items():
        editor.dep(phases[barrier_phase], item_id)

    requirements = {item["id"]: item for item in registry["requirements"]}
    combined = "bn-2z3"
    editor.kind(combined, "goal")
    lane_specs = {
        "FR-02": (
            "Ratify exploration-reduction thresholds and denominator",
            "C",
            "A",
            "bn-d4fm",
        ),
        "FR-10": (
            "Ratify bidirectional-lens ambiguity threshold and fallback",
            "D",
            "B",
            "bn-2kzi",
        ),
        "FR-13": (
            "Ratify nominal/orbit-finite promotion and kill thresholds",
            "F",
            "D",
            "bn-s2b",
        ),
        "FR-14": (
            "Ratify sheaf-composition utility threshold and fallback",
            "D",
            "B",
            "bn-2k6q",
        ),
    }
    for req_id, (title, deadline_phase, barrier_phase, consumer) in lane_specs.items():
        requirement = requirements[req_id]
        task = editor.task(
            f"frontier:{req_id}",
            title,
            combined,
            {
                f"req:{req_id}",
                f"req:PHASE-{deadline_phase}",
                f"phase-{deadline_phase.lower()}",
                f"plan-key:frontier-{req_id.lower()}",
                "research",
            },
            _requirement_description(
                requirement,
                f"During the phase before Phase {deadline_phase}, fix the lane's numeric "
                "threshold and denominator, quote it verbatim in the authoritative register "
                "with matching quote-id markers, validate the fallback, and leave the consumer "
                "blocked if ratification fails.",
            ),
            size="s",
        )
        editor.dep(phases[barrier_phase], task)
        editor.undep(combined, consumer)
        editor.dep(task, consumer)

    editor.task(
        "frontier:docs31-residuals",
        "Merge residual docs/31 thresholds into the authoritative frontier register",
        combined,
        {"req:PROGRAM", "docs", "governance", "research"},
        "# Scope\n"
        "Merge docs/31's remaining cubical-reduction, semiring, assumption-synthesis, "
        "and abstraction-discovery thresholds into plan §24.5 so the program has one "
        "lane authority; reconcile disagreements in the source notes.\n\n"
        "# Acceptance Criteria\n"
        "- [ ] Each residual threshold has one register row, baseline, promotion/kill "
        "criterion, fallback, and source citation.\n"
        "- [ ] Draft qualitative criteria stay visibly draft until quantified.\n"
        "- [ ] Register quote-identity and dossier validation pass.",
        size="m",
    )

    debt = {
        "SD-01": "bn-1cw",
        "SD-07": "bn-1lz",
        "SD-08": "bn-xd2",
        "SD-09": "bn-v1c",
        "SD-10": "bn-7kj",
    }
    for req_id, item_id in debt.items():
        editor.tag(item_id, f"req:{req_id}")


def _corpus_rows() -> list[dict[str, str]]:
    with (ROOT / "corpus/tla-examples/validated-examples.csv").open(
        encoding="utf-8", newline=""
    ) as handle:
        return list(csv.DictReader(handle))


def _corpus_tasks(
    editor: Editor,
    requirements: dict[str, dict[str, Any]],
    pr_parents: dict[str, str],
) -> None:
    parent_ids = {
        "wave0": "bn-23z",
        "wave1": "bn-2my",
        "wave2": "bn-16n",
        "wave3": "bn-2um",
        "wave4p2": "bn-29bf",
        "wave4p3": "bn-2z63",
        "wave4p4": "bn-2a7y",
        "raise3": "bn-2mcp",
        "raise4": "bn-3k5a",
        "interaction": "bn-32j",
    }
    for key, item_id in parent_ids.items():
        editor.kind(item_id, "goal")
        phase = "C" if key in {"wave0", "wave1"} else ("D" if key in {"wave2", "wave3", "raise3", "raise4"} else "F")
        editor.tag(item_id, f"req:PHASE-{phase}")
    existing_p5 = {
        "TV-023": "bn-3f4b",
        "TV-025": "bn-sb7j",
        "TV-028": "bn-19oi",
        "TV-029": "bn-1zd5",
        "TV-039": "bn-3lsu",
    }
    existing_wave5 = {
        "TV-034": "bn-3vo5",
        "TV-073": "bn-37al",
        "TV-075": "bn-3mjk",
        "TV-080": "bn-2wj4",
    }
    for row in _corpus_rows():
        req_id = row["id"]
        requirement = requirements[req_id]
        wave = int(row["porting_wave"])
        parity = row["required_parity"]
        if wave == 0:
            parent, phase = parent_ids["wave0"], "C"
        elif wave == 1:
            parent, phase = parent_ids["wave1"], "C"
        elif wave == 2:
            parent, phase = parent_ids["wave2"], "D"
        elif wave == 3:
            parent, phase = parent_ids["wave3"], "D"
        elif wave == 4:
            parent, phase = parent_ids[f"wave4{parity.lower()}"], "F"
        else:
            parent, phase = "bn-1ium", "F"

        if req_id in existing_wave5:
            port = existing_wave5[req_id]
            editor.tag(port, f"req:{req_id}", "req:PHASE-F")
        else:
            target = "P2" if wave in {0, 1} else parity
            extra = {"req:PR-27A"} if wave in {0, 1} else set()
            port = editor.task(
                f"corpus:{req_id}:port",
                f"{req_id} — {row['title']} — native port through {target}",
                parent,
                {f"req:{req_id}", f"req:PHASE-{phase}", "corpus", *extra},
                _requirement_description(
                    requirement,
                    f"Port the pinned source family through parity {target}. Retain source "
                    "correspondence, normalized model identity, oracle differentials, mutants, "
                    "and the parity manifest required by the corpus policy.",
                ),
                size="m",
            )

        final_task = port
        if wave in {0, 1} and parity in {"P3", "P4"}:
            raise_parent = parent_ids["raise3" if parity == "P3" else "raise4"]
            final_task = editor.task(
                f"corpus:{req_id}:raise",
                f"{req_id} — raise {_short(row['title'], 65)} from P2 to {parity}",
                raise_parent,
                {f"req:{req_id}", "req:PHASE-D", "corpus", "proof"},
                _requirement_description(
                    requirement,
                    f"Raise the Phase C P2 port to full {parity}: temporal/fairness evidence, "
                    "refinement correspondence, and Lean theorem/bridge evidence as required.",
                ),
                size="m",
            )
            editor.dep(port, final_task)

        if req_id in existing_p5:
            p5 = existing_p5[req_id]
            editor.tag(p5, f"req:{req_id}", "req:PHASE-E")
            editor.dep(final_task, p5)
            final_task = p5

        interaction = editor.task(
            f"corpus:{req_id}:interaction",
            f"{req_id} — interaction artifact and hidden agent task",
            parent_ids["interaction"],
            {f"req:{req_id}", "req:PHASE-F", "corpus", "aci"},
            _requirement_description(
                requirement,
                "Produce the B23 interaction-parity artifact: seeded failure or proof hole, "
                "exact oracle evidence, deterministic hidden mutation split, Context Pack, "
                "repair/proof task, grading contract, and leakage audit.",
            ),
            size="s",
        )
        editor.dep(final_task, interaction)


def _experiment_tasks(
    editor: Editor,
    requirements: dict[str, dict[str, Any]],
    pr_parents: dict[str, str],
    pr_exits: dict[str, str],
) -> None:
    targets = {
        "G0-DX-01": ("PR-11", pr_parents["PR-11"]),
        "G0-DX-02": ("PR-12", pr_parents["PR-12"]),
        "G0-DX-03": ("PR-6", pr_parents["PR-6"]),
        "G0-DX-04": ("PR-19", pr_parents["PR-19"]),
        "G0-DX-05": ("PR-24", pr_parents["PR-24"]),
        "G0-DX-06": ("PR-21", pr_parents["PR-21"]),
        "G0-DX-07": ("PR-29", pr_parents["PR-29"]),
        "G0-DX-08": (None, "bn-1db"),
        "G0-DX-09": (None, "bn-236"),
        "G0-DX-10": ("PR-10", pr_parents["PR-10"]),
        "G0-DX-11": ("PR-28", pr_parents["PR-28"]),
        "G0-DX-12": ("PR-7", pr_parents["PR-7"]),
        "G0-DX-13": ("PR-2", pr_parents["PR-2"]),
        "G0-DX-14": ("PR-6", pr_parents["PR-6"]),
        "G0-DX-15": (None, "bn-1z1"),
    }
    for req_id, (pr_id, parent) in targets.items():
        requirement = requirements[req_id]
        phase = (
            "E" if req_id == "G0-DX-07" else
            "D" if req_id in {"G0-DX-08", "G0-DX-11"} else
            "F" if req_id in {"G0-DX-09", "G0-DX-15"} else
            "C" if req_id == "G0-DX-05" else
            "B" if req_id in {"G0-DX-04", "G0-DX-06"} else
            "A"
        )
        labels = {f"req:{req_id}", f"req:PHASE-{phase}", "g0", "quality"}
        if pr_id:
            labels.add(f"req:{pr_id}")
        task = editor.task(
            f"experiment:{req_id}",
            f"{req_id} falsification — {_short(requirement['summary'])}",
            parent,
            labels,
            _requirement_description(
                requirement,
                "Execute the falsification experiment against the production implementation, "
                "including adversarial controls and the named baseline. A finite artifact-shape "
                "spike is not sufficient evidence.",
            ),
            size="m",
        )
        if pr_id and pr_id in pr_exits:
            editor.dep(task, pr_exits[pr_id])


def _category_goal(
    editor: Editor,
    key: str,
    title: str,
    parent: str,
    phase: str,
) -> str:
    return editor.goal(
        key,
        title,
        parent,
        {f"req:PHASE-{phase}", "governance", "quality"},
        "Execution container for stable obligations extracted from the authoritative plan. "
        "Children are the evidence-owning units; this goal does not substitute for them.",
    )


def _atomic_category_tasks(
    editor: Editor,
    items: list[dict[str, Any]],
    parents: dict[str, tuple[str, str]],
    category: str,
    purpose: str,
    phase_selector: Any,
) -> list[str]:
    created: list[str] = []
    goals: dict[str, str] = {}
    for requirement in items:
        phase = phase_selector(requirement)
        parent, title = parents[phase]
        if phase not in goals:
            goals[phase] = _category_goal(
                editor, f"{category}:phase-{phase}", title, parent, phase
            )
        task = editor.task(
            f"{category}:{requirement['id']}",
            f"{requirement['id']} — {_short(requirement['summary'])}",
            goals[phase],
            {f"req:{requirement['id']}", f"req:PHASE-{phase}", category, "quality"},
            _requirement_description(requirement, purpose),
            size="s",
        )
        created.append(task)
    return created


def _crosscutting_tasks(
    editor: Editor,
    registry: dict[str, Any],
    phases: dict[str, str],
    root: str,
) -> dict[str, list[str]]:
    active = [
        item for item in registry["requirements"] if item["status"] == "active"
    ]
    by_category: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for item in active:
        by_category[item["category"]].append(item)

    _atomic_category_tasks(
        editor,
        by_category["invariant"],
        {
            phase: (phases[phase], f"Constitutional invariant regressions — Phase {phase}")
            for phase in "ABCF"
        },
        "invariant",
        "Implement the invariant as an executable architectural guard and permanent "
        "regression. Include a mutant that would violate it and prove the guard detects it.",
        lambda item: (
            "C" if item["id"] == "INV-010" else
            "B" if item["id"] in {"INV-011", "INV-013", "INV-017"} else
            "F" if item["id"] == "INV-018" else
            "A"
        ),
    )
    _atomic_category_tasks(
        editor,
        by_category["proof-obligation"],
        {"D": (phases["D"], "Proof-obligation implementation and checker matrix")},
        "proof-obligation",
        "Implement the generating-side check, independent checking path, malformed-evidence "
        "mutants, and receipt fields for this obligation. Kernel acceptance is the authority.",
        lambda _item: "D",
    )
    _atomic_category_tasks(
        editor,
        by_category["claim"],
        {
            "B": (phases["B"], "Claim evidence campaigns — semantic and repair loop"),
            "D": (phases["D"], "Claim evidence campaigns — proof and reduction"),
            "F": (phases["F"], "Claim evidence campaigns — corpus and adoption"),
        },
        "claim",
        "Run the required evidence campaign, update the machine claim ledger, test prohibited "
        "wording, and promote wording only to the evidence state independently reproduced.",
        lambda item: (
            "D" if item["id"] in {
                "C003", "C007", "C008", "C011", "C012", "C014", "C015", "C016",
                "C017", "C024", "C026", "C027", "C028", "C029", "C035",
            } else
            "F" if item["id"] in {
                "C009", "C010", "C013", "C019", "C021", "C022", "C032", "C034",
            } else
            "B"
        ),
    )
    _atomic_category_tasks(
        editor,
        by_category["threat"],
        {
            phase: (phases[phase], f"Threat-control implementation and abuse cases — Phase {phase}")
            for phase in "ABCDF"
        },
        "threat",
        "Implement every listed control at the actual trust boundary, retain a hostile fixture "
        "or exploit mutant, and prove assurance can only stay equal or weaken on failure.",
        lambda item: {
            "T01": "A", "T02": "A", "T03": "C", "T04": "A", "T05": "F",
            "T06": "B", "T07": "B", "T08": "D", "T09": "B", "T10": "B",
            "T11": "F", "T12": "A", "T13": "C", "T14": "D", "T15": "D",
        }[item["id"]],
    )

    risk_goal = editor.goal(
        "risk-and-kill",
        "Risk retirement, kill-signal assays, and scope decisions",
        root,
        {"req:PROGRAM", "governance", "risk"},
        "Manual decision ledger for docs/08 risks and plan §24 kill criteria. Evidence tasks "
        "may execute autonomously; accepting a kill/defer/narrow decision remains privileged.",
        manual=True,
    )
    # Earliest phase where the signal can be measured and a bounded decision
    # package can be produced. These checkpoints are not permission for an
    # agent to take the privileged continue/narrow/defer/kill decision.
    risk_phase = {
        "R01": "A",
        "R02": "D",
        "R03": "B",
        "R04": "C",
        "R05": "C",
        "R06": "B",
        "R07": "D",
        "R08": "C",
        "R09": "F",
        "R10": "B",
        "R11": "D",
        "R12": "D",
        "R13": "B",
        "R14": "A",
        "R15": "D",
        "R16": "A",
        "R17": "F",
        "R18": "B",
        "R19": "C",
        "R20": "F",
        "R21": "C",
    }
    kill_phase = {
        "KILL-01": "B",
        "KILL-02": "B",
        "KILL-03": "D",
        "KILL-04": "D",
        "KILL-05": "C",
        "KILL-06": "D",
        "KILL-07": "B",
        "KILL-08": "A",
        "KILL-09": "B",
        "KILL-10": "C",
        "KILL-11": "D",
        "KILL-12": "E",
        "KILL-13": "E",
        "KILL-14": "F",
        "KILL-15": "F",
        "KILL-16": "B",
        "KILL-17": "F",
        "KILL-18": "F",
        "KILL-19": "B",
    }
    expected_risks = {item["id"] for item in by_category["risk"]}
    expected_kills = {item["id"] for item in by_category["kill-criterion"]}
    if set(risk_phase) != expected_risks:
        raise AssertionError(
            "risk phase map drift: "
            f"missing={sorted(expected_risks - set(risk_phase))}, "
            f"extra={sorted(set(risk_phase) - expected_risks)}"
        )
    if set(kill_phase) != expected_kills:
        raise AssertionError(
            "kill phase map drift: "
            f"missing={sorted(expected_kills - set(kill_phase))}, "
            f"extra={sorted(set(kill_phase) - expected_kills)}"
        )

    risk_tasks: dict[str, list[str]] = defaultdict(list)
    for category in ("risk", "kill-criterion"):
        for requirement in by_category[category]:
            phase = (
                risk_phase[requirement["id"]]
                if category == "risk"
                else kill_phase[requirement["id"]]
            )
            task = editor.task(
                f"{category}:{requirement['id']}",
                f"{requirement['id']} — {_short(requirement['summary'])}",
                risk_goal,
                {
                    f"req:{requirement['id']}",
                    f"req:PHASE-{phase}",
                    f"phase-{phase.lower()}",
                    "req:PROGRAM",
                    category,
                    "governance",
                },
                _requirement_description(
                    requirement,
                    f"At the Phase {phase} checkpoint, instrument the leading indicator, "
                    "execute the mitigation or falsification assay, retain trend data, and "
                    "prepare an explicit continue/narrow/defer/kill decision package. Agents "
                    "do not take the privileged program decision.",
                ),
                size="m",
            )
            editor.size(task, "m")
            if phase != "A":
                editor.dep(phases[chr(ord(phase) - 1)], task)
            risk_tasks[phase].append(task)

    _atomic_category_tasks(
        editor,
        by_category["success-metric"],
        {"F": (phases["F"], "Success-metric instrumentation and decision dashboard")},
        "success-metric",
        "Define numerator, denominator, cohort/workload, collection point, confidence treatment, "
        "and decision threshold; retain raw measurements and prevent metric gaming.",
        lambda _item: "F",
    )
    return risk_tasks


def _policy_tasks(
    editor: Editor,
    registry: dict[str, Any],
    phases: dict[str, str],
    pr_parents: dict[str, str],
) -> None:
    active = [item for item in registry["requirements"] if item["status"] == "active"]
    requirements = {item["id"]: item for item in active}
    phase_c_goal = _category_goal(
        editor,
        "test-policy:phase-c",
        "Determinism and performance validation matrix",
        phases["C"],
        "C",
    )
    phase_f_goal = _category_goal(
        editor,
        "release-policy:phase-f",
        "Release reproducibility, compatibility, and blocker enforcement",
        phases["F"],
        "F",
    )
    group_specs: list[tuple[str, str, str, str]] = []
    for prefix in ("TEST", "GOV"):
        groups: dict[str, list[str]] = defaultdict(list)
        for req_id in requirements:
            match = re.match(rf"^{prefix}-(\d+)-", req_id)
            if match:
                groups[match.group(1)].append(req_id)
        for section, ids in groups.items():
            ids.sort(key=lambda value: int(value.rsplit("-", 1)[1]))
            for chunk_index in range(0, len(ids), 6):
                chunk = ids[chunk_index : chunk_index + 6]
                if prefix == "TEST":
                    if int(section) <= 6 or section == "9":
                        parent, phase = "bn-3ly0", "B"
                    elif section in {"7", "8"}:
                        parent, phase = phase_c_goal, "C"
                    else:
                        parent, phase = phase_f_goal, "F"
                else:
                    if section in {"1", "2", "3"}:
                        parent, phase = pr_parents["PR-0"], "A"
                    elif section == "4":
                        parent, phase = "bn-3ly0", "B"
                    else:
                        parent, phase = phase_f_goal, "F"
                group_specs.append((prefix, section, parent, phase, chunk))

    for prefix, section, parent, phase, chunk in group_specs:
        selected = [requirements[req_id] for req_id in chunk]
        first, last = chunk[0].rsplit("-", 1)[1], chunk[-1].rsplit("-", 1)[1]
        descriptions = "\n".join(
            f"- [ ] `{item['id']}` — {item['summary']}" for item in selected
        )
        editor.task(
            f"policy:{prefix}:{section}:{first}-{last}",
            f"{prefix} §{section} executable policy obligations {first}–{last}",
            parent,
            {
                *(f"req:{req_id}" for req_id in chunk),
                f"req:PHASE-{phase}",
                "governance" if prefix == "GOV" else "quality",
            },
            "# Authoritative obligations\n"
            + descriptions
            + "\n\n# Delivery and evidence\n"
            "Implement these related rules in one shared harness or policy boundary. Each "
            "checkbox requires positive enforcement, a violating fixture, retained output, "
            "and a stable link from the requirement ID to its evidence. Split this Bone if "
            "the obligations stop sharing implementation or review scope.",
            size="m",
        )


def _map_phase_deliverables(
    editor: Editor,
    registry: dict[str, Any],
    phases: dict[str, str],
) -> None:
    for requirement in registry["requirements"]:
        if requirement["status"] != "active" or requirement["category"] != "phase-deliverable":
            continue
        phase = requirement["id"].split("-")[1]
        candidates = [
            item_id
            for item_id in editor.leaves(phases[phase])
            if len(_req_labels(editor, item_id)) < 8
        ]
        ranked = sorted(
            ((_score(requirement, editor.bones[item_id]), item_id) for item_id in candidates),
            reverse=True,
        )
        if ranked and ranked[0][0] >= 2.0:
            editor.tag(ranked[0][1], f"req:{requirement['id']}")
        else:
            editor.task(
                f"phase-deliverable:{requirement['id']}",
                f"{requirement['id']} — {_short(requirement['summary'])}",
                phases[phase],
                {f"req:{requirement['id']}", f"req:PHASE-{phase}", "integration"},
                _requirement_description(
                    requirement,
                    "Own the phase-level integration boundary that is not fully represented by "
                    "one PR. Retain cross-component acceptance evidence and name every constituent "
                    "Bone rather than silently broadening its scope.",
                ),
                size="m",
            )


def _gate_tasks(
    editor: Editor,
    registry: dict[str, Any],
    phases: dict[str, str],
    risk_tasks: dict[str, list[str]],
) -> None:
    gates_for_phase = {
        "A": ["G0", "G1", "G2"],
        "B": ["G3", "G4"],
        "C": ["G5"],
        "D": ["G6"],
        "E": ["G7"],
        "F": ["G8", "G9", "G10"],
    }
    exit_existing = {
        "A": "bn-1grk",
        "B": "bn-1itt",
        "C": "bn-waj3",
        "D": "bn-3cpt",
        "E": "bn-o1oy",
        "F": "bn-39yh",
    }
    requirements = {item["id"]: item for item in registry["requirements"]}
    for phase, gate_ids in gates_for_phase.items():
        exit_goal = exit_existing[phase]
        editor.kind(exit_goal, "goal")
        editor.tag(
            exit_goal,
            "goal:manual",
            f"req:PHASE-{phase}",
            f"req:PHASE-{phase}-EXIT",
            *(f"req:{gate_id}" for gate_id in gate_ids),
        )
        integrated = editor.task(
            f"phase-exit:{phase}:integration",
            f"Phase {phase} integrated exit evidence and privileged decision package",
            exit_goal,
            {
                f"req:PHASE-{phase}",
                f"req:PHASE-{phase}-EXIT",
                *(f"req:{gate_id}" for gate_id in gate_ids),
                "integration",
                "quality",
            },
            _requirement_description(
                requirements[f"PHASE-{phase}-EXIT"],
                "Run the complete phase demonstration from clean state, bind every artifact to "
                "the source/toolchain/semantic epochs, summarize unsupported and inconclusive "
                "dimensions, and prepare the human/manual phase-exit decision. Agents may prepare "
                "the package but may not close the `goal:manual` phase.",
            ),
            size="m",
        )
        gate_subtree = editor.descendants(exit_goal) | {exit_goal}
        for leaf in sorted(editor.leaves(phases[phase]) - gate_subtree):
            editor.dep(leaf, integrated)
        for task in risk_tasks.get(phase, []):
            editor.dep(task, integrated)
        for other_phase, tasks in risk_tasks.items():
            if other_phase == phase:
                continue
            for task in tasks:
                editor.undep(task, integrated)
        for gate_id in gate_ids:
            criteria = [
                item
                for item in registry["requirements"]
                if item.get("parent") == gate_id
                and item["category"] == "gate-criterion"
                and item["status"] == "active"
            ]
            for requirement in criteria:
                criterion = editor.task(
                    f"gate:{requirement['id']}",
                    f"{requirement['id']} acceptance — {_short(requirement['summary'])}",
                    exit_goal,
                    {
                        f"req:{requirement['id']}",
                        f"req:{gate_id}",
                        f"req:PHASE-{phase}-EXIT",
                        f"req:PHASE-{phase}",
                        "gate",
                        "quality",
                    },
                    _requirement_description(
                        requirement,
                        "Independently rerun this criterion against the integrated phase artifact. "
                        "Retain raw evidence and negative controls; a feature-complete demo is not "
                        "a pass when this criterion is unsupported, stale, or inconclusive.",
                    ),
                    size="s",
                )
                editor.dep(integrated, criterion)


def _close_pr_dependencies(
    editor: Editor, pr_parents: dict[str, str], pr_exits: dict[str, str]
) -> None:
    for pr_id, parent in pr_parents.items():
        exit_task = pr_exits.get(pr_id)
        if not exit_task:
            continue
        for leaf in editor.leaves(parent):
            if leaf != exit_task:
                editor.dep(leaf, exit_task)


def _phase_sequence_dependencies(
    editor: Editor, phases: dict[str, str]
) -> None:
    """Containment is not sequencing: gate every later-phase leaf explicitly."""
    for previous, current in zip("ABCDE", "BCDEF", strict=True):
        blocker = phases[previous]
        for leaf in editor.leaves(phases[current]):
            editor.dep(blocker, leaf)


def _repair_traceability(
    editor: Editor,
    registry: dict[str, Any],
    phases: dict[str, str],
    root: str,
) -> None:
    state = traceability_state(registry, editor.bones)
    requirement_map = {item["id"]: item for item in registry["requirements"]}
    fallback_goals: dict[str, str] = {}
    for item in state["uncovered"] + state["nonleaf_only"]:
        req_id = item["requirement"]
        requirement = requirement_map[req_id]
        category = requirement["category"]
        if category in {"program", "phase", "pr", "gate"}:
            raise AssertionError(f"structural requirement unexpectedly uncovered: {req_id}")
        match = re.match(r"PHASE-([A-F])", req_id)
        phase = match.group(1) if match else "F"
        parent = phases[phase]
        if phase not in fallback_goals:
            fallback_goals[phase] = _category_goal(
                editor,
                f"traceability-closure:{phase}",
                f"Explicit orphan-requirement closure — Phase {phase}",
                parent,
                phase,
            )
        editor.task(
            f"traceability-closure:{req_id}",
            f"{req_id} explicit execution home — {_short(requirement['summary'])}",
            fallback_goals[phase],
            {f"req:{req_id}", f"req:PHASE-{phase}", "governance"},
            _requirement_description(
                requirement,
                "This obligation had no safe semantic owner after the structured grooming pass. "
                "Resolve it explicitly here, or move the Bone under the correct subsystem and "
                "record the dependency/evidence boundary before implementation begins.",
            ),
        )

    state = traceability_state(registry, editor.bones)
    for item in state["untraced"]:
        item_id = item["bone"]
        phase = next(
            (
                phase
                for phase, phase_id in phases.items()
                if item_id == phase_id or item_id in editor.descendants(phase_id)
            ),
            None,
        )
        editor.tag(item_id, f"req:PHASE-{phase}" if phase else "req:PROGRAM")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--apply",
        action="store_true",
        help="required acknowledgement: mutate the Bones event log through bn",
    )
    args = parser.parse_args()
    if not args.apply:
        raise SystemExit("refusing dry mutation: rerun with --apply after reviewing this script")

    registry = build_registry()
    requirements = {item["id"]: item for item in registry["requirements"]}
    editor = Editor()
    root = editor.by_title("Continuum 1.0 — Revision 3 implementation program")
    if not root:
        raise AssertionError("program root Bone not found")
    phases = _phase_index(editor)
    pr_parents = _pr_index(editor)
    expected_prs = {
        item["id"] for item in registry["requirements"] if item["category"] == "pr"
    }
    if set(pr_parents) != expected_prs:
        raise AssertionError(
            f"PR Bone mismatch: missing={sorted(expected_prs - set(pr_parents))}, "
            f"extra={sorted(set(pr_parents) - expected_prs)}"
        )
    phase_for: dict[str, str] = {}
    for phase, phase_id in phases.items():
        phase_for[phase_id] = phase
        for descendant in editor.descendants(phase_id):
            phase_for[descendant] = phase

    _baseline_trace(editor, root, phases, pr_parents)
    _normalize_goal_sizes(editor)
    for phase_id in phases.values():
        editor.tag(phase_id, "goal:manual")
    pr_exits = _map_prs(editor, registry, pr_parents, phase_for)
    _frontier_and_debt(editor, registry, phases)
    _corpus_tasks(editor, requirements, pr_parents)
    _experiment_tasks(editor, requirements, pr_parents, pr_exits)
    risk_tasks = _crosscutting_tasks(editor, registry, phases, root)
    _policy_tasks(editor, registry, phases, pr_parents)
    _map_phase_deliverables(editor, registry, phases)
    _close_pr_dependencies(editor, pr_parents, pr_exits)
    _gate_tasks(editor, registry, phases, risk_tasks)
    _phase_sequence_dependencies(editor, phases)
    _repair_traceability(editor, registry, phases, root)

    state = traceability_state(registry, editor.bones)
    result = {
        "active_bones": len(state["bones"]),
        "leaf_bones": len(state["leaves"]),
        "requirements": len(registry["requirements"]),
        "uncovered": len(state["uncovered"]),
        "nonleaf_only": len(state["nonleaf_only"]),
        "untraced": len(state["untraced"]),
        "unknown_labels": len(state["unknown_labels"]),
        "overbundled": len(state["overbundled"]),
        "dependencies": len(editor.links),
    }
    print(json.dumps(result, indent=2, sort_keys=True))
    if any(
        result[key]
        for key in ("uncovered", "nonleaf_only", "untraced", "unknown_labels", "overbundled")
    ):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
