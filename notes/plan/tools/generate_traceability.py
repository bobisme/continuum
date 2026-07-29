#!/usr/bin/env python3
"""Regenerate the executable plan registry and Bones coverage report."""
from __future__ import annotations

import json

from traceability import REGISTRY_PATH, REPORT_PATH, build_registry, render_report


def main() -> None:
    registry = build_registry()
    REGISTRY_PATH.write_text(
        json.dumps(registry, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    REPORT_PATH.write_text(render_report(registry), encoding="utf-8")
    active = sum(item["status"] == "active" for item in registry["requirements"])
    print(
        json.dumps(
            {
                "registry": str(REGISTRY_PATH),
                "report": str(REPORT_PATH),
                "requirements": len(registry["requirements"]),
                "active": active,
            },
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
