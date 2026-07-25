#!/usr/bin/env python3
"""Validate the checked-in Continuum inventory for the pinned TLA+ Examples snapshot."""
from __future__ import annotations

import csv
import json
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PINNED_COMMIT = "91c22ea537853196ed1e03e9ad91693ec37642de"


def load_csv(name: str) -> list[dict[str, str]]:
    with (ROOT / name).open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def main() -> None:
    validated = load_csv("validated-examples.csv")
    other = load_csv("other-examples.csv")
    summary = json.loads((ROOT / "corpus-summary.json").read_text(encoding="utf-8"))

    assert len(validated) == 80, len(validated)
    assert len(other) == 39, len(other)
    assert len({row["id"] for row in validated}) == len(validated)
    assert len({row["source_path"] for row in validated}) == len(validated)
    assert all(row["source_commit"] == PINNED_COMMIT for row in validated)
    assert [row["id"] for row in validated] == [f"TV-{i:03d}" for i in range(1, 81)]
    assert [row["id"] for row in other] == [f"TO-{i:03d}" for i in range(1, 40)]
    assert {row["required_parity"] for row in validated} <= {f"P{i}" for i in range(6)}
    assert {row["porting_wave"] for row in validated} <= {str(i) for i in range(6)}

    computed = {
        "validated_count": len(validated),
        "other_count": len(other),
        "tlaps_or_proof": sum(row["tlaps_proof"] != "none" for row in validated),
        "pluscal_or_variant": sum(row["pluscal"] != "none" for row in validated),
        "tlc_models": sum(row["tlc_model"] == "True" for row in validated),
        "apalache": sum(row["apalache"] == "True" for row in validated),
        "categories": dict(sorted(Counter(row["category"] for row in validated).items())),
        "waves": dict(sorted(Counter(row["porting_wave"] for row in validated).items())),
    }
    for key, value in computed.items():
        assert summary[key] == value, (key, summary[key], value)
    assert summary["pinned_commit"] == PINNED_COMMIT
    print(json.dumps({"status": "pass", **computed}, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
