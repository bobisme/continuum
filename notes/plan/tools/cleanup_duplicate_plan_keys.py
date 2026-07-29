#!/usr/bin/env python3
"""Merge and soft-delete duplicate generated Bones by exact plan-key label."""
from __future__ import annotations

import json
import subprocess
from collections import defaultdict
from pathlib import Path
from typing import Any

from traceability import PROJECT_ROOT, project_bones

AGENT = "continuum-planner"


def run(arguments: list[str]) -> str:
    return subprocess.check_output(
        ["bn", "--agent", AGENT, "--format", "json", *arguments],
        cwd=PROJECT_ROOT,
        text=True,
        stderr=subprocess.STDOUT,
    )


def creation_order() -> dict[str, int]:
    order: dict[str, int] = {}
    ordinal = 0
    for path in sorted((PROJECT_ROOT / ".bones/events").glob("*.events")):
        for raw in path.read_text(encoding="utf-8").splitlines():
            if not raw or raw.startswith("#"):
                continue
            fields = raw.split("\t")
            if len(fields) >= 7 and fields[4] == "item.create":
                order[fields[5]] = ordinal
                ordinal += 1
    return order


def blocking_links() -> set[tuple[str, str]]:
    """Return (blocker, blocked) pairs."""
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


def plan_key(item: dict[str, Any]) -> str | None:
    keys = sorted(label for label in item["labels"] if label.startswith("plan-key:"))
    if len(keys) > 1:
        raise AssertionError(f"{item['id']}: multiple plan-key labels: {keys}")
    return keys[0] if keys else None


def chunks(values: list[str], size: int) -> list[list[str]]:
    return [values[index : index + size] for index in range(0, len(values), size)]


def main() -> None:
    bones = project_bones()
    active = {
        item_id: item
        for item_id, item in bones.items()
        if not item["deleted"] and item["state"] not in {"done", "closed", "archived"}
    }
    groups: dict[str, list[str]] = defaultdict(list)
    for item_id, item in active.items():
        key = plan_key(item)
        if key:
            groups[key].append(item_id)
    duplicate_groups = {
        key: item_ids for key, item_ids in groups.items() if len(item_ids) > 1
    }
    order = creation_order()
    canonical_map: dict[str, str] = {}
    for key, item_ids in duplicate_groups.items():
        item_ids.sort(key=lambda item_id: order[item_id])
        canonical = item_ids[0]
        for duplicate in item_ids[1:]:
            canonical_map[duplicate] = canonical

    def normalize_parent(parent: str | None) -> str | None:
        if not parent:
            return None
        parent = canonical_map.get(parent, parent)
        parent_item = active.get(parent)
        return plan_key(parent_item) if parent_item and plan_key(parent_item) else parent

    # Refuse to delete unless each duplicate has the same semantic payload.
    for duplicate, canonical in canonical_map.items():
        left, right = active[duplicate], active[canonical]
        comparable_left = {
            "title": left["title"],
            "description": left["description"],
            "kind": left["kind"],
            "parent": normalize_parent(left["parent"]),
        }
        comparable_right = {
            "title": right["title"],
            "description": right["description"],
            "kind": right["kind"],
            "parent": normalize_parent(right["parent"]),
        }
        if comparable_left != comparable_right:
            raise AssertionError(
                f"refusing non-identical duplicate {duplicate} -> {canonical}: "
                f"{comparable_left!r} != {comparable_right!r}"
            )
        missing_labels = sorted(left["labels"] - right["labels"])
        if missing_labels:
            run(["bone", "tag", canonical, *missing_labels])
            right["labels"].update(missing_labels)

    # Preserve any unique child accidentally attached to the duplicate goal.
    for item_id, item in active.items():
        duplicate_parent = item["parent"]
        if duplicate_parent not in canonical_map or item_id in canonical_map:
            continue
        canonical_parent = canonical_map[duplicate_parent]
        run(["bone", "move", item_id, "--parent", canonical_parent])
        item["parent"] = canonical_parent

    links = blocking_links()
    desired: set[tuple[str, str]] = set()
    for blocker, blocked in links:
        mapped = (
            canonical_map.get(blocker, blocker),
            canonical_map.get(blocked, blocked),
        )
        if mapped[0] != mapped[1]:
            desired.add(mapped)
    added = 0
    for blocker, blocked in sorted(desired - links):
        run(["dep", "add", blocker, "--blocks", blocked])
        added += 1

    duplicates = sorted(canonical_map, key=lambda item_id: order[item_id])
    for batch in chunks(duplicates, 40):
        run(
            [
                "bone",
                "delete",
                *batch,
                "--force",
                "--reason",
                "Duplicate generated by mixed-case plan-key lookup; dependencies merged to earliest canonical Bone.",
            ]
        )
    # Soft deletion intentionally preserves item history, but dependency links
    # to deleted generated copies would still pollute graph/PageRank analytics.
    refreshed = project_bones()
    active_key_owner: dict[str, str] = {}
    for item_id, item in refreshed.items():
        if item["deleted"] or item["state"] in {"done", "closed", "archived"}:
            continue
        key = plan_key(item)
        if key:
            active_key_owner[key] = item_id
    deleted_generated = {
        item_id
        for item_id, item in refreshed.items()
        if item["deleted"]
        and (key := plan_key(item)) is not None
        and key in active_key_owner
    }
    stale_links = sorted(
        (blocker, blocked)
        for blocker, blocked in blocking_links()
        if blocker in deleted_generated or blocked in deleted_generated
    )
    for blocker, blocked in stale_links:
        run(["dep", "rm", blocker, blocked])
    print(
        json.dumps(
            {
                "duplicate_keys": len(duplicate_groups),
                "soft_deleted": len(duplicates),
                "dependency_edges_merged": added,
                "deleted_dependency_edges_unlinked": len(stale_links),
            },
            indent=2,
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
