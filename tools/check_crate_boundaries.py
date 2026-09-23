#!/usr/bin/env python3
"""Mechanically enforce Continuum's crate boundaries.

Source of truth:

- `notes/plan/plan.md` §20 — the Revision 3 crate list and its dependency rules;
- `notes/plan/notes/START_HERE_IMPLEMENTATION.md` — the "Dependency islands"
  section and its four boundary rules;
- `notes/plan/docs/01_ARCHITECTURE.md` §13 — dependency constraints.

The four boundary rules stated in START_HERE, and how each is checked here:

1. "The certificate checker may not depend on search."
   -> RULE certificate-checker-not-search (transitive, exact). Search here is
      the engines, Forge, and the asupersync adapter (plan §20, INV-004).
2. "The model core may not depend on asupersync."
   -> RULE model-core-not-asupersync (transitive, exact).
3. "Adapters may not own semantic state."
   -> RULE adapters-are-sinks (mechanical shadow: an adapter that nothing
      imports cannot become a place semantic state accumulates; the semantic
      half of the rule stays a review obligation).
4. "Forge may not be imported by the verifier."
   -> RULE forge-not-imported-by-verifier (transitive, allowlisted consumers).

Plus:

5. "There is no `continuum-protocol` crate."  -> RULE no-protocol-crate.
6. Plan §20 kernel covenant: no async in the trusted checking base, no plugins
   or dynamic loading.  -> RULE kernel-is-synchronous (see the caveat below).
7. The workspace members are exactly plan §20's crate list.  -> RULE
   members-match-plan-20.  Adding a crate therefore requires updating §20, which
   is the intended friction.

Scope and honesty about limits:

- Edges are read from `cargo metadata --no-deps`, so the workspace-internal
  graph is exact and no network access or lockfile resolution is required.
- Normal and build dependencies are enforced; dev-dependencies are reported but
  not enforced, because a differential test may legitimately link an engine it
  would never ship against.
- Rule 6's external half checks *declared* dependencies only. Deep transitive
  auditing of third-party crates belongs to PR 9's kernel covenant tooling
  (<15,000 non-test lines, reproducible builds), not to this scaffold.

Stdlib only. Exit 0 when every rule holds, 1 otherwise.
"""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys
from collections.abc import Iterable

ROOT = pathlib.Path(__file__).resolve().parents[1]

# --- plan §20, expanded exactly ------------------------------------------------

PLAN_20_CRATES: tuple[str, ...] = (
    "continuum-workspace",
    "continuum-intent",
    "continuum-value",
    "continuum-model-core",
    "continuum-cml-syntax",
    "continuum-cml-elab",
    "continuum-cir",
    "continuum-observer",
    "continuum-refinement",
    "continuum-certificate",
    "continuum-kernel-core",
    "continuum-kernel-sat",
    "continuum-kernel-smt",
    "continuum-kernel-temporal",
    "continuum-evidence",
    "continuum-context",
    "continuum-semantic-diff",
    "continuum-repair",
    "continuum-incremental",
    "continuum-task",
    "continuum-debugger",
    "continuum-forge",
    "continuum-benchmark",
    "continuum-security",
    "continuum-asupersync",
    "continuum-effects-network",
    "continuum-effects-storage",
    "continuum-effects-time",
    "continuum-effects-process",
    "continuum-engine-reference",
    "continuum-engine-explicit",
    "continuum-engine-dpor",
    "continuum-engine-symbolic",
    "continuum-engine-liveness",
    "continuum-proof-client",
    "continuum-corpus",
    "continuumd",
    "continuum-cli",
    "continuum-lsp",
    "continuum-dap",
    "continuum-mcp",
    "continuum-sarif",
)

KERNEL = (
    "continuum-kernel-core",
    "continuum-kernel-sat",
    "continuum-kernel-smt",
    "continuum-kernel-temporal",
)
ENGINES = (
    "continuum-engine-reference",
    "continuum-engine-explicit",
    "continuum-engine-dpor",
    "continuum-engine-symbolic",
    "continuum-engine-liveness",
)
ADAPTERS = (
    "continuum-cli",
    "continuum-lsp",
    "continuum-dap",
    "continuum-mcp",
    "continuum-sarif",
)
# Certificate checking base: the kernel crates plus the certificate formats they
# check. Plan §20: "proof/certificate checker does not depend on search engines".
CHECKER = (*KERNEL, "continuum-certificate")

# Forge is a bottom-of-the-islands consumer. Only the daemon, the human CLI, and
# the benchmark harness may import it; every other crate is verifier-side.
FORGE_CONSUMERS = ("continuumd", "continuum-cli", "continuum-benchmark")

# Async runtimes and dynamic loaders the kernel covenant excludes (plan §20:
# "no async, no unsafe, no plugins or dynamic loading").
FORBIDDEN_IN_KERNEL_EXTERNAL = (
    "tokio",
    "async-std",
    "smol",
    "futures",
    "futures-util",
    "futures-executor",
    "async-trait",
    "libloading",
    "dlopen2",
    "inventory",
)


class Violation(str):
    """A single rule failure, rendered as a human-readable line."""


