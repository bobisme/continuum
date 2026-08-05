#!/usr/bin/env python3
"""Bind risk register R16 ("Rust compiler churn") to its live enforcement.

Source of truth:

- `notes/plan/docs/08_RISK_REGISTER.md` R16 — five required controls: pinned
  toolchain; narrow compiler-internal usage; extraction adapters isolated;
  compatibility CI; prefer stable metadata over rustc internals where
  possible.

None of R16's five controls has a single existing enforcer of its own; each
is a composite of (a) a direct, narrow check this file owns because no other
requirement id owns it, and/or (b) the *actual* self-test and real run of a
checker that already owns an adjacent, overlapping obligation. As in
`check_t12_evidence.py`, "citing" a control here means re-running its
enforcer, not copying a status string out of a possibly-stale evidence file,
so a control that regresses between commits fails here too.

Direct checks this file owns
-----------------------------

    rustc-internal-usage
        Greps every `crates/**/*.rs` and `crates/*/Cargo.toml` (plus the root
        `Cargo.toml`) for nightly feature gates (`#![feature(...)]`), the
        `rustc_private` crate-linkage escape hatch, direct dependence on any
        `rustc_*` compiler-internal crate, `#[rustc_*]` internal attributes,
        `proc_macro::` (the compiler's own proc-macro ABI, as opposed to a
        third-party proc-macro crate), `RUSTC_BOOTSTRAP` (the env var that
        unlocks nightly-only features on a stable compiler), a Cargo
        `cargo-features = [...]` unstable-features table, and `proc-macro =
        true` library declarations. Also inventories `build.rs` build
        scripts. The count this file pins is zero for every pattern; "narrow
        compiler-internal usage" is satisfied by *no* usage, and "prefer
        stable metadata over rustc internals where possible" is the same
        fact read the other way — a zero count exceeds "prefer".

    sanitizer-lane-excluded-from-pinned-gate
        Parses `Justfile` and asserts (a) the `check:` recipe's dependency
        list — the pinned-toolchain default gate — does not pull in
        `sanitizers`, and (b) a `sanitizers` recipe still exists, parameterized
        by a `toolchain` argument rather than a hardcoded one. This is the
        mechanical half of the "pinned toolchain" control's boundary: the
        sanitizer lane (bn-cho5) is the one place this workspace deliberately
        runs a nightly compiler (`-Zsanitizer`, `-Zbuild-std`), and the
        control is that it stays reachable only by explicit invocation, never
        by `just check`.

    extraction-boundary-zero-lean-edges
        Scans every workspace crate manifest for a dependency whose name
        starts with `lean` (case-insensitive) and every crate source file for
        a subprocess invocation of `lake`, `lean`, or `elan`. The count is
        zero: no Rust crate holds a build, normal, dev, or FFI edge to the
        Lean toolchain today. This is the mechanical half of "extraction
        adapters isolated" — see the module docstring's honesty note below
        for what it does not yet claim.

    ci-runs-pinned-gate
        Reads `.github/workflows/check.yml` and asserts it triggers on both
        `push` and `pull_request` against `main`, selects the pinned
        toolchain (`rustup show`, which resolves `rust-toolchain.toml`), and
        runs `just check`. This is "compatibility CI": the mechanical fact
        that an unreviewed change to the pinned build is caught before merge,
        not only whenever a human happens to run the gate locally.

Delegates re-run for real (not re-implemented)
------------------------------------------------

    check_code_policy.py GOV-1-01 (toolchain-pinned) and GOV-1-02
        (unsafe-forbidden, the adjacent stable-metadata discipline: no
        source-level escape from the workspace-wide compiler-enforced lint).
    check_t12_evidence.py "pinned lockfile" and "reproducible build metadata"
        (Cargo.lock committed, every non-fmt cargo gate carries `--locked`,
        `[profile.release] overflow-checks = true`) — the INV-005 ambient-
        nondeterminism determinism-matrix half of this is bn-jme9's; cited,
        not duplicated, here or there.
    check_kernel_covenant.py KCOV-03 (kernel-is-synchronous / no async),
        KCOV-04 (no-unsafe), KCOV-07 (no-plugins-or-dynamic-loading) — the
        three kernel-covenant lint walls bn-2he landed over the trusted
        checking base. A compiler-internal or plugin-shaped extraction
        surface could not enter the kernel crates even if someone tried.
    check_crate_boundaries.py overall pass — no forbidden dependency edge
        exists anywhere in the workspace graph, `kernel-is-synchronous` and
        `certificate-checker-not-search` included.

Honesty about limits
---------------------

- "Extraction adapters isolated" is checked as *zero live edges*, which is
  true today because the Lean proof-service client (`continuum-proof-client`,
  plan §20's reserved boundary crate) is still the PR-4A/IMPL-01-era scaffold
  named in its own module doc — it declares no dependencies and speaks to
  nothing yet (PR 28, Phase D, not delivered). A zero-edge count is real
  evidence now, but it is evidence of "nothing has been built to leak
  through," not evidence that a boundary would hold once PR 28 lands. This
  file's own re-run catches regression before that day; the day PR 28 lands,
  the rule here needs a positive successor (e.g. "only continuum-proof-client
  may invoke `lake`/`lean`"), which is out of this bone's scope to invent
  ahead of the code it would govern.
- `check_crate_boundaries.py` has no rule *named* for Lean because no crate
  reaches it yet — there is nothing for a forbidden-edge rule to forbid. Its
  "overall pass" citation here is boundary-adjacent evidence (the kernel
  covenant and certificate-checker isolation it does enforce), not a
  Lean-specific rule.
- "Compatibility CI" is read as "an unreviewed change is validated against
  the pinned build before merge," not as multi-toolchain (beta/nightly)
  compatibility testing — this workspace's INV-014 posture is one pinned
  toolchain identity, not a compatibility matrix, and R16's sibling control
  ("prefer stable metadata over rustc internals") points the same way.

Stdlib only; the only subprocesses run are the delegate checkers themselves
(each already `python3 tools/...` invocations `just check` runs), never a
network client. Exit 0 when every control's citations pass, 1 otherwise.

Usage:

    python3 tools/governance/check_r16_evidence.py --self-test
    python3 tools/governance/check_r16_evidence.py
    python3 tools/governance/check_r16_evidence.py --evidence tools/governance/evidence/r16.json
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
EVIDENCE = "tools/governance/evidence/r16.json"
RISK_REGISTER = "notes/plan/docs/08_RISK_REGISTER.md"

DELEGATES: dict[str, str] = {
    "check_code_policy": "tools/governance/check_code_policy.py",
    "check_t12_evidence": "tools/governance/check_t12_evidence.py",
    "check_kernel_covenant": "tools/check_kernel_covenant.py",
    "check_crate_boundaries": "tools/check_crate_boundaries.py",
}

PRUNED_DIRS = frozenset({".git", ".maw", ".manifold", "repo.git", "target", ".lake", "__pycache__", ".venv", "node_modules"})


# ============================================================================
# Repository view (fixture overlay)
# ============================================================================


@dataclass
class Tree:
    root: Path
    overlay: dict[str, str] = field(default_factory=dict)
    removed: frozenset[str] = frozenset()
    _index: tuple[str, ...] | None = field(default=None, repr=False, compare=False)

    def with_changes(self, overlay: dict[str, str], removed: tuple[str, ...] = ()) -> "Tree":
        merged = {**self.overlay, **overlay}
        return Tree(self.root, merged, frozenset(self.removed) | frozenset(removed))

    def _real_index(self) -> tuple[str, ...]:
        if self._index is None:
            found: list[str] = []
            for name in sorted(os.listdir(self.root)):
                path = self.root / name
                if path.is_file():
                    found.append(name)
            base = self.root / "crates"
            if base.is_dir():
                for dirpath, dirnames, filenames in os.walk(base):
                    dirnames[:] = sorted(d for d in dirnames if d not in PRUNED_DIRS)
                    rel_dir = Path(dirpath).relative_to(self.root).as_posix()
                    for fname in sorted(filenames):
                        found.append(f"{rel_dir}/{fname}")
            self._index = tuple(sorted(found))
        return self._index

    def read_text(self, rel: str) -> str | None:
        if rel in self.removed:
            return None
        if rel in self.overlay:
            return self.overlay[rel]
        path = self.root / rel
        if not path.is_file():
            return None
        return path.read_text(encoding="utf-8")

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
        else:
            out.append(re.escape(char))
            i += 1
    return re.compile("".join(out))


@dataclass(frozen=True, order=True)
class Violation:
    rule: str
    message: str

    def render(self) -> str:
        return f"[{self.rule}] {self.message}"


# ============================================================================
# Direct check 1 — rustc-internal-usage (narrow compiler-internal usage /
# prefer stable metadata over rustc internals)
# ============================================================================

RUSTC_INTERNAL_SOURCE_PATTERNS = {
    "nightly feature gate (#![feature(...)])": r"#!\s*\[\s*feature\s*\(",
    "nightly feature gate (#[feature(...)])": r"#\s*\[\s*feature\s*\(",
    "rustc_private": r"\brustc_private\b",
    "extern crate rustc_*": r"\bextern\s+crate\s+rustc_\w+",
    "rustc compiler-internal crate path": r"\brustc_(driver|interface|ast|hir|middle|session|span|"
    r"data_structures|errors|lint|metadata|resolve|target|query|mir|trait_selection)\s*::",
    "#[rustc_*] internal attribute": r"#\s*\[\s*rustc_\w+",
    "proc_macro:: (compiler ABI, not a third-party proc-macro crate)": r"\bproc_macro\s*::",
    "RUSTC_BOOTSTRAP (nightly-feature unlock on stable)": r"\bRUSTC_BOOTSTRAP\b",
}


def rule_rustc_internal_usage(tree: Tree) -> list[Violation]:
    rule = "rustc-internal-usage"
    out: list[Violation] = []

    sources = sorted(set(tree.glob("crates/**/*.rs")))
    for rel in sources:
        raw = tree.read_text(rel)
        if raw is None:
            continue
        for label, pattern in sorted(RUSTC_INTERNAL_SOURCE_PATTERNS.items()):
            for match in re.finditer(pattern, raw):
                lineno = raw.count("\n", 0, match.start()) + 1
                out.append(Violation(rule, f"{rel}:{lineno} uses `{label}` — R16 pins this count at zero"))

    manifests = sorted(set(tree.glob("crates/*/Cargo.toml")) | {"Cargo.toml"})
    for rel in manifests:
        raw = tree.read_text(rel)
        if raw is None:
            continue
        if re.search(r"(?m)^\s*cargo-features\s*=", raw):
            out.append(Violation(rule, f"{rel} declares an unstable Cargo `cargo-features` table"))
        doc = tree.read_toml(rel)
        if doc is not None:
            lib = doc.get("lib", {})
            if isinstance(lib, dict) and lib.get("proc-macro") is True:
                out.append(Violation(rule, f"{rel} declares `[lib] proc-macro = true`"))

    build_scripts = sorted(set(tree.glob("crates/*/build.rs")) | set(tree.glob("build.rs")))
    for rel in build_scripts:
        out.append(Violation(rule, f"{rel} is a build script — R16's proc-macro/build-script inventory is zero"))
    return out


# ============================================================================
# Direct check 2 — sanitizer-lane-excluded-from-pinned-gate (pinned toolchain)
# ============================================================================

CHECK_RECIPE_ANCHOR = "check: fmt-check lint test boundaries covenant governance dossier lean"
SANITIZERS_RECIPE_ANCHOR = re.compile(r"(?m)^sanitizers\s")


def rule_sanitizer_lane_excluded(tree: Tree) -> list[Violation]:
    rule = "sanitizer-lane-excluded-from-pinned-gate"
    text = tree.read_text("Justfile")
    if text is None:
        return [Violation(rule, "Justfile is missing")]
    out: list[Violation] = []

    check_line = None
    for line in text.splitlines():
        if line.startswith("check:"):
            check_line = line
            break
    if check_line is None:
        out.append(Violation(rule, "Justfile declares no `check:` recipe"))
    else:
        deps = check_line[len("check:"):].split()
        if "sanitizers" in deps:
            out.append(
                Violation(
                    rule,
                    "Justfile `check:` recipe pulls in `sanitizers`; the sanitizer lane is "
                    "nightly-only (-Zsanitizer, -Zbuild-std) and must stay outside the "
                    "pinned-toolchain default gate (bn-cho5)",
                )
            )

    if not SANITIZERS_RECIPE_ANCHOR.search(text):
        out.append(
            Violation(
                rule,
                "Justfile declares no `sanitizers` recipe; the nightly compiler lane bn-cho5 "
                "landed is not reachable at all",
            )
        )
    return out


# ============================================================================
# Direct check 3 — extraction-boundary-zero-lean-edges (extraction adapters
# isolated)
# ============================================================================

LEAN_DEP_NAME = re.compile(r"^lean", re.IGNORECASE)
LEAN_SUBPROCESS = re.compile(r'Command::new\s*\(\s*"(lake|lean|elan)"\s*\)')
DEP_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")


def rule_extraction_boundary(tree: Tree) -> list[Violation]:
    rule = "extraction-boundary-zero-lean-edges"
    out: list[Violation] = []

    for rel in sorted(tree.glob("crates/*/Cargo.toml")):
        doc = tree.read_toml(rel)
        if doc is None:
            continue
        for table in DEP_TABLES:
            for key in (doc.get(table) or {}):
                if isinstance(key, str) and LEAN_DEP_NAME.match(key):
                    out.append(
                        Violation(
                            rule,
                            f"{rel} [{table}] declares `{key}`, a Lean-named dependency edge; "
                            "the extraction/proof-service boundary must stay zero-edge until "
                            "PR 28 lands it deliberately",
                        )
                    )

    for rel in sorted(tree.glob("crates/**/*.rs")):
        raw = tree.read_text(rel)
        if raw is None:
            continue
        for match in LEAN_SUBPROCESS.finditer(raw):
            lineno = raw.count("\n", 0, match.start()) + 1
            out.append(
                Violation(
                    rule,
                    f"{rel}:{lineno} spawns `{match.group(1)}` as a subprocess; no crate may "
                    "reach the Lean toolchain directly outside the PR-28 proof-service client",
                )
            )
    return out


# ============================================================================
# Direct check 4 — ci-runs-pinned-gate (compatibility CI)
# ============================================================================

CI_WORKFLOW = ".github/workflows/check.yml"


def rule_ci_runs_pinned_gate(tree: Tree) -> list[Violation]:
    rule = "ci-runs-pinned-gate"
    text = tree.read_text(CI_WORKFLOW)
    if text is None:
        return [Violation(rule, f"{CI_WORKFLOW} is missing; compatibility CI has no enforcement point (bn-28ur)")]

    out: list[Violation] = []
    if "push:" not in text:
        out.append(Violation(rule, f"{CI_WORKFLOW} declares no `push:` trigger"))
    if "pull_request:" not in text:
        out.append(Violation(rule, f"{CI_WORKFLOW} declares no `pull_request:` trigger"))
    if "branches: [main]" not in text:
        out.append(Violation(rule, f"{CI_WORKFLOW} does not scope its triggers to `main`"))
    # A `run:` step line, not merely a comment discussing `rustup show`/`just
    # check` (the file's own header prose mentions both by name at length).
    if not re.search(r"(?m)^\s*run:\s*rustup show\s*$", text):
        out.append(
            Violation(
                rule,
                f"{CI_WORKFLOW} has no `run: rustup show` step; nothing selects the pinned "
                "`rust-toolchain.toml` channel before the gate runs",
            )
        )
    if not re.search(r"(?m)^\s*run:\s*just check\s*$", text):
        out.append(Violation(rule, f"{CI_WORKFLOW} has no `run: just check` step"))
    return out


def direct_checks(tree: Tree) -> dict[str, list[Violation]]:
    return {
        "rustc-internal-usage": rule_rustc_internal_usage(tree),
        "sanitizer-lane-excluded-from-pinned-gate": rule_sanitizer_lane_excluded(tree),
        "extraction-boundary-zero-lean-edges": rule_extraction_boundary(tree),
        "ci-runs-pinned-gate": rule_ci_runs_pinned_gate(tree),
    }


# ============================================================================
# Self-test
# ============================================================================


def direct_self_test(tree: Tree) -> dict[str, object]:
    failures: list[str] = []
    baseline = direct_checks(tree)
    dirty_baseline = {k: [v.render() for v in vs] for k, vs in baseline.items() if vs}
    if dirty_baseline:
        failures.append(f"the real repository already violates direct checks {dirty_baseline}")

    caught: list[str] = []

    # rustc-internal-usage: a planted nightly feature gate in a new fixture file.
    dirty = tree.with_changes({"crates/_r16_fixture/src/lib.rs": "#![feature(rustc_private)]\nfn f() {}\n"})
    if not [v for v in rule_rustc_internal_usage(dirty) if v.rule == "rustc-internal-usage"]:
        failures.append("fixture: a planted `#![feature(rustc_private)]` was not caught")
    else:
        caught.append("rustc-internal-usage/feature-gate")

    # rustc-internal-usage: a planted proc_macro:: compiler-ABI use.
    dirty = tree.with_changes({"crates/_r16_fixture/src/lib.rs": "fn f() { let _ = proc_macro::TokenStream::new(); }\n"})
    if not rule_rustc_internal_usage(dirty):
        failures.append("fixture: a planted `proc_macro::` use was not caught")
    else:
        caught.append("rustc-internal-usage/proc-macro-abi")

    # rustc-internal-usage: a planted proc-macro = true library declaration.
    dirty = tree.with_changes(
        {"crates/_r16_fixture/Cargo.toml": '[package]\nname = "r16-fixture"\n\n[lib]\nproc-macro = true\n'}
    )
    if not rule_rustc_internal_usage(dirty):
        failures.append("fixture: a planted `[lib] proc-macro = true` was not caught")
    else:
        caught.append("rustc-internal-usage/proc-macro-lib")

    # rustc-internal-usage: a planted build.rs.
    dirty = tree.with_changes({"crates/_r16_fixture/build.rs": "fn main() {}\n"})
    if not rule_rustc_internal_usage(dirty):
        failures.append("fixture: a planted build.rs was not caught")
    else:
        caught.append("rustc-internal-usage/build-script")

    # sanitizer-lane-excluded-from-pinned-gate: check: pulls in sanitizers.
    justfile = tree.read_text("Justfile") or ""
    if justfile.count(CHECK_RECIPE_ANCHOR) != 1:
        failures.append(f"sanitizer fixture anchor {CHECK_RECIPE_ANCHOR!r} matches {justfile.count(CHECK_RECIPE_ANCHOR)} times, expected 1")
    else:
        dirty_j = justfile.replace(CHECK_RECIPE_ANCHOR, CHECK_RECIPE_ANCHOR + " sanitizers", 1)
        if not rule_sanitizer_lane_excluded(tree.with_changes({"Justfile": dirty_j})):
            failures.append("fixture: `sanitizers` folded into `check:` was not caught")
        else:
            caught.append("sanitizer-lane-excluded-from-pinned-gate/pulled-into-check")

    if not SANITIZERS_RECIPE_ANCHOR.search(justfile):
        failures.append("sanitizer fixture: the real Justfile has no `sanitizers` recipe to remove for the fixture")
    else:
        dirty_j = SANITIZERS_RECIPE_ANCHOR.sub("notsanitizers ", justfile, count=1)
        if not rule_sanitizer_lane_excluded(tree.with_changes({"Justfile": dirty_j})):
            failures.append("fixture: removing the `sanitizers` recipe entirely was not caught")
        else:
            caught.append("sanitizer-lane-excluded-from-pinned-gate/recipe-removed")

    # extraction-boundary-zero-lean-edges: a planted dependency edge.
    dirty = tree.with_changes(
        {"crates/_r16_fixture/Cargo.toml": '[package]\nname = "r16-fixture"\n\n[dependencies]\nlean-sys = "0.1"\n'}
    )
    if not rule_extraction_boundary(dirty):
        failures.append("fixture: a planted `lean-sys` dependency was not caught")
    else:
        caught.append("extraction-boundary-zero-lean-edges/dependency")

    # extraction-boundary-zero-lean-edges: a planted subprocess invocation.
    dirty = tree.with_changes(
        {"crates/_r16_fixture/src/lib.rs": 'fn f() { std::process::Command::new("lake").spawn().ok(); }\n'}
    )
    if not rule_extraction_boundary(dirty):
        failures.append("fixture: a planted `Command::new(\"lake\")` was not caught")
    else:
        caught.append("extraction-boundary-zero-lean-edges/subprocess")

    # ci-runs-pinned-gate: missing rustup show / just check / removed entirely.
    ci = tree.read_text(CI_WORKFLOW)
    if ci is None:
        failures.append(f"ci fixture: {CI_WORKFLOW} is absent in the real tree, nothing to mutate")
    else:
        anchor = "run: rustup show"
        if ci.count(anchor) != 1:
            failures.append(f"ci fixture anchor {anchor!r} matches {ci.count(anchor)} times, expected 1")
        else:
            dirty_ci = ci.replace(anchor, "run: true", 1)
            if not rule_ci_runs_pinned_gate(tree.with_changes({CI_WORKFLOW: dirty_ci})):
                failures.append("fixture: dropping `rustup show` from CI was not caught")
            else:
                caught.append("ci-runs-pinned-gate/no-toolchain-select")

        anchor = "run: just check"
        if ci.count(anchor) != 1:
            failures.append(f"ci fixture anchor {anchor!r} matches {ci.count(anchor)} times, expected 1")
        else:
            dirty_ci = ci.replace(anchor, "run: true", 1)
            if not rule_ci_runs_pinned_gate(tree.with_changes({CI_WORKFLOW: dirty_ci})):
                failures.append("fixture: dropping `just check` from CI was not caught")
            else:
                caught.append("ci-runs-pinned-gate/no-check-step")

        if not rule_ci_runs_pinned_gate(tree.with_changes({}, removed=(CI_WORKFLOW,))):
            failures.append("fixture: removing the CI workflow entirely was not caught")
        else:
            caught.append("ci-runs-pinned-gate/file-removed")

    return {"status": "fail" if failures else "pass", "checks_caught": sorted(caught), "failures": failures}


def run_delegate(rel: str, *args: str) -> tuple[int, dict | None, str]:
    path = ROOT / rel
    proc = subprocess.run(
        [sys.executable, str(path), *args],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    try:
        data = json.loads(proc.stdout) if proc.stdout.strip() else None
    except json.JSONDecodeError:
        data = None
    return proc.returncode, data, proc.stdout


def delegate_self_test() -> dict[str, object]:
    results: dict[str, object] = {}
    failed: list[str] = []
    for name, rel in sorted(DELEGATES.items()):
        code, data, _ = run_delegate(rel, "--self-test")
        report = data if isinstance(data, dict) else {}
        # check_crate_boundaries.py's self-test reports its verdict under
        # `self_test` rather than `status`, like every other delegate.
        status = report.get("status", report.get("self_test"))
        ok = code == 0 and status == "pass"
        results[name] = {"exit_code": code, "status": status}
        if not ok:
            failed.append(f"{name} --self-test did not pass cleanly (exit {code}, status {status!r})")
    return {"status": "fail" if failed else "pass", "delegates": results, "failures": failed}


def delegate_real_run() -> dict[str, dict]:
    reports: dict[str, dict] = {}
    for name, rel in sorted(DELEGATES.items()):
        _, data, _ = run_delegate(rel)
        reports[name] = data or {"status": "fail", "note": "delegate produced no parseable JSON report"}
    return reports


def rid_status(report: dict, rid: str) -> str:
    if rid in report.get("passing", ()):
        return "pass"
    if rid in report.get("failing", {}):
        return "fail"
    return "unknown"


# ============================================================================
# The five-control binding
# ============================================================================


@dataclass(frozen=True)
class Citation:
    artifact: str
    enforces: str
    status: str


def build_controls(direct: dict[str, list[Violation]], reports: dict[str, dict]) -> dict[str, dict]:
    gov1 = reports["check_code_policy"]
    t12 = reports["check_t12_evidence"]
    kcov = reports["check_kernel_covenant"]
    boundaries = reports["check_crate_boundaries"]

    def direct_status(check: str) -> str:
        return "fail" if direct.get(check) else "pass"

    t12_controls = t12.get("controls", {}) if isinstance(t12.get("controls"), dict) else {}

    controls: dict[str, list[Citation]] = {
        "pinned toolchain": [
            Citation(
                "tools/governance/check_code_policy.py GOV-1-01",
                "rust-toolchain.toml pins an exact major.minor.patch channel; every crate "
                "inherits one workspace edition",
                rid_status(gov1, "GOV-1-01"),
            ),
            Citation(
                "tools/governance/check_t12_evidence.py 'pinned lockfile'",
                "Cargo.lock present and committed; every non-fmt cargo gate carries --locked",
                str(t12_controls.get("pinned lockfile", "unknown")),
            ),
            Citation(
                "tools/governance/check_t12_evidence.py 'reproducible build metadata'",
                "toolchain pin + --locked gates + overflow-checks=true + INV-014 receipt "
                "build identity (KCOV-09) as one composite obligation",
                str(t12_controls.get("reproducible build metadata", "unknown")),
            ),
            Citation(
                "tools/governance/check_r16_evidence.py sanitizer-lane-excluded-from-pinned-gate",
                "the one deliberately nightly lane (bn-cho5's ASan/TSan sweep) sits outside "
                "`just check`'s dependency chain; its own dated-pin residual (a floating "
                "`nightly` toolchain arg, parameterized so a dated pin can be supplied) is "
                "recorded there, not re-litigated here",
                direct_status("sanitizer-lane-excluded-from-pinned-gate"),
            ),
        ],
        "narrow compiler-internal usage": [
            Citation(
                "tools/governance/check_r16_evidence.py rustc-internal-usage",
                "zero nightly feature gates, zero rustc_private/rustc_* compiler-internal "
                "crate references, zero #[rustc_*] attributes, zero proc_macro:: (compiler "
                "ABI) use, zero RUSTC_BOOTSTRAP, zero cargo-features unstable tables, zero "
                "proc-macro=true libraries, zero build.rs scripts",
                direct_status("rustc-internal-usage"),
            ),
            Citation(
                "tools/governance/check_code_policy.py GOV-1-02",
                "unsafe forbidden workspace-wide with no crate- or source-level relaxation "
                "(the adjacent stable-metadata discipline: no compiler-trust escape hatch "
                "of any kind, not just the nightly-feature one)",
                rid_status(gov1, "GOV-1-02"),
            ),
        ],
        "extraction adapters isolated": [
            Citation(
                "tools/governance/check_r16_evidence.py extraction-boundary-zero-lean-edges",
                "zero workspace crate declares a Lean-named dependency edge and zero source "
                "file spawns lake/lean/elan as a subprocess; continuum-proof-client (plan "
                "§20's reserved boundary crate, PR 28/Phase D) is a docs-only scaffold with "
                "no dependencies today",
                direct_status("extraction-boundary-zero-lean-edges"),
            ),
            Citation(
                "tools/check_kernel_covenant.py KCOV-03/KCOV-04/KCOV-07",
                "no-async (kernel-is-synchronous), no-unsafe, no-plugins-or-dynamic-loading "
                "lint walls over the trusted checking base (bn-2he)",
                "pass" if all(rid_status(kcov, r) == "pass" for r in ("KCOV-03", "KCOV-04", "KCOV-07")) else "fail",
            ),
            Citation(
                "tools/check_crate_boundaries.py",
                "no forbidden dependency edge exists anywhere in the workspace graph "
                "(kernel-is-synchronous, certificate-checker-not-search included); no "
                "Lean-specific rule exists because no crate reaches Lean yet to forbid",
                str(boundaries.get("status", "unknown")),
            ),
        ],
        "compatibility CI": [
            Citation(
                "tools/governance/check_r16_evidence.py ci-runs-pinned-gate",
                ".github/workflows/check.yml (bn-28ur) runs `just check` under the pinned "
                "toolchain (`rustup show`) on every push to main and every PR against main",
                direct_status("ci-runs-pinned-gate"),
            ),
        ],
        "prefer stable metadata over rustc internals where possible": [
            Citation(
                "tools/governance/check_r16_evidence.py rustc-internal-usage",
                "the same zero count as 'narrow compiler-internal usage' — the preference is "
                "not merely honored, it is exceeded (zero, not minimized)",
                direct_status("rustc-internal-usage"),
            ),
        ],
    }

    return {
        control: {
            "citations": [c.__dict__ for c in citations],
            "status": "pass" if all(c.status == "pass" for c in citations) else "fail",
        }
        for control, citations in controls.items()
    }


# ============================================================================
# Evidence
# ============================================================================


def build_evidence(controls: dict[str, dict], direct_st: dict, delegate_st: dict) -> dict:
    overall = "pass" if all(c["status"] == "pass" for c in controls.values()) else "fail"
    return {
        "artifact": "continuum.governance.evidence/r16",
        "produced_by": "tools/governance/check_r16_evidence.py",
        "reproduce": f"python3 tools/governance/check_r16_evidence.py --evidence {EVIDENCE}",
        "authority": f"{RISK_REGISTER} R16 (Rust compiler churn)",
        "determinism": (
            "This file records no timestamp, commit id, or absolute path: it is a function of "
            "the repository contents and the delegate checkers' own outputs alone, so an "
            "unchanged tree reproduces it byte-for-byte. The only subprocesses run are the "
            "delegate checkers listed below, invoked exactly as Justfile invokes them; none "
            "makes a network access."
        ),
        "delegates": sorted(DELEGATES.values()),
        "controls": controls,
        "self_test": {
            "direct": direct_st,
            "delegates": delegate_st,
            "status": "pass" if direct_st.get("status") == "pass" and delegate_st.get("status") == "pass" else "fail",
        },
        "boundary": (
            "See the module docstring's 'Honesty about limits' section: extraction isolation "
            "is a zero-edge count, true today because the PR-28 proof-service client has not "
            "landed, not a positive boundary rule that would survive it landing; compatibility "
            "CI is read as 'the pinned build is validated pre-merge', not multi-toolchain "
            "compatibility testing, consistent with this workspace's one-pinned-toolchain "
            "(INV-014) posture."
        ),
        "status": overall,
    }


# ============================================================================
# Entry point
# ============================================================================


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="prove this file's four direct checks and every delegate's self-test all still catch their fixtures",
    )
    parser.add_argument(
        "--evidence",
        nargs="?",
        const=str(ROOT / EVIDENCE),
        default=None,
        metavar="PATH",
        help="run the self-test and the real binding, then write the evidence JSON to PATH",
    )
    parser.add_argument("--quiet", action="store_true", help="print only the status line")
    args = parser.parse_args(argv)

    tree = Tree(ROOT)

    if args.self_test and not args.evidence:
        direct_st = direct_self_test(tree)
        delegate_st = delegate_self_test()
        status = "pass" if direct_st["status"] == "pass" and delegate_st["status"] == "pass" else "fail"
        report = {"status": status, "direct": direct_st, "delegates": delegate_st}
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0 if status == "pass" else 1

    direct_st: dict[str, object] = {"status": "not-run"}
    delegate_st: dict[str, object] = {"status": "not-run"}
    st_ok = True
    if args.evidence:
        direct_st = direct_self_test(tree)
        delegate_st = delegate_self_test()
        st_ok = direct_st["status"] == "pass" and delegate_st["status"] == "pass"

    direct_raw = direct_checks(tree)
    direct = {k: [v.render() for v in vs] for k, vs in direct_raw.items()}
    reports = delegate_real_run()
    controls = build_controls(direct_raw, reports)

    if args.evidence:
        evidence = build_evidence(controls, direct_st, delegate_st)
        path = Path(args.evidence)
        if not path.is_absolute():
            path = ROOT / path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    failing_controls = {k: v for k, v in controls.items() if v["status"] != "pass"}
    report = {
        "controls": {k: v["status"] for k, v in sorted(controls.items())},
        "direct": direct,
        "failing_controls": sorted(failing_controls),
        "self_test": {"direct": direct_st.get("status"), "delegates": delegate_st.get("status")},
        "evidence": args.evidence,
        "status": "fail" if (failing_controls or not st_ok) else "pass",
    }
    if args.quiet:
        print(report["status"])
    else:
        print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if (failing_controls or not st_ok) else 0


if __name__ == "__main__":
    raise SystemExit(main())
