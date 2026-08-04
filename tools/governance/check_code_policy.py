#!/usr/bin/env python3
"""Mechanically enforce the GOV §1 repository constitution.

Source of truth:

- `notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md` §1 "Repository constitution"
  — seven code-policy bullets and five semantic-policy bullets;
- `notes/plan/notes/PLAN_REQUIREMENTS.json` — the same twelve bullets carrying
  the stable ids `GOV-1-01` … `GOV-1-12`, which are the keys of the evidence
  file this script writes.

One rule per requirement id, each rule a set of named sub-checks:

    GOV-1-01  toolchain-pinned                  Rust edition/toolchain pinned
    GOV-1-02  unsafe-forbidden                  `unsafe` forbidden by default
    GOV-1-03  deterministic-collections         deterministic collections in semantic paths
    GOV-1-04  no-ambient-time-rng               no ambient time/RNG in core
    GOV-1-05  no-platform-hashing               no platform-dependent hashing in canonical formats
    GOV-1-06  no-network-in-checker             no network access in certificate checking
    GOV-1-07  dependency-rationale              dependency additions require rationale and TCB class
    GOV-1-08  semantic-change-adr               semantic changes require ADR
    GOV-1-09  epoch-discipline                  breaking changes increment the semantic epoch
    GOV-1-10  pack-operation-contract           every pack operation has a normative contract
    GOV-1-11  reference-precedes-optimization   reference semantics precedes optimization
    GOV-1-12  ambiguity-is-an-error             ambiguous behavior is an error

Scope and honesty about limits
------------------------------

`GOV-1-01` … `GOV-1-07` are code policy and are enforced directly against the
manifests and sources they talk about. `GOV-1-08` … `GOV-1-12` are *process*
obligations: no run of any program over one revision can observe "this change
was breaking" or "this optimization was written after its reference". Each of
those rules therefore enforces a checkable core — an artifact-level invariant
whose violation would be a necessary consequence of breaking the obligation —
and every such rule records an explicit `boundary` string in the evidence file
naming what it does *not* see.

For `GOV-1-08` and `GOV-1-09` the missing half is no longer missing: it is
`tools/governance/check_revision_delta.py`, which reads *two* revisions (a base,
by default the merge base with the trunk, and a head) and enforces the delta
rules `semantic-change-adr-delta` and `epoch-discipline-delta`. This file keeps
enforcing the state; that one enforces the change. Both are wired into the
`governance` recipe, and each requirement's `delta_gate` field in the evidence
file names its two-revision half.

Self-test
---------

The workspace deliberately satisfies every rule here, so a naive run passes
vacuously. `--self-test` replays each violating fixture in
`tools/governance/fixtures/code/` through the same rule functions and fails if
any fixture goes uncaught. Fixtures are inert data — a fixture never reaches
`cargo`, `rustc`, or the dossier validator; it is read as text, overlaid onto an
in-memory view of the repository, and thrown away.

Stdlib only. Exit 0 when every rule holds, 1 otherwise.

Usage:

    python3 tools/governance/check_code_policy.py --self-test
    python3 tools/governance/check_code_policy.py
    python3 tools/governance/check_code_policy.py --evidence tools/governance/evidence/gov-1.json
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import tomllib
from collections.abc import Iterable, Iterator
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GOVERNANCE = "tools/governance"
FIXTURE_DIR = f"{GOVERNANCE}/fixtures/code"
RATIONALE_PATH = f"{GOVERNANCE}/dependency-rationale.toml"

CONSTITUTION = "notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md"
PLAN = "notes/plan/plan.md"
SCHEMA_DIR = "notes/plan/schemas"
ADR_DIR = "notes/plan/adr"
RFC_DIR = "notes/plan/rfcs"

# Directories never walked: build output, VCS internals, and the maw admin tree.
PRUNED_DIRS = frozenset(
    {".git", ".maw", ".manifold", "repo.git", "target", ".lake", "__pycache__", ".venv", "node_modules"}
)
# Only these top-level entries are indexed; no rule reads anything else.
INDEXED_ROOTS = ("crates", "notes", "tools")


# ============================================================================
# Repository view
# ============================================================================


class Tree:
    """A read-only view of the repository, optionally with fixture overlays.

    Every rule reads the repository *only* through this class, so the exact code
    path that runs against the real tree runs against a fixture tree too: a
    fixture cannot be caught by a check the real run does not perform.
    """

    def __init__(
        self,
        root: Path,
        overlay: dict[str, str] | None = None,
        removed: Iterable[str] = (),
    ) -> None:
        self.root = root
        self.overlay = dict(overlay or {})
        self.removed = frozenset(removed)
        self._index: tuple[str, ...] | None = None

    # -- construction ------------------------------------------------------

    def with_changes(self, overlay: dict[str, str], removed: Iterable[str] = ()) -> Tree:
        merged = {**self.overlay, **overlay}
        return Tree(self.root, merged, frozenset(self.removed) | frozenset(removed))

    # -- file access -------------------------------------------------------

    def _real_index(self) -> tuple[str, ...]:
        if self._index is None:
            found: list[str] = []
            for name in sorted(os.listdir(self.root)):
                path = self.root / name
                if path.is_file():
                    found.append(name)
            for top in INDEXED_ROOTS:
                base = self.root / top
                if not base.is_dir():
                    continue
                for dirpath, dirnames, filenames in os.walk(base):
                    dirnames[:] = sorted(d for d in dirnames if d not in PRUNED_DIRS)
                    rel_dir = Path(dirpath).relative_to(self.root).as_posix()
                    for fname in sorted(filenames):
                        found.append(f"{rel_dir}/{fname}")
            self._index = tuple(sorted(found))
        return self._index

    def exists(self, rel: str) -> bool:
        if rel in self.removed:
            return False
        if rel in self.overlay:
            return True
        return (self.root / rel).is_file()

    def read_text(self, rel: str) -> str | None:
        if rel in self.removed:
            return None
        if rel in self.overlay:
            return self.overlay[rel]
        path = self.root / rel
        if not path.is_file():
            return None
        return path.read_text(encoding="utf-8")

    def read_json(self, rel: str) -> object | None:
        text = self.read_text(rel)
        if text is None:
            return None
        try:
            return json.loads(text)
        except json.JSONDecodeError:
            return None

    def read_toml(self, rel: str) -> dict | None:
        text = self.read_text(rel)
        if text is None:
            return None
        try:
            return tomllib.loads(text)
        except tomllib.TOMLDecodeError:
            return None

    def glob(self, pattern: str) -> list[str]:
        matcher = _glob_re(pattern)
        candidates = set(self._real_index()) | set(self.overlay)
        return sorted(p for p in candidates if p not in self.removed and matcher.fullmatch(p))


def _glob_re(pattern: str) -> re.Pattern[str]:
    """Translate a posix glob (`*`, `**`, `?`) into a full-match regex."""
    out: list[str] = []
    i = 0
    while i < len(pattern):
        char = pattern[i]
        if pattern.startswith("**/", i):
            out.append("(?:[^/]+/)*")
            i += 3
        elif pattern.startswith("**", i):
            out.append(".*")
            i += 2
        elif char == "*":
            out.append("[^/]*")
            i += 1
        elif char == "?":
            out.append("[^/]")
            i += 1
        elif char == "[":
            close = pattern.find("]", i + 1)
            if close == -1:
                out.append(re.escape(char))
                i += 1
            else:
                body = pattern[i + 1 : close]
                body = ("^" + body[1:]) if body.startswith("!") else body
                out.append(f"[{body}]")
                i = close + 1
        else:
            out.append(re.escape(char))
            i += 1
    return re.compile("".join(out))


# ============================================================================
# Violations
# ============================================================================


@dataclass(frozen=True, order=True)
class Violation:
    rule: str
    check: str
    message: str

    def render(self) -> str:
        return f"[{self.rule}/{self.check}] {self.message}"


# ============================================================================
# Rust source handling
# ============================================================================


def strip_rust_comments(src: str) -> str:
    """Blank out comments while preserving offsets and line structure.

    Symbol scans run over the result, so a doc comment that *names* a forbidden
    construct in order to forbid it (`continuum-value` does exactly that) is not
    mistaken for a use of it.
    """
    out: list[str] = []
    i, n = 0, len(src)
    while i < n:
        char = src[i]
        if char == "/" and i + 1 < n and src[i + 1] == "/":
            end = src.find("\n", i)
            end = n if end == -1 else end
            out.append(" " * (end - i))
            i = end
        elif char == "/" and i + 1 < n and src[i + 1] == "*":
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth += 1
                    j += 2
                elif src.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            out.append("".join(c if c == "\n" else " " for c in src[i:j]))
            i = j
        elif char == "r" and (m := _RAW_STRING_OPEN.match(src, i)):
            close = '"' + "#" * (len(m.group(1)))
            end = src.find(close, m.end())
            end = n if end == -1 else end + len(close)
            out.append(src[i:end])
            i = end
        elif char == '"':
            j = i + 1
            while j < n:
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == '"':
                    j += 1
                    break
                j += 1
            out.append(src[i:j])
            i = j
        elif char == "'" and (m := _CHAR_LITERAL.match(src, i)):
            out.append(m.group(0))
            i = m.end()
        else:
            out.append(char)
            i += 1
    return "".join(out)


_RAW_STRING_OPEN = re.compile(r'r(#*)"')
_CHAR_LITERAL = re.compile(r"'(?:\\.|[^\\'])'")

WAIVER_RE = re.compile(r"//\s*continuum:allow\(([a-z0-9-]+)\)\s*:\s*(\S.*)$")


@dataclass(frozen=True)
class Waiver:
    path: str
    line: int
    check: str
    reason: str


def collect_waivers(path: str, raw: str) -> tuple[dict[str, set[int]], list[Waiver], list[str]]:
    """Parse inline `// continuum:allow(<check>): <reason>` waivers.

    A waiver covers its own line and the line immediately below it. A waiver
    with a reason shorter than 12 characters is not a waiver — it is reported.
    """
    covered: dict[str, set[int]] = {}
    granted: list[Waiver] = []
    problems: list[str] = []
    for lineno, line in enumerate(raw.splitlines(), start=1):
        match = WAIVER_RE.search(line)
        if not match:
            continue
        check, reason = match.group(1), match.group(2).strip()
        if len(reason) < 12:
            problems.append(f"{path}:{lineno} waiver for `{check}` has no usable reason: {reason!r}")
            continue
        covered.setdefault(check, set()).update({lineno, lineno + 1})
        granted.append(Waiver(path, lineno, check, reason))
    return covered, granted, problems


@dataclass
class SourceScan:
    """Result of scanning a set of Rust sources for forbidden symbols."""

    hits: list[Violation] = field(default_factory=list)
    waivers: list[Waiver] = field(default_factory=list)
    files: int = 0


def scan_sources(
    tree: Tree,
    paths: Iterable[str],
    rule: str,
    check: str,
    patterns: dict[str, str],
    why: str,
) -> SourceScan:
    """Report every occurrence of `patterns` outside comments in `paths`."""
    scan = SourceScan()
    for rel in sorted(paths):
        raw = tree.read_text(rel)
        if raw is None:
            continue
        scan.files += 1
        covered, granted, problems = collect_waivers(rel, raw)
        scan.waivers.extend(w for w in granted if w.check == check)
        for problem in problems:
            scan.hits.append(Violation(rule, check, problem))
        waived_lines = covered.get(check, set())
        code = strip_rust_comments(raw)
        for label, pattern in sorted(patterns.items()):
            for match in re.finditer(pattern, code):
                lineno = code.count("\n", 0, match.start()) + 1
                if lineno in waived_lines:
                    continue
                scan.hits.append(
                    Violation(rule, check, f"{rel}:{lineno} uses `{label}` — {why}")
                )
    return scan


# ============================================================================
# Workspace model
# ============================================================================

DEP_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")
DEP_KIND = {"dependencies": "normal", "dev-dependencies": "dev", "build-dependencies": "build"}


@dataclass(frozen=True)
class Edge:
    consumer: str
    dependency: str
    kind: str


@dataclass
class Workspace:
    root_manifest: dict
    crates: dict[str, dict]  # crate name -> parsed manifest
    manifest_path: dict[str, str]  # crate name -> repo-relative Cargo.toml
    edges: tuple[Edge, ...]
    problems: tuple[str, ...]

    @property
    def members(self) -> frozenset[str]:
        return frozenset(self.crates)

    def deps_of(self, crate: str, kinds: Iterable[str] = ("normal", "build")) -> set[str]:
        wanted = set(kinds)
        return {e.dependency for e in self.edges if e.consumer == crate and e.kind in wanted}

    def closure(self, start: str, kinds: Iterable[str] = ("normal", "build")) -> dict[str, list[str]]:
        """Transitive workspace-internal closure of `start`, with witness paths."""
        paths: dict[str, list[str]] = {}
        stack: list[tuple[str, list[str]]] = [(start, [start])]
        while stack:
            node, path = stack.pop()
            for nxt in sorted(self.deps_of(node, kinds)):
                if nxt not in self.members or nxt in paths:
                    continue
                nxt_path = [*path, nxt]
                paths[nxt] = nxt_path
                stack.append((nxt, nxt_path))
        return paths

    def external_deps(self, crate: str, kinds: Iterable[str] = ("normal", "build")) -> set[str]:
        return {d for d in self.deps_of(crate, kinds) if d not in self.members}


def _dep_names(table: object) -> list[str]:
    if not isinstance(table, dict):
        return []
    names: list[str] = []
    for key, value in table.items():
        # `foo = { package = "bar" }` renames: the real crate is `package`.
        if isinstance(value, dict) and isinstance(value.get("package"), str):
            names.append(value["package"])
        else:
            names.append(key)
    return names


def load_workspace(tree: Tree) -> Workspace:
    problems: list[str] = []
    root_manifest = tree.read_toml("Cargo.toml")
    if root_manifest is None:
        problems.append("Cargo.toml is missing or is not valid TOML")
        root_manifest = {}

    crates: dict[str, dict] = {}
    manifest_path: dict[str, str] = {}
    edges: list[Edge] = []
    for rel in tree.glob("crates/*/Cargo.toml"):
        manifest = tree.read_toml(rel)
        if manifest is None:
            problems.append(f"{rel} is not valid TOML")
            continue
        name = manifest.get("package", {}).get("name")
        if not isinstance(name, str):
            problems.append(f"{rel} declares no [package] name")
            continue
        crates[name] = manifest
        manifest_path[name] = rel
        for table, kind in DEP_KIND.items():
            for dep in _dep_names(manifest.get(table)):
                edges.append(Edge(name, dep, kind))
        # `[target.'cfg(...)'.dependencies]` counts too.
        for target in (manifest.get("target") or {}).values():
            if not isinstance(target, dict):
                continue
            for table, kind in DEP_KIND.items():
                for dep in _dep_names(target.get(table)):
                    edges.append(Edge(name, dep, kind))

    return Workspace(
        root_manifest=root_manifest,
        crates=crates,
        manifest_path=manifest_path,
        edges=tuple(sorted(set(edges), key=lambda e: (e.consumer, e.dependency, e.kind))),
        problems=tuple(problems),
    )


PLAN_20_FENCE = re.compile(r"^## 20\. Crate and service architecture\s*\n+```text\n(.*?)\n```", re.S | re.M)
BRACE_EXPANSION = re.compile(r"^(.*)\{([^}]*)\}(.*)$")


def plan_20_crates(tree: Tree) -> tuple[tuple[str, ...], list[str]]:
    """The plan §20 crate table, expanded. Adding a crate means editing §20."""
    text = tree.read_text(PLAN)
    if text is None:
        return (), [f"{PLAN} is missing"]
    match = PLAN_20_FENCE.search(text)
    if not match:
        return (), [f"{PLAN} §20 has no `text` fence naming the crate list"]
    names: list[str] = []
    for line in match.group(1).splitlines():
        line = line.strip()
        if not line:
            continue
        expansion = BRACE_EXPANSION.match(line)
        if expansion:
            head, body, tail = expansion.groups()
            names.extend(f"{head}{part.strip()}{tail}" for part in body.split(","))
        else:
            names.append(line)
    return tuple(names), []


# ---------------------------------------------------------------------------
# Crate tiers.
#
# Plan §20 is the crate list; it does not itself partition the list by policy
# class, so this table does — once, in the open, with the classification rule
# stated. `tier-partition-covers-plan-20` (under GOV-1-03) fails if §20 and this
# table ever disagree, so a new crate cannot be added without being classified.
#
#   SEMANTIC_CORE — computes, encodes, compares, or checks a semantic artifact.
#     docs/19 §7's determinism matrix ("different hash seeds where internal
#     structures allow […] semantic artifacts must be identical") applies to
#     every one of them, which is what makes nondeterministic collections,
#     ambient clocks, and ambient RNG policy violations here. The search engines
#     are included deliberately: a seed-dependent frontier yields a
#     seed-dependent counterexample.
#
#   BOUNDARY — the crates whose *job* is to own an ambient resource behind an
#     explicit capability (INV-005, ADR-0003): the four effect packs, the
#     execution substrate, Forge's sandbox, the benchmark harness, the security
#     crate, and the proof-service client. Time, randomness, and sockets are
#     expected here; they are excluded from GOV-1-03/04/05 by design, not by
#     oversight, and GOV-1-06 keeps them out of the certificate checker.
#
#   ADAPTERS — protocol surfaces that own no semantic state (plan §20). They are
#     excluded for the same reason: an adapter may legitimately timestamp a log
#     line or index open documents by hash.
# ---------------------------------------------------------------------------

SEMANTIC_CORE = frozenset(
    {
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
        "continuum-corpus",
        "continuum-engine-reference",
        "continuum-engine-explicit",
        "continuum-engine-dpor",
        "continuum-engine-symbolic",
        "continuum-engine-liveness",
    }
)
BOUNDARY = frozenset(
    {
        "continuum-forge",
        "continuum-benchmark",
        "continuum-security",
        "continuum-asupersync",
        "continuum-effects-network",
        "continuum-effects-storage",
        "continuum-effects-time",
        "continuum-effects-process",
        "continuum-proof-client",
    }
)
ADAPTERS = frozenset(
    {"continuumd", "continuum-cli", "continuum-lsp", "continuum-dap", "continuum-mcp", "continuum-sarif"}
)

KERNEL = ("continuum-kernel-core", "continuum-kernel-sat", "continuum-kernel-smt", "continuum-kernel-temporal")
# Plan §20: "proof/certificate checker does not depend on search engines". The
# certificate-checking base is the four kernel crates plus the certificate
# formats they check from wire form.
CHECKER = frozenset({*KERNEL, "continuum-certificate"})

ENGINES_OPTIMIZED = (
    "continuum-engine-explicit",
    "continuum-engine-dpor",
    "continuum-engine-symbolic",
    "continuum-engine-liveness",
)
ENGINE_REFERENCE = "continuum-engine-reference"


def core_sources(tree: Tree, crates: Iterable[str]) -> list[str]:
    """`src/**/*.rs` of the named crates. Tests are not semantic artifacts."""
    out: list[str] = []
    for crate in sorted(set(crates)):
        out.extend(tree.glob(f"crates/{crate}/src/**/*.rs"))
        out.extend(tree.glob(f"crates/{crate}/src/*.rs"))
    return sorted(set(out))


# ============================================================================
# GOV-1-01 — Rust edition/toolchain pinned
# ============================================================================

EXACT_CHANNEL = re.compile(r"^\d+\.\d+\.\d+$")


def rule_toolchain_pinned(tree: Tree) -> list[Violation]:
    rule = "toolchain-pinned"
    out: list[Violation] = []

    text = tree.read_text("rust-toolchain.toml")
    if text is None:
        out.append(
            Violation(
                rule,
                "toolchain-file-present",
                "rust-toolchain.toml is absent; the toolchain would be whatever "
                "`rustup` resolves on the day of the build (docs/12 §1; INV-014)",
            )
        )
    else:
        manifest = tree.read_toml("rust-toolchain.toml")
        channel = (manifest or {}).get("toolchain", {}).get("channel")
        if not isinstance(channel, str):
            out.append(
                Violation(rule, "toolchain-channel-exact", "rust-toolchain.toml declares no [toolchain] channel")
            )
        elif not EXACT_CHANNEL.fullmatch(channel):
            out.append(
                Violation(
                    rule,
                    "toolchain-channel-exact",
                    f"rust-toolchain.toml pins channel {channel!r}, which is not an exact "
                    "`major.minor.patch` version; a floating channel makes the toolchain "
                    "identity a function of install date (INV-014)",
                )
            )

    workspace = load_workspace(tree)
    package = workspace.root_manifest.get("workspace", {}).get("package", {})
    if not isinstance(package.get("edition"), str):
        out.append(
            Violation(rule, "workspace-edition-set", "Cargo.toml [workspace.package] declares no `edition`")
        )
    if not isinstance(package.get("rust-version"), str):
        out.append(
            Violation(
                rule,
                "workspace-edition-set",
                "Cargo.toml [workspace.package] declares no `rust-version` compatibility floor",
            )
        )

    for crate, manifest in sorted(workspace.crates.items()):
        edition = manifest.get("package", {}).get("edition")
        if edition is None:
            out.append(
                Violation(
                    rule,
                    "crate-inherits-edition",
                    f"{workspace.manifest_path[crate]} declares no edition at all",
                )
            )
        elif edition != {"workspace": True}:
            out.append(
                Violation(
                    rule,
                    "crate-inherits-edition",
                    f"{workspace.manifest_path[crate]} sets its own edition ({edition!r}) instead of "
                    "`edition.workspace = true`; one workspace, one edition",
                )
            )
    return out


# ============================================================================
# GOV-1-02 — `unsafe` forbidden by default
# ============================================================================

ALLOW_UNSAFE_ATTR = re.compile(r"#!?\[[^\]]*\b(?:allow|expect|warn|deny)\s*\(\s*unsafe_code\b")
UNSAFE_TOKEN = re.compile(r"\bunsafe\b")


def rule_unsafe_forbidden(tree: Tree) -> list[Violation]:
    rule = "unsafe-forbidden"
    out: list[Violation] = []
    workspace = load_workspace(tree)

    lints = workspace.root_manifest.get("workspace", {}).get("lints", {}).get("rust", {})
    level = lints.get("unsafe_code")
    if level != "forbid":
        out.append(
            Violation(
                rule,
                "workspace-lint-forbids-unsafe",
                f"Cargo.toml [workspace.lints.rust] unsafe_code is {level!r}, not \"forbid\" "
                "(docs/12 §1 \"`unsafe` forbidden by default\"; docs/03 checker architecture)",
            )
        )

    for crate, manifest in sorted(workspace.crates.items()):
        path = workspace.manifest_path[crate]
        crate_lints = manifest.get("lints", {})
        if crate_lints.get("workspace") is not True:
            out.append(
                Violation(
                    rule,
                    "crate-opts-into-workspace-lints",
                    f"{path} does not set `[lints] workspace = true`, so the workspace "
                    "`unsafe_code = \"forbid\"` does not reach it",
                )
            )
        override = crate_lints.get("rust", {})
        if isinstance(override, dict) and "unsafe_code" in override:
            out.append(
                Violation(
                    rule,
                    "no-crate-level-override",
                    f"{path} re-declares unsafe_code = {override['unsafe_code']!r} at crate level; "
                    "no crate-level override exists (Cargo.toml, [workspace.lints.rust] comment)",
                )
            )

    sources = sorted(set(tree.glob("crates/**/*.rs")))
    for rel in sources:
        raw = tree.read_text(rel)
        if raw is None:
            continue
        code = strip_rust_comments(raw)
        for match in ALLOW_UNSAFE_ATTR.finditer(code):
            lineno = code.count("\n", 0, match.start()) + 1
            out.append(
                Violation(
                    rule,
                    "no-source-level-override",
                    f"{rel}:{lineno} relaxes the `unsafe_code` lint in source; the workspace "
                    "forbid level admits no exception",
                )
            )
        for match in UNSAFE_TOKEN.finditer(code):
            lineno = code.count("\n", 0, match.start()) + 1
            out.append(Violation(rule, "no-unsafe-in-sources", f"{rel}:{lineno} contains an `unsafe` item"))
    return out


# ============================================================================
# GOV-1-03 — deterministic collections in semantic paths
# ============================================================================

NONDETERMINISTIC_COLLECTIONS = {
    "std::collections::HashMap": r"\bHashMap\b",
    "std::collections::HashSet": r"\bHashSet\b",
    "std::collections::hash_map": r"\bhash_map\b",
    "std::collections::hash_set": r"\bhash_set\b",
    "dashmap::DashMap": r"\bDashMap\b",
}
NONDETERMINISTIC_COLLECTION_CRATES = ("dashmap", "hashbrown", "im", "indexmap", "scc", "papaya", "flurry")


def rule_deterministic_collections(tree: Tree) -> list[Violation]:
    rule = "deterministic-collections"
    out: list[Violation] = []

    # The tier table must cover plan §20 exactly, or the scanned set is a guess.
    plan_crates, plan_problems = plan_20_crates(tree)
    out.extend(Violation(rule, "tier-partition-covers-plan-20", p) for p in plan_problems)
    if plan_crates:
        tiered = SEMANTIC_CORE | BOUNDARY | ADAPTERS
        for name in sorted(set(plan_crates) - tiered):
            out.append(
                Violation(
                    rule,
                    "tier-partition-covers-plan-20",
                    f"plan §20 names `{name}` but tools/governance/check_code_policy.py classifies "
                    "it into no policy tier; classify it as semantic core, boundary, or adapter",
                )
            )
        for name in sorted(tiered - set(plan_crates)):
            out.append(
                Violation(
                    rule,
                    "tier-partition-covers-plan-20",
                    f"`{name}` is classified into a policy tier but plan §20 does not name it",
                )
            )

    workspace = load_workspace(tree)
    for crate in sorted(SEMANTIC_CORE & workspace.members):
        for dep in sorted(workspace.external_deps(crate, ("normal", "build"))):
            if dep in NONDETERMINISTIC_COLLECTION_CRATES:
                out.append(
                    Violation(
                        rule,
                        "no-nondeterministic-collection-crates",
                        f"{workspace.manifest_path[crate]} depends on `{dep}`; the semantic core "
                        "uses BTreeMap/BTreeSet so iteration order is a function of the data alone "
                        "(docs/19 §7)",
                    )
                )

    scan = scan_sources(
        tree,
        core_sources(tree, SEMANTIC_CORE & workspace.members),
        rule,
        "deterministic-collections",
        NONDETERMINISTIC_COLLECTIONS,
        "the semantic core uses BTreeMap/BTreeSet; hash-ordered iteration is "
        "seed-dependent and fails the docs/19 §7 determinism matrix",
    )
    out.extend(scan.hits)
    return out


# ============================================================================
# GOV-1-04 — no ambient time/RNG in core
# ============================================================================

AMBIENT_TIME = {
    "std::time::SystemTime": r"\bSystemTime\b",
    "std::time::Instant": r"\bInstant\b",
    "std::time::UNIX_EPOCH": r"\bUNIX_EPOCH\b",
    "chrono": r"\bchrono\s*::",
    "time::OffsetDateTime": r"\bOffsetDateTime\b",
}
AMBIENT_RNG = {
    "rand": r"\brand\s*::",
    "rand_core": r"\brand_core\s*::",
    "getrandom": r"\bgetrandom\b",
    "thread_rng": r"\bthread_rng\b",
    "OsRng": r"\bOsRng\b",
    "StdRng": r"\bStdRng\b",
    "SmallRng": r"\bSmallRng\b",
    "fastrand": r"\bfastrand\b",
    "oorandom": r"\boorandom\b",
    "nanorand": r"\bnanorand\b",
    "random()": r"\brandom\s*\(",
}
# ADR-0003 / INV-005 forbid *ambient nondeterminism*, of which the process
# environment is one source. Enforcing it is broader than docs/12 §1's literal
# "time/RNG"; the evidence file declares that.
AMBIENT_ENVIRONMENT = {
    "std::env::var": r"\benv\s*::\s*var",
    "std::env::vars": r"\benv\s*::\s*vars",
    "std::process::id": r"\bprocess\s*::\s*id\b",
}
AMBIENT_CRATES = (
    "rand",
    "rand_core",
    "rand_chacha",
    "getrandom",
    "fastrand",
    "oorandom",
    "nanorand",
    "chrono",
    "time",
    "quanta",
    "minstant",
    "coarsetime",
    "uuid",
    "ulid",
)


def rule_no_ambient_time_rng(tree: Tree) -> list[Violation]:
    rule = "no-ambient-time-rng"
    out: list[Violation] = []
    workspace = load_workspace(tree)
    core = SEMANTIC_CORE & workspace.members
    sources = core_sources(tree, core)

    for crate in sorted(core):
        for dep in sorted(workspace.external_deps(crate, ("normal", "build"))):
            if dep in AMBIENT_CRATES:
                out.append(
                    Violation(
                        rule,
                        "no-ambient-dependency",
                        f"{workspace.manifest_path[crate]} depends on `{dep}`; time and randomness "
                        "reach the core only through a declared capability (INV-005, ADR-0003)",
                    )
                )

    for check, patterns, why in (
        (
            "ambient-time",
            AMBIENT_TIME,
            "the core reads no clock; a timing choice is a declared, replayable event (INV-005, ADR-0003)",
        ),
        (
            "ambient-rng",
            AMBIENT_RNG,
            "the core draws no randomness; a seed arrives as an explicit capability (INV-005, ADR-0003)",
        ),
        (
            "ambient-environment",
            AMBIENT_ENVIRONMENT,
            "the core reads no process environment; ambient nondeterminism is prohibited in verified cores (ADR-0003)",
        ),
    ):
        out.extend(scan_sources(tree, sources, rule, check, patterns, why).hits)
    return out


# ============================================================================
# GOV-1-05 — no platform-dependent hashing in canonical formats
# ============================================================================

PLATFORM_HASHERS = {
    "std::collections::hash_map::DefaultHasher": r"\bDefaultHasher\b",
    "std::hash::RandomState": r"\bRandomState\b",
    "std::hash::BuildHasherDefault": r"\bBuildHasherDefault\b",
    "SipHasher": r"\bSipHasher\d*\b",
    "ahash": r"\bahash\b|\bAHash",
    "fxhash": r"\bfxhash\b|\bFxHash",
    "rustc_hash": r"\brustc_hash\b",
    "twox_hash": r"\btwox_hash\b|\bXxHash",
    "seahash": r"\bseahash\b",
    "metrohash": r"\bmetrohash\b",
}
PLATFORM_HASH_CRATES = ("ahash", "fxhash", "rustc-hash", "twox-hash", "seahash", "metrohash", "highway", "fnv")
IDENTITY_SEAM = "crates/continuum-value/src/identity.rs"
# The canonical value module. `Value` deliberately implements no `Hash`: that is
# what makes hash-indexed state identity unrepresentable rather than merely
# discouraged (value.rs, "Seams left open on purpose"; ADR-0013). The rest of
# `continuum-value` — epoch tokens, assurance dimensions — may derive `Hash`
# freely; those are labels, not canonical states.
CANONICAL_VALUE_MODULE = "crates/continuum-value/src/value.rs"


def rule_no_platform_hashing(tree: Tree) -> list[Violation]:
    rule = "no-platform-hashing"
    out: list[Violation] = []
    workspace = load_workspace(tree)
    core = SEMANTIC_CORE & workspace.members

    for crate in sorted(core):
        for dep in sorted(workspace.external_deps(crate, ("normal", "build"))):
            if dep in PLATFORM_HASH_CRATES:
                out.append(
                    Violation(
                        rule,
                        "no-platform-hash-dependency",
                        f"{workspace.manifest_path[crate]} depends on `{dep}`; canonical identity is "
                        "computed by the declared `ContentHasher` seam, never a platform hasher (ADR-0013)",
                    )
                )

    out.extend(
        scan_sources(
            tree,
            core_sources(tree, core),
            rule,
            "platform-hashing",
            PLATFORM_HASHERS,
            "a std or third-party hasher is unstable across platforms, releases, and process "
            "starts; canonical identity comes from `Value::encode` bytes through the "
            "`ContentHasher` seam (ADR-0013)",
        ).hits
    )

    # The seam itself has to still be a seam.
    seam = tree.read_text(IDENTITY_SEAM)
    if seam is None:
        out.append(
            Violation(
                rule,
                "identity-seam-declared",
                f"{IDENTITY_SEAM} is absent; the labelled hash seam ADR-0013 requires has no home",
            )
        )
    else:
        code = strip_rust_comments(seam)
        for symbol in ("trait ContentHasher", "struct HashAlgorithm"):
            if symbol not in code:
                out.append(
                    Violation(
                        rule,
                        "identity-seam-declared",
                        f"{IDENTITY_SEAM} no longer declares `{symbol}`; the hash is a seam with an "
                        "algorithm label, not an ambient default (ADR-0013)",
                    )
                )
        if re.search(r"\bstd\s*::\s*hash\b", code):
            out.append(
                Violation(
                    rule,
                    "identity-seam-declared",
                    f"{IDENTITY_SEAM} imports `std::hash`; the seam takes `&[u8] -> Digest256` over "
                    "canonical bytes and never hashes an in-memory layout",
                )
            )

    # The canonical value type must stay un-`Hash`able: a `Hash` impl is exactly
    # the affordance that lets a caller index states by fingerprint and skip the
    # canonical comparison ADR-0013 requires.
    raw = tree.read_text(CANONICAL_VALUE_MODULE)
    if raw is None:
        out.append(
            Violation(
                rule,
                "canonical-value-is-not-hashable",
                f"{CANONICAL_VALUE_MODULE} is absent; the canonical encoding has no home",
            )
        )
    else:
        code = strip_rust_comments(raw)
        for match in re.finditer(r"#\[derive\(([^)]*)\)\]", code):
            if re.search(r"\bHash\b", match.group(1)):
                lineno = code.count("\n", 0, match.start()) + 1
                out.append(
                    Violation(
                        rule,
                        "canonical-value-is-not-hashable",
                        f"{CANONICAL_VALUE_MODULE}:{lineno} derives `Hash` on a canonical value type; "
                        "not implementing `Hash` is what makes hash-indexed state identity "
                        "unrepresentable rather than merely discouraged (ADR-0013, value.rs "
                        "\"Seams left open on purpose\")",
                    )
                )
        for match in re.finditer(r"\bimpl\b[^\n{;]*\bHash\b[^\n{;]*\bfor\b", code):
            lineno = code.count("\n", 0, match.start()) + 1
            out.append(
                Violation(
                    rule,
                    "canonical-value-is-not-hashable",
                    f"{CANONICAL_VALUE_MODULE}:{lineno} implements `Hash` on a canonical value type "
                    "(ADR-0013)",
                )
            )
    return out


# ============================================================================
# GOV-1-06 — no network access in certificate checking
# ============================================================================

NETWORK_SYMBOLS = {
    "std::net": r"\bstd\s*::\s*net\b",
    "TcpStream": r"\bTcpStream\b",
    "TcpListener": r"\bTcpListener\b",
    "UdpSocket": r"\bUdpSocket\b",
    "UnixStream": r"\bUnixStream\b",
    "UnixListener": r"\bUnixListener\b",
    "ToSocketAddrs": r"\bToSocketAddrs\b",
}
NETWORK_CRATES = (
    "reqwest",
    "hyper",
    "hyper-util",
    "ureq",
    "curl",
    "isahc",
    "surf",
    "attohttpc",
    "tokio",
    "async-std",
    "smol",
    "tonic",
    "quinn",
    "s2n-quic",
    "socket2",
    "mio",
    "rustls",
    "native-tls",
    "openssl",
    "h2",
    "h3",
    "axum",
    "warp",
    "actix-web",
    "rocket",
    "tungstenite",
    "tokio-tungstenite",
    "tiny_http",
    "rouille",
    "trust-dns-resolver",
    "hickory-resolver",
)
# Workspace crates that exist to touch a network or a peer process. A checker
# that reaches one of these transitively has a network in its closure.
NETWORK_CAPABLE_MEMBERS = frozenset(
    {"continuum-effects-network", "continuum-asupersync", "continuum-proof-client", "continuumd", *ADAPTERS}
)


def rule_no_network_in_checker(tree: Tree) -> list[Violation]:
    rule = "no-network-in-checker"
    out: list[Violation] = []
    workspace = load_workspace(tree)
    checkers = sorted(CHECKER & workspace.members)

    for missing in sorted(CHECKER - workspace.members):
        out.append(
            Violation(
                rule,
                "checker-crates-present",
                f"certificate-checking crate `{missing}` is not a workspace member; the rule would "
                "pass by having nothing to check",
            )
        )

    for src in checkers:
        closure = workspace.closure(src, ("normal", "build"))
        reachable = {src: [src], **closure}
        for target, path in sorted(closure.items()):
            if target in NETWORK_CAPABLE_MEMBERS:
                out.append(
                    Violation(
                        rule,
                        "no-network-capable-crate-in-closure",
                        f"{' -> '.join(path)} — certificate checking is offline; a certificate is "
                        "checked from its wire form, never fetched (plan §20, docs/03 §5)",
                    )
                )
        for node, path in sorted(reachable.items()):
            for dep in sorted(workspace.external_deps(node, ("normal", "build"))):
                if dep in NETWORK_CRATES:
                    out.append(
                        Violation(
                            rule,
                            "no-network-dependency-in-closure",
                            f"{' -> '.join(path)} depends on `{dep}` — a network-capable or async "
                            "runtime crate in the certificate-checking closure (plan §20 kernel covenant)",
                        )
                    )

    out.extend(
        scan_sources(
            tree,
            core_sources(tree, checkers),
            rule,
            "no-socket-in-checker",
            NETWORK_SYMBOLS,
            "the certificate checker opens no socket (docs/12 §1, plan §20)",
        ).hits
    )
    return out


# ============================================================================
# GOV-1-07 — dependency additions require rationale and TCB classification
# ============================================================================

TCB_CLASSES = ("trusted-checking-base", "engine", "dev-only", "build-only")
MIN_RATIONALE = 24


def rule_dependency_rationale(tree: Tree) -> list[Violation]:
    rule = "dependency-rationale"
    out: list[Violation] = []
    workspace = load_workspace(tree)

    manifest = tree.read_toml(RATIONALE_PATH)
    if manifest is None:
        out.append(
            Violation(
                rule,
                "manifest-present",
                f"{RATIONALE_PATH} is missing or is not valid TOML; every dependency needs a "
                "rationale and a TCB class before it is added (docs/12 §1)",
            )
        )
        return out

    entries: dict[str, dict] = {}
    for entry in manifest.get("dependency", []):
        name = entry.get("name")
        if not isinstance(name, str):
            out.append(Violation(rule, "manifest-well-formed", f"{RATIONALE_PATH} has a [[dependency]] with no name"))
            continue
        if name in entries:
            out.append(Violation(rule, "manifest-well-formed", f"{RATIONALE_PATH} declares `{name}` twice"))
            continue
        entries[name] = entry

    actual: dict[str, dict[str, set[str]]] = {}
    for edge in workspace.edges:
        record = actual.setdefault(edge.dependency, {"consumers": set(), "kinds": set()})
        record["consumers"].add(edge.consumer)
        record["kinds"].add(edge.kind)

    for name in sorted(set(actual) - set(entries)):
        consumers = ", ".join(sorted(actual[name]["consumers"]))
        out.append(
            Violation(
                rule,
                "every-dependency-has-an-entry",
                f"dependency `{name}` (used by {consumers}) has no entry in {RATIONALE_PATH}; "
                "a dependency addition requires a rationale and a TCB classification",
            )
        )
    for name in sorted(set(entries) - set(actual)):
        out.append(
            Violation(
                rule,
                "no-stale-entries",
                f"{RATIONALE_PATH} records `{name}`, which no manifest in the workspace depends on",
            )
        )

    for name in sorted(set(entries) & set(actual)):
        entry = entries[name]
        observed = actual[name]
        rationale = entry.get("rationale")
        if not isinstance(rationale, str) or len(rationale.strip()) < MIN_RATIONALE:
            out.append(
                Violation(
                    rule,
                    "rationale-is-stated",
                    f"{RATIONALE_PATH} entry `{name}` has no usable rationale "
                    f"(needs at least {MIN_RATIONALE} characters saying why the dependency exists)",
                )
            )
        elif "\n" in rationale.strip():
            out.append(
                Violation(rule, "rationale-is-stated", f"{RATIONALE_PATH} entry `{name}` rationale is not one line")
            )

        tcb = entry.get("tcb_class")
        if tcb not in TCB_CLASSES:
            out.append(
                Violation(
                    rule,
                    "tcb-class-is-declared",
                    f"{RATIONALE_PATH} entry `{name}` has tcb_class {tcb!r}; expected one of {list(TCB_CLASSES)}",
                )
            )

        expected_origin = "workspace" if name in workspace.members else "external"
        if entry.get("origin") != expected_origin:
            out.append(
                Violation(
                    rule,
                    "origin-is-accurate",
                    f"{RATIONALE_PATH} entry `{name}` claims origin {entry.get('origin')!r}; it is {expected_origin!r}",
                )
            )

        declared_by = set(entry.get("used_by", []))
        if declared_by != observed["consumers"]:
            out.append(
                Violation(
                    rule,
                    "used-by-is-accurate",
                    f"{RATIONALE_PATH} entry `{name}` lists used_by {sorted(declared_by)}; "
                    f"the manifests say {sorted(observed['consumers'])}",
                )
            )
        declared_kinds = set(entry.get("kinds", []))
        if declared_kinds != observed["kinds"]:
            out.append(
                Violation(
                    rule,
                    "kinds-are-accurate",
                    f"{RATIONALE_PATH} entry `{name}` lists kinds {sorted(declared_kinds)}; "
                    f"the manifests say {sorted(observed['kinds'])}",
                )
            )

        # Classification has to mean something: a dependency the certificate
        # checker links is in the trusted checking base whether or not anyone
        # wants it to be.
        linked_by_checker = {
            e.consumer
            for e in workspace.edges
            if e.dependency == name and e.consumer in CHECKER and e.kind in ("normal", "build")
        }
        if linked_by_checker and tcb != "trusted-checking-base":
            out.append(
                Violation(
                    rule,
                    "checker-dependencies-are-tcb",
                    f"{RATIONALE_PATH} classifies `{name}` as {tcb!r}, but "
                    f"{sorted(linked_by_checker)} link it; anything the certificate checker links is "
                    "trusted-checking-base (docs/03 §5)",
                )
            )
        if observed["kinds"] == {"dev"} and tcb != "dev-only":
            out.append(
                Violation(
                    rule,
                    "dev-only-is-classified-dev-only",
                    f"{RATIONALE_PATH} classifies `{name}` as {tcb!r}, but it is only ever a dev-dependency",
                )
            )
        if observed["kinds"] == {"build"} and tcb != "build-only":
            out.append(
                Violation(
                    rule,
                    "build-only-is-classified-build-only",
                    f"{RATIONALE_PATH} classifies `{name}` as {tcb!r}, but it is only ever a build-dependency",
                )
            )
    return out


# ============================================================================
# GOV-1-08 — semantic changes require ADR
# ============================================================================

ADR_STATUSES = ("proposed", "accepted", "superseded", "rejected", "experimental")
ADR_STATUS_BOLD = re.compile(r"^\*\*Status:\*\*\s*(.+)$", re.M)
ADR_STATUS_SECTION = re.compile(r"^##\s+Status\s*\n+(.+)$", re.M)
ADR_LINK = re.compile(r"\((\d{4})-[^)]*\.md\)")
ADR_CITATION = re.compile(r"\bADR[- ](\d{4})\b")
RFC_CITATION = re.compile(r"\bRFC[- ](\d{4})\b")


def _decision_record_ids(tree: Tree, directory: str) -> set[str]:
    return {Path(p).name[:4] for p in tree.glob(f"{directory}/[0-9]*.md")}


def rule_semantic_change_adr(tree: Tree) -> list[Violation]:
    rule = "semantic-change-adr"
    out: list[Violation] = []

    adr_ids = _decision_record_ids(tree, ADR_DIR)
    rfc_ids = _decision_record_ids(tree, RFC_DIR)
    if not adr_ids:
        out.append(Violation(rule, "adr-directory-populated", f"{ADR_DIR} contains no ADR files"))
        return out

    index = tree.read_text(f"{ADR_DIR}/README.md")
    if index is None:
        out.append(Violation(rule, "adr-index-is-a-bijection", f"{ADR_DIR}/README.md is missing"))
    else:
        linked = set(ADR_LINK.findall(index))
        for missing in sorted(adr_ids - linked):
            out.append(
                Violation(
                    rule,
                    "adr-index-is-a-bijection",
                    f"ADR {missing} exists but {ADR_DIR}/README.md does not index it; an unindexed "
                    "decision is not discoverable by the next change that would contradict it",
                )
            )
        for dangling in sorted(linked - adr_ids):
            out.append(
                Violation(
                    rule,
                    "adr-index-is-a-bijection",
                    f"{ADR_DIR}/README.md indexes ADR {dangling}, which has no file",
                )
            )

    for rel in sorted(tree.glob(f"{ADR_DIR}/[0-9]*.md")):
        text = tree.read_text(rel) or ""
        match = ADR_STATUS_BOLD.search(text) or ADR_STATUS_SECTION.search(text)
        if not match:
            out.append(
                Violation(
                    rule,
                    "adr-declares-a-status",
                    f"{rel} declares no status; docs/12 §2 gives ADRs a status "
                    f"({', '.join(ADR_STATUSES)})",
                )
            )
            continue
        token = match.group(1).strip().strip(".").split()[0].lower() if match.group(1).strip() else ""
        if token not in ADR_STATUSES:
            out.append(
                Violation(
                    rule,
                    "adr-declares-a-status",
                    f"{rel} declares status {match.group(1).strip()!r}, which does not begin with one "
                    f"of docs/12 §2's statuses ({', '.join(ADR_STATUSES)})",
                )
            )

    # A citation nobody can follow is not a decision record.
    cited_paths = sorted(set(tree.glob("crates/**/*.rs")) | set(tree.glob(f"{GOVERNANCE}/*.toml")))
    for rel in cited_paths:
        text = tree.read_text(rel) or ""
        for number in sorted(set(ADR_CITATION.findall(text))):
            if number not in adr_ids:
                out.append(
                    Violation(
                        rule,
                        "citations-resolve",
                        f"{rel} cites ADR-{number}, which does not exist in {ADR_DIR}",
                    )
                )
        for number in sorted(set(RFC_CITATION.findall(text))):
            if number not in rfc_ids:
                out.append(
                    Violation(
                        rule,
                        "citations-resolve",
                        f"{rel} cites RFC {number}, which does not exist in {RFC_DIR}",
                    )
                )

    # The obligation, in the only form one revision can show: code that carries
    # semantic behavior names the numbered decision that authorized it.
    workspace = load_workspace(tree)
    for crate in sorted(SEMANTIC_CORE & workspace.members):
        files = core_sources(tree, [crate])
        bearing = []
        cites = False
        for rel in files:
            raw = tree.read_text(rel) or ""
            if _code_lines(raw):
                bearing.append(rel)
            if ADR_CITATION.search(raw) or RFC_CITATION.search(raw):
                cites = True
        if bearing and not cites:
            out.append(
                Violation(
                    rule,
                    "semantic-code-cites-a-decision-record",
                    f"crate `{crate}` carries semantic code ({', '.join(bearing)}) but cites no ADR or "
                    "RFC anywhere in src/; a semantic change requires the decision record that authorized it",
                )
            )
    return out


def _code_lines(raw: str) -> int:
    return sum(1 for line in raw.splitlines() if line.strip() and not line.strip().startswith("//"))


# ============================================================================
# GOV-1-09 — every breaking change increments the semantic epoch
# ============================================================================

SCHEMA_ID_RE = re.compile(r"^https://continuum\.dev/schema/v(\d+)/([a-z0-9-]+)\.json$")
EPOCH_AS_STR_ARM = re.compile(r"Self::(\w+)\s*=>\s*\"([a-z]+)\"")
EPOCH_DOC_LIST = re.compile(
    r"six independently versioned\s*\n?\s*epochs\s*—\s*([a-z, \n]+?)\s*—", re.S
)


def rule_epoch_discipline(tree: Tree) -> list[Violation]:
    rule = "epoch-discipline"
    out: list[Violation] = []

    readme = tree.read_text(f"{SCHEMA_DIR}/README.md")
    if readme is None:
        out.append(Violation(rule, "epoch-rules-are-stated", f"{SCHEMA_DIR}/README.md is missing"))
        readme = ""
    if "## What advances a schema epoch" not in readme:
        out.append(
            Violation(
                rule,
                "epoch-rules-are-stated",
                f"{SCHEMA_DIR}/README.md no longer states what advances a schema epoch; the rule that "
                "a breaking change mints a new epoch has to be written down to be followed",
            )
        )
    if "MUST advance `schema_epoch`" not in readme:
        out.append(
            Violation(
                rule,
                "epoch-rules-are-stated",
                f"{SCHEMA_DIR}/README.md no longer carries the normative "
                "\"MUST advance `schema_epoch`\" obligation for breaking changes",
            )
        )

    class_ids: dict[str, tuple[str, int]] = {}
    for rel in sorted(tree.glob(f"{SCHEMA_DIR}/*.schema.json")):
        doc = tree.read_json(rel)
        if not isinstance(doc, dict):
            out.append(Violation(rule, "schema-id-and-epoch-agree", f"{rel} is not a JSON object"))
            continue
        doc_id = doc.get("$id")
        epoch = doc.get("schema_epoch")
        match = SCHEMA_ID_RE.fullmatch(doc_id) if isinstance(doc_id, str) else None
        if not match:
            out.append(
                Violation(
                    rule,
                    "schema-id-and-epoch-agree",
                    f"{rel} has $id {doc_id!r}, which is not "
                    "`https://continuum.dev/schema/v<epoch>/<name>.json` (schemas/README.md \"Identity\")",
                )
            )
            continue
        if int(match.group(1)) != epoch:
            out.append(
                Violation(
                    rule,
                    "schema-id-and-epoch-agree",
                    f"{rel} declares schema_epoch {epoch!r} but its $id says v{match.group(1)}; the "
                    "two-field header and the document $id must determine each other",
                )
            )
        class_id = f"https://continuum.dev/schema/{match.group(2)}.json"
        declared_class = doc.get("properties", {}).get("schema_id", {}).get("const")
        if doc.get("schema_kind") == "artifact" and declared_class != class_id:
            out.append(
                Violation(
                    rule,
                    "schema-id-and-epoch-agree",
                    f"{rel} pins instance schema_id {declared_class!r}; the class identity of its own "
                    f"$id is {class_id!r}",
                )
            )
        if isinstance(epoch, int):
            class_ids[class_id] = (rel, epoch)

    # An instance names the contract it was written under; it may not name one
    # epoch and validate against another.
    for rel in sorted(tree.glob(f"{SCHEMA_DIR}/examples/*.json")):
        doc = tree.read_json(rel)
        if not isinstance(doc, dict):
            continue
        class_id = doc.get("schema_id")
        if not isinstance(class_id, str) or class_id not in class_ids:
            continue
        schema_rel, schema_epoch = class_ids[class_id]
        if doc.get("schema_epoch") != schema_epoch:
            out.append(
                Violation(
                    rule,
                    "instance-epoch-matches-its-schema",
                    f"{rel} declares schema_epoch {doc.get('schema_epoch')!r} but {schema_rel} is "
                    f"epoch {schema_epoch}; an artifact pinned to an epoch that does not exist is "
                    "an unresolvable contract",
                )
            )

    # The epoch vocabulary in prose and the epoch vocabulary in code are one set.
    epoch_rs = tree.read_text("crates/continuum-value/src/epoch.rs")
    if epoch_rs is None:
        out.append(
            Violation(rule, "epoch-kinds-agree-with-docs", "crates/continuum-value/src/epoch.rs is missing")
        )
    else:
        code = strip_rust_comments(epoch_rs)
        code_kinds = {token for _, token in EPOCH_AS_STR_ARM.findall(code)}
        doc_match = EPOCH_DOC_LIST.search(readme)
        doc_kinds = (
            {t.strip() for t in re.split(r",|\band\b", doc_match.group(1)) if t.strip()} if doc_match else set()
        )
        if not doc_kinds:
            out.append(
                Violation(
                    rule,
                    "epoch-kinds-agree-with-docs",
                    f"{SCHEMA_DIR}/README.md no longer names the six independently versioned epochs",
                )
            )
        elif code_kinds != doc_kinds:
            out.append(
                Violation(
                    rule,
                    "epoch-kinds-agree-with-docs",
                    f"epoch.rs names {sorted(code_kinds)} but {SCHEMA_DIR}/README.md names "
                    f"{sorted(doc_kinds)}; the epochs must not be conflated or quietly renamed",
                )
            )

        # Advancing is an event carrying a compatibility verdict, not a counter++.
        if "pub enum Compatibility" not in code:
            out.append(
                Violation(
                    rule,
                    "advance-carries-a-compatibility-verdict",
                    "epoch.rs declares no `Compatibility` verdict; plan §4.6 makes an epoch advance "
                    "publish a typed per-artifact-class compatibility statement",
                )
            )
        for variant in ("Preserved", "Revalidate", "Incompatible"):
            if not re.search(rf"\b{variant}\b", code):
                out.append(
                    Violation(
                        rule,
                        "advance-carries-a-compatibility-verdict",
                        f"epoch.rs `Compatibility` is missing the `{variant}` verdict (plan §4.6)",
                    )
                )
        if not re.search(r"struct EpochAdvance\b", code):
            out.append(
                Violation(
                    rule,
                    "advance-carries-a-compatibility-verdict",
                    "epoch.rs declares no `EpochAdvance`; an advance has to be a recorded event",
                )
            )
        elif not re.search(r"fn new\((?:[^)]*\n)*?[^)]*compatibility:\s*Compatibility", code):
            out.append(
                Violation(
                    rule,
                    "advance-carries-a-compatibility-verdict",
                    "`EpochAdvance::new` no longer requires a `Compatibility` argument, so an advance "
                    "could be recorded without publishing its blast radius (plan §4.6)",
                )
            )
    return out


# ============================================================================
# GOV-1-10 — every pack operation has a normative contract
# ============================================================================

PACK_SCHEMA = f"{SCHEMA_DIR}/domain-pack.schema.json"
PACK_CLASS_ID = "https://continuum.dev/schema/domain-pack.json"
PACK_CONTRACT_DOC = "notes/plan/docs/17_DOMAIN_PACK_CONTRACT.md"

# docs/17 §4 `OperationContract` field -> the manifest property that carries it.
# Both directions are checked, so neither the doc nor the schema can drift alone.
OPERATION_CONTRACT_MAP = {
    "name": "name",
    "input_type": "command_type",
    "output_type": "response_type",
    "phases": "effect_phases",
    "resources": "footprint_rule",
    "observations": "observer_events",
    "cancel_points": "cancellation_points",
}
# `faults` is declared per profile, not per operation (domain-pack.schema.json
# `profiles[].faults`), so it has no operation-level property by design.
OPERATION_CONTRACT_ELSEWHERE = {"faults"}
OPERATION_REQUIRED = ("name", "command_type", "response_type", "effect_phases")
DOC_STRUCT_FIELD = re.compile(r"^\s{4}(\w+)\s*:", re.M)


def rule_pack_operation_contract(tree: Tree) -> list[Violation]:
    rule = "pack-operation-contract"
    out: list[Violation] = []

    schema = tree.read_json(PACK_SCHEMA)
    if not isinstance(schema, dict):
        out.append(Violation(rule, "schema-requires-a-contract", f"{PACK_SCHEMA} is missing or unreadable"))
        return out

    if "operations" not in schema.get("required", []):
        out.append(
            Violation(
                rule,
                "schema-requires-a-contract",
                f"{PACK_SCHEMA} does not require `operations`; a pack manifest could declare none",
            )
        )
    item = schema.get("properties", {}).get("operations", {}).get("items", {})
    required = set(item.get("required", []))
    for field_name in OPERATION_REQUIRED:
        if field_name not in required:
            out.append(
                Violation(
                    rule,
                    "schema-requires-a-contract",
                    f"{PACK_SCHEMA} operations items do not require `{field_name}`; an operation without "
                    "it has no normative contract (docs/17 §4 `OperationContract`)",
                )
            )
    if item.get("additionalProperties") is not False:
        out.append(
            Violation(
                rule,
                "schema-requires-a-contract",
                f"{PACK_SCHEMA} operations items are open; an unrecognized operation field would be "
                "accepted without meaning",
            )
        )

    # docs/17 §4 and the schema describe one contract.
    doc = tree.read_text(PACK_CONTRACT_DOC)
    if doc is None:
        out.append(Violation(rule, "doc-and-schema-agree", f"{PACK_CONTRACT_DOC} is missing"))
    else:
        struct = re.search(r"struct OperationContract \{(.*?)\n\}", doc, re.S)
        if not struct:
            out.append(
                Violation(
                    rule,
                    "doc-and-schema-agree",
                    f"{PACK_CONTRACT_DOC} §4 no longer declares `struct OperationContract`",
                )
            )
        else:
            doc_fields = set(DOC_STRUCT_FIELD.findall(struct.group(1)))
            properties = set(item.get("properties", {}))
            for field_name in sorted(doc_fields - set(OPERATION_CONTRACT_MAP) - OPERATION_CONTRACT_ELSEWHERE):
                out.append(
                    Violation(
                        rule,
                        "doc-and-schema-agree",
                        f"{PACK_CONTRACT_DOC} §4 `OperationContract` declares `{field_name}`, which maps "
                        f"to no property of {PACK_SCHEMA}; a contract field the manifest cannot carry is "
                        "not enforceable",
                    )
                )
            for field_name in sorted(set(OPERATION_CONTRACT_MAP) - doc_fields):
                out.append(
                    Violation(
                        rule,
                        "doc-and-schema-agree",
                        f"{PACK_CONTRACT_DOC} §4 no longer declares `{field_name}`, which "
                        f"{PACK_SCHEMA} still carries",
                    )
                )
            for doc_field, prop in sorted(OPERATION_CONTRACT_MAP.items()):
                if doc_field in doc_fields and prop not in properties:
                    out.append(
                        Violation(
                            rule,
                            "doc-and-schema-agree",
                            f"{PACK_SCHEMA} has no `{prop}` property for docs/17 §4 `{doc_field}`",
                        )
                    )

    # Every pack manifest in the dossier states the whole contract, including
    # the parts the schema leaves optional.
    manifests = 0
    for rel in sorted(tree.glob(f"{SCHEMA_DIR}/examples/*.json")):
        instance = tree.read_json(rel)
        if not isinstance(instance, dict) or instance.get("schema_id") != PACK_CLASS_ID:
            continue
        manifests += 1
        operations = instance.get("operations")
        if not isinstance(operations, list) or not operations:
            out.append(
                Violation(rule, "every-operation-states-its-contract", f"{rel} declares no operations")
            )
            continue
        for index, operation in enumerate(operations):
            label = operation.get("name", f"#{index}") if isinstance(operation, dict) else f"#{index}"
            if not isinstance(operation, dict):
                out.append(
                    Violation(rule, "every-operation-states-its-contract", f"{rel} operation {label} is not an object")
                )
                continue
            for prop in sorted(set(OPERATION_CONTRACT_MAP.values())):
                value = operation.get(prop)
                if value is None or value == "" or value == []:
                    out.append(
                        Violation(
                            rule,
                            "every-operation-states-its-contract",
                            f"{rel} operation `{label}` states no `{prop}`; docs/17 §4 makes it part of "
                            "the operation contract",
                        )
                    )
    if manifests == 0:
        out.append(
            Violation(
                rule,
                "every-operation-states-its-contract",
                f"no domain-pack manifest exists under {SCHEMA_DIR}/examples/; the rule would pass by "
                "having nothing to check",
            )
        )
    return out


# ============================================================================
# GOV-1-11 — reference semantics precedes optimization
# ============================================================================

ARCHITECTURE = "notes/plan/docs/01_ARCHITECTURE.md"
REFERENCE_HEADING = "### 7.1 Reference path"
OPTIMIZED_HEADING = "### 7.2 Optimized paths"
# Plan §20's serialization boundary, as every engine crate states it: "results
# reach the trust base only as certificates checked from wire form by
# `continuum-kernel-*`". An engine that drops this sentence is claiming its own
# output is the meaning — which is exactly the inversion this obligation forbids.
SERIALIZATION_BOUNDARY = "checked from wire form"


def rule_reference_precedes_optimization(tree: Tree) -> list[Violation]:
    rule = "reference-precedes-optimization"
    out: list[Violation] = []

    architecture = tree.read_text(ARCHITECTURE) or ""
    ref_at = architecture.find(REFERENCE_HEADING)
    opt_at = architecture.find(OPTIMIZED_HEADING)
    if ref_at < 0:
        out.append(
            Violation(
                rule,
                "architecture-orders-reference-first",
                f"{ARCHITECTURE} no longer has `{REFERENCE_HEADING}`; the reference path is where "
                "executable finite meaning is defined",
            )
        )
    if opt_at < 0:
        out.append(
            Violation(
                rule, "architecture-orders-reference-first", f"{ARCHITECTURE} no longer has `{OPTIMIZED_HEADING}`"
            )
        )
    if ref_at >= 0 and opt_at >= 0:
        if ref_at > opt_at:
            out.append(
                Violation(
                    rule,
                    "architecture-orders-reference-first",
                    f"{ARCHITECTURE} places the optimized paths before the reference path",
                )
            )
        reference_body = architecture[ref_at:opt_at]
        optimized_body = architecture[opt_at : opt_at + 1200]
        if "differential truth" not in reference_body:
            out.append(
                Violation(
                    rule,
                    "reference-path-is-the-oracle",
                    f"{ARCHITECTURE} §7.1 no longer says the reference path defines differential truth",
                )
            )
        if "checked outside the search implementation" not in optimized_body:
            out.append(
                Violation(
                    rule,
                    "optimized-paths-are-checked-outside-search",
                    f"{ARCHITECTURE} §7.2 no longer requires every optimized path to emit evidence "
                    "checked outside the search implementation",
                )
            )

    workspace = load_workspace(tree)
    if ENGINE_REFERENCE not in workspace.members:
        out.append(
            Violation(
                rule,
                "reference-engine-exists",
                f"`{ENGINE_REFERENCE}` is not a workspace member; there is no oracle for an optimized "
                "path to be measured against",
            )
        )
    else:
        closure = workspace.closure(ENGINE_REFERENCE, ("normal", "build"))
        for optimized in ENGINES_OPTIMIZED:
            path = closure.get(optimized)
            if path:
                out.append(
                    Violation(
                        rule,
                        "reference-engine-is-independent",
                        f"{' -> '.join(path)} — the reference path may not be built on an optimized "
                        "path; it is written for obvious correctness, not speed",
                    )
                )
        seam = "\n".join(tree.read_text(p) or "" for p in core_sources(tree, [ENGINE_REFERENCE]))
        if "differential oracle" not in seam:
            out.append(
                Violation(
                    rule,
                    "reference-engine-is-declared-the-oracle",
                    f"crates/{ENGINE_REFERENCE}/src no longer declares itself the differential oracle "
                    "every optimized path is measured against",
                )
            )

    for optimized in ENGINES_OPTIMIZED:
        if optimized not in workspace.members:
            continue
        text = "\n".join(tree.read_text(p) or "" for p in core_sources(tree, [optimized]))
        if SERIALIZATION_BOUNDARY not in text:
            out.append(
                Violation(
                    rule,
                    "optimized-engine-declares-its-check",
                    f"crates/{optimized}/src no longer states that its results reach the trust base only "
                    f"as certificates \"{SERIALIZATION_BOUNDARY}\"; an optimized path never becomes the "
                    "meaning, it produces evidence the reference-derived checker re-checks (plan §20)",
                )
            )
    return out


# ============================================================================
# GOV-1-12 — ambiguous behavior is an error, not implementation freedom
# ============================================================================

CONSTITUTION_STATEMENT = "- ambiguous behavior is an error, not implementation freedom."
ASSURANCE_SCHEMA = f"{SCHEMA_DIR}/assurance-result.schema.json"
VERDICTS = ["established", "refuted", "inconclusive"]
TASK_RESULT = "crates/continuum-task/src/result.rs"


def rule_ambiguity_is_an_error(tree: Tree) -> list[Violation]:
    rule = "ambiguity-is-an-error"
    out: list[Violation] = []

    constitution = tree.read_text(CONSTITUTION) or ""
    if CONSTITUTION_STATEMENT not in constitution:
        out.append(
            Violation(
                rule,
                "constitution-states-the-rule",
                f"{CONSTITUTION} §1 no longer states {CONSTITUTION_STATEMENT!r}",
            )
        )

    schemas = sorted(tree.glob(f"{SCHEMA_DIR}/*.schema.json"))
    if not schemas:
        out.append(Violation(rule, "schemas-are-closed", f"no schema documents found under {SCHEMA_DIR}"))
    for rel in schemas:
        doc = tree.read_json(rel)
        if not isinstance(doc, dict):
            out.append(Violation(rule, "schemas-are-closed", f"{rel} is not a JSON object"))
            continue
        if doc.get("additionalProperties") is not False:
            out.append(
                Violation(
                    rule,
                    "schemas-are-closed",
                    f"{rel} does not set top-level `additionalProperties: false`; an unrecognized "
                    "field would be accepted with no defined meaning (schemas/README.md)",
                )
            )
        if doc.get("schema_kind") != "artifact":
            continue
        required = set(doc.get("required", []))
        properties = doc.get("properties", {})
        for field_name in ("schema_id", "schema_epoch"):
            if field_name not in required:
                out.append(
                    Violation(
                        rule,
                        "instances-name-their-contract",
                        f"{rel} does not require `{field_name}`; an instance that does not say which "
                        "contract it was written under has to be guessed at",
                    )
                )
            if "const" not in properties.get(field_name, {}):
                out.append(
                    Violation(
                        rule,
                        "instances-name-their-contract",
                        f"{rel} does not pin `{field_name}` with a `const`; a free-form identity is "
                        "an ambiguity the validator cannot resolve",
                    )
                )

    assurance = tree.read_json(ASSURANCE_SCHEMA)
    if not isinstance(assurance, dict):
        out.append(Violation(rule, "verdict-vocabulary-is-closed", f"{ASSURANCE_SCHEMA} is missing"))
    else:
        verdict = assurance.get("properties", {}).get("verdict", {}).get("enum")
        if verdict != VERDICTS:
            out.append(
                Violation(
                    rule,
                    "verdict-vocabulary-is-closed",
                    f"{ASSURANCE_SCHEMA} verdict enum is {verdict!r}; it is closed at {VERDICTS} — "
                    "budget exhaustion, unsupported, and engine error are not verdicts (INV-008, plan §11.4)",
                )
            )
        conditionals = json.dumps(assurance.get("allOf", []), sort_keys=True)
        if "inconclusive_reason" not in conditionals:
            out.append(
                Violation(
                    rule,
                    "inconclusive-carries-its-reason",
                    f"{ASSURANCE_SCHEMA} no longer requires `inconclusive_reason` when the verdict is "
                    "inconclusive; an unexplained inconclusive is exactly the ambiguity INV-008 forbids",
                )
            )

    result = tree.read_text(TASK_RESULT)
    if result is None:
        out.append(Violation(rule, "inconclusive-carries-its-reason", f"{TASK_RESULT} is missing"))
    else:
        code = strip_rust_comments(result)
        if not re.search(r"\bInconclusive\(\s*InconclusiveReason\s*\)", code):
            out.append(
                Violation(
                    rule,
                    "inconclusive-carries-its-reason",
                    f"{TASK_RESULT} no longer makes the INV-008 reason a non-optional payload of "
                    "`TaskOutcome::Inconclusive`; the reason must not be constructible as absent",
                )
            )
        if re.search(r"\bInconclusive\(\s*Option\s*<", code):
            out.append(
                Violation(
                    rule,
                    "inconclusive-carries-its-reason",
                    f"{TASK_RESULT} makes the INV-008 reason optional; an inconclusive outcome without "
                    "a typed reason must not compile",
                )
            )
    return out


# ============================================================================
# Requirement registry
# ============================================================================


@dataclass(frozen=True)
class Requirement:
    rid: str
    summary: str
    rule: str
    enforces: str
    scanned: tuple[str, ...]
    boundary: str | None
    fn: object
    # The two-revision half, where one exists: the rule in
    # `check_revision_delta.py` that enforces the *change* this rule can only
    # enforce the *state* of. `None` means this requirement is enforced from one
    # revision alone, and its `boundary` says what that costs.
    delta_gate: str | None = None


REQUIREMENTS: tuple[Requirement, ...] = (
    Requirement(
        rid="GOV-1-01",
        summary="Rust edition/toolchain pinned.",
        rule="toolchain-pinned",
        enforces=(
            "`rust-toolchain.toml` exists and pins an exact `major.minor.patch` channel; "
            "`[workspace.package]` sets `edition` and the `rust-version` floor; every crate inherits "
            "the edition with `edition.workspace = true` rather than pinning its own."
        ),
        scanned=("rust-toolchain.toml", "Cargo.toml", "crates/*/Cargo.toml"),
        boundary=None,
        fn=rule_toolchain_pinned,
    ),
    Requirement(
        rid="GOV-1-02",
        summary="`unsafe` forbidden by default.",
        rule="unsafe-forbidden",
        enforces=(
            "`[workspace.lints.rust] unsafe_code = \"forbid\"` is set; every crate opts in with "
            "`[lints] workspace = true`; no crate manifest re-declares `unsafe_code`; no source relaxes "
            "the lint by attribute; no `unsafe` item exists in any crate source, tests included."
        ),
        scanned=("Cargo.toml", "crates/*/Cargo.toml", "crates/**/*.rs"),
        boundary=None,
        fn=rule_unsafe_forbidden,
    ),
    Requirement(
        rid="GOV-1-03",
        summary="Deterministic collections in semantic paths.",
        rule="deterministic-collections",
        enforces=(
            "No `HashMap`/`HashSet`/`hash_map`/`hash_set`/`DashMap` and no nondeterministic-collection "
            "dependency in the semantic core's `src/`. The crate tiering that defines the semantic core "
            "is checked against plan §20 in the same rule, so a new crate cannot be added without being "
            "classified. An inline `// continuum:allow(deterministic-collections): <reason>` waiver "
            "covers its own line and the next; every granted waiver is listed in this file."
        ),
        scanned=("notes/plan/plan.md §20", "crates/<semantic-core>/src/**/*.rs", "crates/*/Cargo.toml"),
        boundary=(
            "Scope is the semantic-core tier only. Boundary crates (the effect packs, asupersync, Forge, "
            "the benchmark harness, the security crate, the proof-service client) and the protocol "
            "adapters are excluded by design: owning an ambient resource behind a declared capability is "
            "their job. The rule sees declared uses, not a `HashMap` reached through a third-party "
            "dependency's public API."
        ),
        fn=rule_deterministic_collections,
    ),
    Requirement(
        rid="GOV-1-04",
        summary="No ambient time/RNG in core.",
        rule="no-ambient-time-rng",
        enforces=(
            "No `SystemTime`/`Instant`/`UNIX_EPOCH`/`chrono`/`OffsetDateTime`, no `rand`/`getrandom`/"
            "`thread_rng`/`OsRng`/`StdRng`/`SmallRng`/`fastrand`/`random()`, and no dependency on a "
            "clock or RNG crate anywhere in the semantic core's `src/`. `std::env::var`/`vars` and "
            "`std::process::id` are enforced under the same rule."
        ),
        scanned=("crates/<semantic-core>/src/**/*.rs", "crates/*/Cargo.toml"),
        boundary=(
            "Two deliberate extensions beyond the literal bullet, both declared: the process environment "
            "is enforced on ADR-0003/INV-005's authority (\"ambient nondeterminism\"), and the search "
            "engines are inside the enforced tier because a seed-dependent frontier yields a "
            "seed-dependent counterexample. Scope is the semantic-core tier; boundary crates are the "
            "capability providers and are excluded by design. The rule sees declared uses, not time or "
            "randomness reached through a third-party dependency."
        ),
        fn=rule_no_ambient_time_rng,
    ),
    Requirement(
        rid="GOV-1-05",
        summary="No platform-dependent hashing in canonical formats.",
        rule="no-platform-hashing",
        enforces=(
            "No `DefaultHasher`/`RandomState`/`BuildHasherDefault`/`SipHasher` and no `ahash`/`fxhash`/"
            "`rustc_hash`/`twox_hash`/`seahash`/`metrohash` symbol or dependency in the semantic core. "
            "The identity seam `crates/continuum-value/src/identity.rs` still declares `trait "
            "ContentHasher` and `struct HashAlgorithm` and imports no `std::hash`. No type in the "
            "canonical value module `crates/continuum-value/src/value.rs` derives or implements `Hash`, "
            "which is what makes hash-indexed state identity unrepresentable (ADR-0013)."
        ),
        scanned=(
            "crates/<semantic-core>/src/**/*.rs",
            IDENTITY_SEAM,
            CANONICAL_VALUE_MODULE,
            "crates/*/Cargo.toml",
        ),
        boundary=(
            "The hasher scan runs over the whole semantic-core tier, a superset of the canonical-format "
            "crates, so that half is wider than the bullet rather than narrower. The `Hash`-derive "
            "prohibition is scoped to the canonical value module alone: `continuum-value`'s epoch tokens "
            "and assurance dimensions, and `continuum-workspace`'s path and handle types, derive `Hash` "
            "legitimately — they are labels, not canonical states. The prohibition is on fingerprinting "
            "a *value*, not on being a map key."
        ),
        fn=rule_no_platform_hashing,
    ),
    Requirement(
        rid="GOV-1-06",
        summary="No network access in certificate checking.",
        rule="no-network-in-checker",
        enforces=(
            "The workspace-internal closure of the four `continuum-kernel-*` crates and "
            "`continuum-certificate` contains no network-capable member (`continuum-effects-network`, "
            "`continuum-asupersync`, `continuum-proof-client`, `continuumd`, any adapter); no crate in "
            "that closure declares a network or async-runtime dependency; no checker source names a "
            "socket type."
        ),
        scanned=("crates/*/Cargo.toml (transitive closure)", "crates/<checker>/src/**/*.rs"),
        boundary=(
            "Workspace-internal edges are exact. External dependencies are checked at the *declared* "
            "level: a third-party crate that itself pulls in a socket is not audited here. Deep "
            "transitive auditing of third-party crates is PR 9's kernel covenant tooling. The workspace "
            "declares one external dependency (blake3, dev-visible plus continuum-value's hasher seam, "
            "recorded in dependency-rationale.toml since bn-30eym); the external half is exercised by "
            "that entry and by its fixtures."
        ),
        fn=rule_no_network_in_checker,
    ),
    Requirement(
        rid="GOV-1-07",
        summary="Dependency additions require rationale and TCB classification.",
        rule="dependency-rationale",
        enforces=(
            f"Every dependency declared anywhere in the workspace has exactly one entry in "
            f"`{RATIONALE_PATH}` and every entry corresponds to a real dependency. Each entry states a "
            "one-line rationale, a `tcb_class` from {trusted-checking-base, engine, dev-only, "
            "build-only}, an accurate `origin`, and accurate `used_by`/`kinds` sets. A dependency linked "
            "by the certificate checker must be classified `trusted-checking-base`; a dependency used "
            "only in dev or build position must be classified `dev-only`/`build-only`."
        ),
        scanned=("crates/*/Cargo.toml", RATIONALE_PATH),
        boundary=(
            "The rule enforces that a rationale and a classification exist and that the classification "
            "is consistent with how the dependency is actually linked. Whether the rationale is a *good* "
            "reason is a review judgement no check can make."
        ),
        fn=rule_dependency_rationale,
    ),
    Requirement(
        rid="GOV-1-08",
        summary="Semantic changes require ADR.",
        rule="semantic-change-adr",
        enforces=(
            "`notes/plan/adr/README.md` and the ADR files are a bijection, so a decision cannot be "
            "landed unindexed or indexed without a file. Every ADR declares a docs/12 §2 status. Every "
            "`ADR-NNNN`/`RFC NNNN` citation in crate sources resolves to an existing record. Every "
            "semantic-core crate that carries code cites at least one numbered decision record in its "
            "`src/`."
        ),
        scanned=(f"{ADR_DIR}/*.md", f"{ADR_DIR}/README.md", f"{RFC_DIR}/*.md", "crates/**/*.rs"),
        boundary=(
            "This half reads one revision and enforces the artifact-level consequences: decisions are "
            "discoverable, statuses are declared, citations resolve, and semantic code names the record "
            "that authorized it. The half a single revision cannot see — that a *change* was semantic — "
            "is enforced against the merge base by `check_revision_delta.py`'s `semantic-change-adr-delta` "
            "rule (see `delta_gate`), at file granularity rather than this rule's crate granularity. What "
            "neither half decides is whether the cited record actually *governs* the change it is cited "
            "for; that remains the docs/12 §4 review obligation."
        ),
        fn=rule_semantic_change_adr,
        delta_gate=(
            f"{GOVERNANCE}/check_revision_delta.py rule `semantic-change-adr-delta` "
            "(semantic-change-cites-a-decision-record, added-semantic-source-cites-a-decision-record); "
            f"evidence {GOVERNANCE}/evidence/gov-1-delta.json"
        ),
    ),
    Requirement(
        rid="GOV-1-09",
        summary="Every breaking change increments semantic epoch.",
        rule="epoch-discipline",
        enforces=(
            "`schemas/README.md` still states the normative \"MUST advance `schema_epoch`\" rule and "
            "what advances an epoch. Every schema document's `$id` epoch segment equals its "
            "`schema_epoch`, and its pinned instance class identity equals its own class identity. Every "
            "example instance names an epoch its schema actually is. The six epoch kinds in "
            "`crates/continuum-value/src/epoch.rs` are exactly the six the schemas README names, and "
            "`EpochAdvance::new` cannot record an advance without a `Compatibility` verdict."
        ),
        scanned=(
            f"{SCHEMA_DIR}/README.md",
            f"{SCHEMA_DIR}/*.schema.json",
            f"{SCHEMA_DIR}/examples/*.json",
            "crates/continuum-value/src/epoch.rs",
        ),
        boundary=(
            "This half reads one revision and enforces that the epoch machinery cannot be bypassed or "
            "quietly desynchronized: identities agree, the vocabulary is one set, and an advance carries "
            "a published compatibility verdict. The breaking edit made *without* touching `$id` or "
            "`schema_epoch` — invisible to any single revision — is enforced against the merge base by "
            "`check_revision_delta.py`'s `epoch-discipline-delta` rule (see `delta_gate`). Two limits are "
            "stated there and are real: while `schemas/README.md` still carries its pre-freeze draft "
            "clause, an in-place edit at epoch 1 is *recorded and printed but not failed*, because the "
            "convention explicitly permits it until PR 5 freezes the interface; and the delta rule reads "
            "schema documents, so a breaking change made in the IDL or an RFC without touching a schema "
            "file is outside it."
        ),
        fn=rule_epoch_discipline,
        delta_gate=(
            f"{GOVERNANCE}/check_revision_delta.py rule `epoch-discipline-delta` "
            "(schema-bytes-changed-without-epoch-advance, published-schema-document-deleted, "
            "schema-epoch-never-retreats, epoch-advance-publishes-a-compatibility-statement); "
            f"evidence {GOVERNANCE}/evidence/gov-1-delta.json"
        ),
    ),
    Requirement(
        rid="GOV-1-10",
        summary="Every pack operation has a normative contract.",
        rule="pack-operation-contract",
        enforces=(
            "`domain-pack.schema.json` requires `operations`, requires name/command_type/response_type/"
            "effect_phases on every operation, and closes the operation object, so an operation without "
            "a contract is unrepresentable in a valid manifest. docs/17 §4's `OperationContract` fields "
            "and the schema's operation properties are checked against each other in both directions. "
            "Every domain-pack manifest in the dossier states the whole contract for every operation, "
            "including the parts the schema leaves optional (footprint_rule, cancellation_points, "
            "observer_events)."
        ),
        scanned=(PACK_SCHEMA, PACK_CONTRACT_DOC, f"{SCHEMA_DIR}/examples/*.json"),
        boundary=(
            "PROXY for one clause. \"Every pack operation\" is enforced over every pack manifest the "
            "repository contains — today that is the storage append-log manifest. A pack shipped "
            "out-of-tree is reached by the same schema but not by this run. Whether a stated contract is "
            "*correct* is the docs/17 §5 three-implementation refinement obligation, not a shape check."
        ),
        fn=rule_pack_operation_contract,
    ),
    Requirement(
        rid="GOV-1-11",
        summary="Reference semantics precedes optimization.",
        rule="reference-precedes-optimization",
        enforces=(
            "docs/01 §7.1 (Reference path) precedes §7.2 (Optimized paths) and still declares the "
            "reference path to define differential truth; §7.2 still requires every optimized path to "
            "emit evidence checked outside the search implementation. `continuum-engine-reference` "
            "exists, declares itself the differential oracle, and has no workspace dependency — direct "
            "or transitive — on any optimized engine. Every optimized engine crate still states plan "
            "§20's serialization boundary: its results reach the trust base only as certificates "
            "\"checked from wire form\", so an optimized path never becomes the meaning."
        ),
        scanned=(ARCHITECTURE, "crates/continuum-engine-*/src/**/*.rs", "crates/*/Cargo.toml"),
        boundary=(
            "PROXY. Temporal precedence between a specific optimization and its reference semantics is "
            "not observable from one revision. What is enforced is the structural form of the rule: the "
            "oracle exists, is independent of the things it judges, and no optimized path claims its own "
            "output as truth. The two documentation sub-checks (docs/01 §7.1/§7.2 wording, and the "
            "per-engine serialization-boundary sentence) prove a normative claim is still made, not that "
            "any differential campaign was run — that evidence is docs/19's, not this check's."
        ),
        fn=rule_reference_precedes_optimization,
    ),
    Requirement(
        rid="GOV-1-12",
        summary="Ambiguous behavior is an error, not implementation freedom.",
        rule="ambiguity-is-an-error",
        enforces=(
            "The constitution still states the rule. Every schema document closes its top level, so an "
            "unrecognized field is rejected rather than given a local meaning. Every artifact schema "
            "requires and `const`-pins `schema_id` and `schema_epoch`, so an instance always names the "
            "contract it was written under. The assurance verdict vocabulary is closed at exactly "
            "{established, refuted, inconclusive}, and an inconclusive verdict is required to carry its "
            "reason — in the schema conditionally, and in `continuum-task` structurally, where "
            "`TaskOutcome::Inconclusive` takes a non-optional `InconclusiveReason`."
        ),
        scanned=(
            CONSTITUTION,
            f"{SCHEMA_DIR}/*.schema.json",
            ASSURANCE_SCHEMA,
            TASK_RESULT,
        ),
        boundary=(
            "PROXY. \"Ambiguous behavior\" is a property of a specification, and no program decides "
            "whether prose is ambiguous. This enforces the mechanical instances the dossier already "
            "commits to: closed schemas, named contracts, a closed verdict vocabulary, and a typed "
            "inconclusive. Nested schema objects are *not* required to be closed — several legitimately "
            "carry open maps — so the closure check is top-level only."
        ),
        fn=rule_ambiguity_is_an_error,
    ),
)

RULES = {req.rule: req for req in REQUIREMENTS}


# ============================================================================
# Fixtures
# ============================================================================


@dataclass(frozen=True)
class Fixture:
    fid: str
    requirement: str
    rule: str
    check: str | None
    description: str
    overlay: dict[str, str]
    removed: tuple[str, ...]
    directory: str


def load_fixtures(tree: Tree) -> tuple[list[Fixture], list[str]]:
    """Read every fixture description. Fixtures are inert data, never compiled."""
    fixtures: list[Fixture] = []
    problems: list[str] = []
    for rel in sorted(tree.glob(f"{FIXTURE_DIR}/*/fixture.json")):
        directory = str(Path(rel).parent)
        spec = tree.read_json(rel)
        if not isinstance(spec, dict):
            problems.append(f"{rel} is not a JSON object")
            continue
        overlay: dict[str, str] = {}
        ok = True

        for target, payload in sorted((spec.get("overlay") or {}).items()):
            text = tree.read_text(f"{directory}/{payload}")
            if text is None:
                problems.append(f"{rel}: overlay payload {payload!r} is missing")
                ok = False
                continue
            overlay[target] = text

        for target, edits in sorted((spec.get("substitute") or {}).items()):
            base = overlay.get(target, tree.read_text(target))
            if base is None:
                problems.append(f"{rel}: substitute target {target!r} does not exist")
                ok = False
                continue
            for edit in edits:
                find, replace = edit["find"], edit["replace"]
                count = base.count(find)
                if count != 1:
                    problems.append(
                        f"{rel}: substitution anchor for {target!r} matches {count} times, expected 1 "
                        f"— the fixture is stale: {find[:60]!r}"
                    )
                    ok = False
                    break
                base = base.replace(find, replace, 1)
            overlay[target] = base

        for target in spec.get("remove") or []:
            if not tree.exists(target):
                problems.append(f"{rel}: remove target {target!r} does not exist")
                ok = False

        if not ok:
            continue
        fixtures.append(
            Fixture(
                fid=spec.get("id", Path(directory).name),
                requirement=spec["requirement"],
                rule=spec["rule"],
                check=spec.get("check"),
                description=spec["description"],
                overlay=overlay,
                removed=tuple(spec.get("remove") or ()),
                directory=directory,
            )
        )
    return fixtures, problems


# ============================================================================
# Runner
# ============================================================================


def run_rules(tree: Tree) -> dict[str, list[Violation]]:
    return {req.rid: sorted(req.fn(tree)) for req in REQUIREMENTS}


def collect_waiver_report(tree: Tree) -> list[dict[str, object]]:
    """Every granted inline waiver, so a waiver is never invisible."""
    granted: list[dict[str, object]] = []
    for rel in sorted(set(tree.glob("crates/**/*.rs"))):
        raw = tree.read_text(rel)
        if raw is None:
            continue
        _, waivers, _ = collect_waivers(rel, raw)
        granted.extend(
            {"path": w.path, "line": w.line, "check": w.check, "reason": w.reason} for w in waivers
        )
    return granted


def self_test(tree: Tree) -> tuple[bool, dict[str, object]]:
    """Replay every fixture through the real rules; fail if any goes uncaught."""
    fixtures, problems = load_fixtures(tree)
    failures: list[str] = list(problems)

    baseline = run_rules(tree)
    dirty = {rid: [v.render() for v in vs] for rid, vs in baseline.items() if vs}
    if dirty:
        failures.append(f"the real repository already violates {sorted(dirty)}; fixtures prove nothing on it")

    covered: dict[str, list[str]] = {req.rid: [] for req in REQUIREMENTS}
    for fixture in sorted(fixtures, key=lambda f: f.fid):
        req = RULES.get(fixture.rule)
        if req is None:
            failures.append(f"{fixture.fid}: unknown rule {fixture.rule!r}")
            continue
        if req.rid != fixture.requirement:
            failures.append(
                f"{fixture.fid}: rule {fixture.rule!r} belongs to {req.rid}, not {fixture.requirement}"
            )
            continue
        found = req.fn(tree.with_changes(fixture.overlay, fixture.removed))
        if not found:
            failures.append(f"{fixture.fid}: {req.rid} rule `{req.rule}` did not catch it")
            continue
        if fixture.check and not any(v.check == fixture.check for v in found):
            failures.append(
                f"{fixture.fid}: caught, but not by check {fixture.check!r} "
                f"(fired: {sorted({v.check for v in found})})"
            )
            continue
        covered[req.rid].append(fixture.fid)

    for req in REQUIREMENTS:
        if not covered[req.rid]:
            failures.append(f"{req.rid} has no violating fixture; the rule could pass vacuously")

    report = {
        "status": "fail" if failures else "pass",
        "fixtures": len(fixtures),
        "requirements_covered": sum(1 for v in covered.values() if v),
        "fixtures_by_requirement": {k: sorted(v) for k, v in sorted(covered.items())},
        "failures": sorted(failures),
    }
    return (not failures), report


def build_evidence(tree: Tree, results: dict[str, list[Violation]], st: dict[str, object]) -> dict:
    """A deterministic function of the repository — no clock, no commit id.

    Re-running the check on an unchanged tree rewrites the file byte-for-byte, so
    a diff in `evidence/gov-1.json` always means the policy state moved.
    """
    fixtures_by_req = st.get("fixtures_by_requirement", {})
    workspace = load_workspace(tree)
    requirements: dict[str, object] = {}
    for req in REQUIREMENTS:
        violations = results[req.rid]
        requirements[req.rid] = {
            "summary": req.summary,
            "source": f"{CONSTITUTION} §1",
            "rule": req.rule,
            "enforces": req.enforces,
            "scanned": list(req.scanned),
            "result": "fail" if violations else "pass",
            "violations": [v.render() for v in violations],
            "fixtures_proven_caught": list(fixtures_by_req.get(req.rid, [])),
            "boundary": req.boundary,
            "delta_gate": req.delta_gate,
        }
    return {
        "artifact": "continuum.governance.evidence/gov-1",
        "produced_by": f"{GOVERNANCE}/check_code_policy.py",
        "reproduce": f"python3 {GOVERNANCE}/check_code_policy.py --evidence {GOVERNANCE}/evidence/gov-1.json",
        "authority": {
            "constitution": f"{CONSTITUTION} §1 (Repository constitution)",
            "requirement_ids": "notes/plan/notes/PLAN_REQUIREMENTS.json",
        },
        "determinism": (
            "This file records no timestamp, commit id, or absolute path: it is a function of the "
            "repository contents alone, so an unchanged tree reproduces it byte-for-byte."
        ),
        "scope": {
            "workspace_members": len(workspace.members),
            "semantic_core_crates": sorted(SEMANTIC_CORE),
            "boundary_crates": sorted(BOUNDARY),
            "adapter_crates": sorted(ADAPTERS),
            "certificate_checking_base": sorted(CHECKER),
        },
        "self_test": st,
        "waivers_granted": collect_waiver_report(tree),
        "requirements": requirements,
        "status": "fail" if any(results.values()) else "pass",
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="replay every violating fixture through the rules and fail if any goes uncaught",
    )
    parser.add_argument(
        "--evidence",
        metavar="PATH",
        help="run the self-test and the real check, then write the evidence JSON to PATH",
    )
    parser.add_argument("--quiet", action="store_true", help="print only the status line")
    args = parser.parse_args(argv)

    tree = Tree(ROOT)

    if args.self_test and not args.evidence:
        ok, report = self_test(tree)
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0 if ok else 1

    st: dict[str, object] = {"status": "not-run"}
    st_ok = True
    if args.evidence:
        st_ok, st = self_test(tree)

    results = run_rules(tree)
    failed = {rid: [v.render() for v in vs] for rid, vs in results.items() if vs}

    if args.evidence:
        evidence = build_evidence(tree, results, st)
        path = Path(args.evidence)
        if not path.is_absolute():
            path = ROOT / path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    report = {
        "requirements": len(REQUIREMENTS),
        "passing": sorted(rid for rid in results if not results[rid]),
        "failing": {k: v for k, v in sorted(failed.items())},
        "self_test": st.get("status"),
        "evidence": args.evidence,
        "status": "fail" if failed or not st_ok else "pass",
    }
    if args.quiet:
        print(report["status"])
    else:
        print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if failed or not st_ok else 0


if __name__ == "__main__":
    raise SystemExit(main())