def cargo_metadata() -> dict:
    proc = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        raise SystemExit(f"cargo metadata failed with exit {proc.returncode}")
    return json.loads(proc.stdout)


def build_graph(meta: dict) -> tuple[dict[str, set[str]], dict[str, set[str]], set[str]]:
    """Return (enforced_edges, dev_edges, members).

    `enforced_edges[a]` holds the normal and build dependencies of `a`;
    `dev_edges[a]` holds its dev-dependencies, which are reported only.
    """
    members = {pkg["name"] for pkg in meta["packages"]}
    enforced: dict[str, set[str]] = {name: set() for name in members}
    dev: dict[str, set[str]] = {name: set() for name in members}
    for pkg in meta["packages"]:
        for dep in pkg["dependencies"]:
            kind = dep.get("kind")  # None == normal, "dev", "build"
            bucket = dev if kind == "dev" else enforced
            bucket[pkg["name"]].add(dep["name"])
    return enforced, dev, members


def closure(edges: dict[str, set[str]], start: str, members: set[str]) -> dict[str, list[str]]:
    """Transitive workspace-internal closure of `start`, with a witness path."""
    paths: dict[str, list[str]] = {}
    stack = [(start, [start])]
    while stack:
        node, path = stack.pop()
        for nxt in sorted(edges.get(node, ())):
            if nxt not in members or nxt in paths:
                continue
            nxt_path = [*path, nxt]
            paths[nxt] = nxt_path
            stack.append((nxt, nxt_path))
    return paths


def fmt(path: Iterable[str]) -> str:
    return " -> ".join(path)


def evaluate(enforced: dict[str, set[str]], members: set[str]) -> list[Violation]:
    """Apply every boundary rule to a workspace-internal dependency graph."""
    violations: list[Violation] = []

    # RULE members-match-plan-20
    expected = set(PLAN_20_CRATES)
    missing = sorted(expected - members)
    extra = sorted(members - expected)
    for name in missing:
        violations.append(
            Violation(f"[members-match-plan-20] plan §20 crate is missing from the workspace: {name}")
        )
    for name in extra:
        violations.append(
            Violation(
                f"[members-match-plan-20] workspace member is not in plan §20: {name} "
                "(add it to §20 first, or remove the crate)"
            )
        )

    # RULE no-protocol-crate
    if "continuum-protocol" in members:
        violations.append(
            Violation(
                "[no-protocol-crate] `continuum-protocol` exists; the native protocol "
                "lives in `continuumd` (START_HERE_IMPLEMENTATION.md, Dependency islands)"
            )
        )
    if "continuumd" not in members:
        violations.append(
            Violation("[no-protocol-crate] `continuumd`, which owns the native protocol, is missing")
        )

    closures = {name: closure(enforced, name, members) for name in sorted(members)}

    # RULE certificate-checker-not-search
    #
    # Plan §20 (AGENTS.md, INV-004): the checking base may not depend on
    # `continuum-engine-*`, `continuum-forge`, or `continuum-asupersync`. The
    # asupersync prong was missing for `continuum-certificate` until bn-2270's
    # C023 audit (`tools/check_triptych_independence.py`, rule gate-coverage)
    # found that an edge `continuum-certificate -> continuum-asupersync` passed
    # this gate: `kernel-is-synchronous` covers the four kernel crates only.
    search = set(ENGINES) | {"continuum-forge", "continuum-asupersync"}
    for src in CHECKER:
        for target, path in sorted(closures.get(src, {}).items()):
            if target in search:
                violations.append(
                    Violation(
                        f"[certificate-checker-not-search] {fmt(path)} — the certificate "
                        "checker may not depend on search (plan §20; docs/01 §13)"
                    )
                )

    # RULE model-core-not-asupersync
    for target, path in sorted(closures.get("continuum-model-core", {}).items()):
        if target == "continuum-asupersync":
            violations.append(
                Violation(
                    f"[model-core-not-asupersync] {fmt(path)} — the model core may not "
                    "depend on asupersync (plan §20; docs/01 §13)"
                )
            )

    # RULE kernel-is-synchronous (external, declared deps only)
    for src in KERNEL:
        for dep in sorted(enforced.get(src, ())):
            if dep in FORBIDDEN_IN_KERNEL_EXTERNAL:
                violations.append(
                    Violation(
                        f"[kernel-is-synchronous] {src} -> {dep} — the kernel covenant "
                        "forbids async runtimes, plugins, and dynamic loading (plan §20)"
                    )
                )
        for target, path in sorted(closures.get(src, {}).items()):
            if target == "continuum-asupersync":
                violations.append(
                    Violation(
                        f"[kernel-is-synchronous] {fmt(path)} — the trusted checking base "
                        "is synchronous by covenant (plan §20)"
                    )
                )

    # RULE forge-not-imported-by-verifier
    for src in sorted(members):
        if src == "continuum-forge" or src in FORGE_CONSUMERS:
            continue
        path = closures.get(src, {}).get("continuum-forge")
        if path:
            violations.append(
                Violation(
                    f"[forge-not-imported-by-verifier] {fmt(path)} — Forge depends on "
                    "verifier interfaces, never vice versa (plan §20; START_HERE)"
                )
            )

    # RULE adapters-are-sinks
    for src in sorted(members):
        for adapter in ADAPTERS:
            if src == adapter:
                continue
            path = closures.get(src, {}).get(adapter)
            if path:
                violations.append(
                    Violation(
                        f"[adapters-are-sinks] {fmt(path)} — adapters do not own semantic "
                        "state and must stay sinks in the workspace graph (plan §20; "
                        "START_HERE)"
                    )
                )

    return violations


