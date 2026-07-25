#!/usr/bin/env python3
"""Scaffold for regenerating Continuum's corpus facts from a pinned local clone.

This script intentionally fails rather than guessing when upstream metadata or
README structure changes. It emits a machine-readable census for Tribunal
review; it does not overwrite the curated parity/category decisions.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
from pathlib import Path

EXPECTED_COMMIT = "91c22ea537853196ed1e03e9ad91693ec37642de"


def git(repo: Path, *args: str) -> str:
    return subprocess.check_output(["git", "-C", str(repo), *args], text=True).strip()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("clone", type=Path, help="local tlaplus/Examples clone")
    parser.add_argument("--allow-newer", action="store_true")
    args = parser.parse_args()
    repo = args.clone.resolve()
    commit = git(repo, "rev-parse", "HEAD")
    if commit != EXPECTED_COMMIT and not args.allow_newer:
        raise SystemExit(f"expected {EXPECTED_COMMIT}, found {commit}; pass --allow-newer only for drift review")

    manifests = sorted(repo.glob("specifications/*/manifest.json"))
    modules = models = proof_modules = pluscal_modules = 0
    features: dict[str, int] = {}
    for path in manifests:
        data = json.loads(path.read_text(encoding="utf-8"))
        for module in data.get("modules", []):
            modules += 1
            models += len(module.get("models", []))
            proof_modules += int("proof" in module)
            module_features = module.get("features", [])
            pluscal_modules += int("pluscal" in module_features)
            for feature in module_features:
                features[feature] = features.get(feature, 0) + 1

    cfg_tokens: dict[str, int] = {}
    for path in repo.glob("specifications/**/*.cfg"):
        text = path.read_text(encoding="utf-8", errors="replace")
        for token in ("SYMMETRY", "VIEW", "ALIAS", "CONSTRAINT", "ACTION_CONSTRAINT", "DEADLOCK"):
            if re.search(rf"(?m)^\s*{token}\b", text):
                cfg_tokens[token] = cfg_tokens.get(token, 0) + 1

    result = {
        "commit": commit,
        "manifest_count": len(manifests),
        "module_count": modules,
        "model_count": models,
        "proof_module_count": proof_modules,
        "pluscal_module_count": pluscal_modules,
        "module_features": dict(sorted(features.items())),
        "cfg_feature_files": dict(sorted(cfg_tokens.items())),
    }
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