# --- self-test -----------------------------------------------------------------
#
# Every crate is dependency-free at PR-1 except `continuum-cml-elab ->
# continuum-cml-syntax`, so the rules above would pass vacuously. The self-test
# injects each forbidden edge into a clean graph and asserts it is caught. Run it
# with `--self-test`; `just check` runs it before the real check.

SELF_TEST_CASES: tuple[tuple[str, str, str, str], ...] = (
    ("certificate-checker-not-search", "continuum-certificate", "continuum-engine-explicit", "direct"),
    ("certificate-checker-not-search", "continuum-kernel-core", "continuum-engine-dpor", "direct"),
    ("certificate-checker-not-search", "continuum-kernel-smt", "continuum-forge", "direct"),
    ("certificate-checker-not-search", "continuum-certificate", "continuum-asupersync", "direct"),
    ("model-core-not-asupersync", "continuum-model-core", "continuum-asupersync", "direct"),
    ("kernel-is-synchronous", "continuum-kernel-temporal", "continuum-asupersync", "direct"),
    ("kernel-is-synchronous", "continuum-kernel-core", "tokio", "external"),
    ("forge-not-imported-by-verifier", "continuum-repair", "continuum-forge", "direct"),
    ("forge-not-imported-by-verifier", "continuum-model-core", "continuum-forge", "direct"),
    ("adapters-are-sinks", "continuumd", "continuum-cli", "direct"),
    ("adapters-are-sinks", "continuum-evidence", "continuum-sarif", "direct"),
    # Transitivity: a forbidden target reached through an innocent hop.
    ("certificate-checker-not-search", "continuum-certificate", "continuum-engine-dpor", "via:continuum-value"),
    ("forge-not-imported-by-verifier", "continuum-context", "continuum-forge", "via:continuum-debugger"),
)


def self_test() -> int:
    members = set(PLAN_20_CRATES)
    clean = {name: set() for name in members}
    if evaluate({k: set(v) for k, v in clean.items()}, members):
        print(json.dumps({"self_test": "fail", "reason": "clean graph reported violations"}))
        return 1

    failures: list[str] = []
    for rule, src, dst, mode in SELF_TEST_CASES:
        graph = {k: set(v) for k, v in clean.items()}
        if mode.startswith("via:"):
            hop = mode.split(":", 1)[1]
            graph[src].add(hop)
            graph[hop].add(dst)
        else:
            graph[src].add(dst)
        found = [v for v in evaluate(graph, members) if v.startswith(f"[{rule}]")]
        if not found:
            failures.append(f"{rule}: {src} -> {dst} ({mode}) was not caught")

    # A missing plan §20 crate and an unlisted extra crate must both be caught.
    short = {k: set(v) for k, v in clean.items() if k != "continuum-corpus"}
    if not [v for v in evaluate(short, set(short)) if v.startswith("[members-match-plan-20]")]:
        failures.append("members-match-plan-20: a missing crate was not caught")
    wide = {**{k: set(v) for k, v in clean.items()}, "continuum-protocol": set()}
    wide_v = evaluate(wide, set(wide))
    if not [v for v in wide_v if v.startswith("[no-protocol-crate]")]:
        failures.append("no-protocol-crate: `continuum-protocol` was not caught")
    if not [v for v in wide_v if v.startswith("[members-match-plan-20]")]:
        failures.append("members-match-plan-20: an unlisted crate was not caught")

    print(
        json.dumps(
            {
                "self_test": "fail" if failures else "pass",
                "cases": len(SELF_TEST_CASES) + 3,
                "failures": failures,
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 1 if failures else 0


def main() -> int:
    if "--self-test" in sys.argv[1:]:
        return self_test()

    meta = cargo_metadata()
    enforced, dev, members = build_graph(meta)
    violations = evaluate(enforced, members)

    internal_edges = sum(len(v & members) for v in enforced.values())
    dev_edges = sorted(
        f"{src} -> {d}" for src, ds in dev.items() for d in sorted(ds & members)
    )

    report = {
        "members": len(members),
        "plan_20_crates": len(PLAN_20_CRATES),
        "workspace_internal_edges": internal_edges,
        "dev_dependency_edges_not_enforced": dev_edges,
        "rules": [
            "members-match-plan-20",
            "no-protocol-crate",
            "certificate-checker-not-search",
            "model-core-not-asupersync",
            "kernel-is-synchronous",
            "forge-not-imported-by-verifier",
            "adapters-are-sinks",
        ],
        "violations": list(violations),
        "status": "fail" if violations else "pass",
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if violations else 0


if __name__ == "__main__":
    raise SystemExit(main())
