#!/usr/bin/env python3
"""Mechanically enforce vet/advisory-scanning posture for external dependencies.

Source of truth:

- `notes/plan/docs/09_THREAT_MODEL.md` T12 ("Dependency/unsafe compromise"),
  control 3, "cargo-vet/advisory scanning";
- `tools/governance/dependency-audits.toml` — the committed audit manifest this
  script checks `Cargo.lock` against. Its own module comment explains the
  criteria vocabulary and why it carries a `checksum` field upstream
  `cargo-vet` does not.

What this checks and why
-------------------------

`tools/governance/check_code_policy.py`'s GOV-1-07 already enforces that every
dependency — workspace-internal or external — has a rationale and a TCB class.
It does not, and was never meant to, answer a narrower question: has anyone
looked at the actual bytes of the one external dependency tree this workspace
pulls in (`blake3` and its transitive closure) for supply-chain risk, and does
that review still describe the *exact* version and checksum `Cargo.lock`
resolves today? Before this script, nothing in the workspace answered that
question mechanically — docs/09 T12 named "cargo-vet/advisory scanning" as a
required control and nothing implemented it. This is the honest minimal
version: a house-style, stdlib-only checker with no network access, checking a
human-authored, human-refreshed audit manifest against `Cargo.lock`.

One rule, `dependency-audit-posture`, with named sub-checks:

    manifest-present                     the audit manifest exists and parses
    manifest-well-formed                 `[audits.<pkg>]` is an array of
                                          tables, each entry names a version
    no-duplicate-entries                 no `(package, version)` pair is
                                          audited twice
    every-external-dependency-is-audited every externally-sourced `Cargo.lock`
                                          package has a matching entry —
                                          a brand-new or upgraded dependency
                                          has none, and fails until vetted
    no-stale-entries                     every audit entry names a package and
                                          version `Cargo.lock` still resolves
    who-is-stated                        the entry names who vetted it
    notes-are-stated                     the entry states what was reviewed
    criteria-is-recognized               the entry's criteria are drawn from
                                          the closed set {safe-to-deploy,
                                          safe-to-run}
    checksum-matches-lockfile            the entry's checksum equals the one
                                          `Cargo.lock` pins for that exact
                                          version — a same-version,
                                          different-bytes substitution is
                                          caught as loudly as a version bump

Scope and honesty about limits
-------------------------------

- Only *externally*-sourced `Cargo.lock` packages are in scope: a `[[package]]`
  entry with a `source` field. Workspace-internal crates (no `source`) are
  GOV-1-07's job, not this one's.
- This is a manifest check, not an advisory-database check. It cannot tell you
  a version is free of a *known* CVE; it can only tell you a human recorded a
  review of the exact bytes `Cargo.lock` resolves, and that no dependency
  changed those bytes since. A gate that queried a live advisory feed would be
  nondeterministic (network-dependent, time-dependent) and is deliberately not
  what this does — see README-gov1.md's determinism rule and this script's own
  `--self-test`, which never touches the network. Refreshing the manifest
  against whatever advisory source a maintainer trusts is a human action.
- No delta audits. Every entry is a full audit of the exact version it names.
  Stricter than upstream `cargo-vet`, not weaker: a version bump always
  requires a fresh entry rather than an unreviewed diff.

Self-test
---------

The workspace's own `Cargo.lock`/`dependency-audits.toml` deliberately satisfy
the rule, so a naive run passes vacuously. `--self-test` replays every
violating fixture in `tools/governance/fixtures/dependency-audit/` through the
same rule function and fails if any fixture goes uncaught. Fixtures are inert
data: full-file overlays of `Cargo.lock` or `dependency-audits.toml`, applied
to an in-memory view and thrown away — nothing under `tools/` is ever resolved
by `cargo` or read by the dossier validator.

Stdlib only. Exit 0 when the rule holds, 1 otherwise.

Usage:

    python3 tools/governance/check_dependency_audit.py --self-test
    python3 tools/governance/check_dependency_audit.py
    python3 tools/governance/check_dependency_audit.py --evidence tools/governance/evidence/dependency-audit.json
"""

from __future__ import annotations

import argparse
import json
import re
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GOVERNANCE = "tools/governance"
CARGO_LOCK = "Cargo.lock"
AUDITS_PATH = f"{GOVERNANCE}/dependency-audits.toml"
FIXTURE_DIR = f"{GOVERNANCE}/fixtures/dependency-audit"
EVIDENCE = f"{GOVERNANCE}/evidence/dependency-audit.json"

THREAT_MODEL = "notes/plan/docs/09_THREAT_MODEL.md"
RULE = "dependency-audit-posture"

CRITERIA_VOCAB = ("safe-to-deploy", "safe-to-run")
MIN_WHO = 6
MIN_NOTES = 24
CHECKSUM_RE = re.compile(r"^[0-9a-f]{64}$")


# ============================================================================
# Repository view (fixture overlay, mirroring tools/governance/check_code_policy.py's Tree)
# ============================================================================


@dataclass
class Tree:
    """A read-only view of two files, optionally with fixture overlays.

    The rule reads the repository *only* through this class, so the exact code
    path that runs against the real tree runs against a fixture tree too.
    """

    root: Path
    overlay: dict[str, str] = field(default_factory=dict)
    removed: frozenset[str] = frozenset()

    def with_changes(self, overlay: dict[str, str], removed: tuple[str, ...] = ()) -> "Tree":
        merged = {**self.overlay, **overlay}
        return Tree(self.root, merged, frozenset(self.removed) | frozenset(removed))

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


# ============================================================================
# Violations
# ============================================================================


@dataclass(frozen=True, order=True)
class Violation:
    check: str
    message: str

    def render(self) -> str:
        return f"[{RULE}/{self.check}] {self.message}"


# ============================================================================
# The rule
# ============================================================================


@dataclass(frozen=True)
class LockedPackage:
    name: str
    version: str
    checksum: str | None


def externally_sourced_packages(lock_doc: dict) -> dict[tuple[str, str], LockedPackage]:
    """Every `Cargo.lock` package with a `source` — i.e., not a workspace member.

    A workspace-internal crate (a `crates/*` path member) has no `source` key
    at all; anything with one was fetched from a registry (or, in principle, a
    git remote) and is in scope for this check. Keyed by (name, version) since
    Cargo.lock can in principle resolve two incompatible major versions of one
    package at once.
    """
    packages: dict[tuple[str, str], LockedPackage] = {}
    for entry in lock_doc.get("package", []):
        name = entry.get("name")
        version = entry.get("version")
        source = entry.get("source")
        if not isinstance(name, str) or not isinstance(version, str) or not source:
            continue
        packages[(name, version)] = LockedPackage(name, version, entry.get("checksum"))
    return packages


def rule_dependency_audit(tree: Tree) -> list[Violation]:
    out: list[Violation] = []

    lock_doc = tree.read_toml(CARGO_LOCK)
    if lock_doc is None:
        out.append(
            Violation(
                "cargo-lock-present",
                f"{CARGO_LOCK} is missing or is not valid TOML; there is nothing to audit against",
            )
        )
        return out
    locked = externally_sourced_packages(lock_doc)

    manifest = tree.read_toml(AUDITS_PATH)
    if manifest is None:
        out.append(
            Violation(
                "manifest-present",
                f"{AUDITS_PATH} is missing or is not valid TOML; every externally-sourced dependency "
                f"needs a vetted audit entry before it is trusted ({THREAT_MODEL} T12)",
            )
        )
        return out

    audits_table = manifest.get("audits")
    if not isinstance(audits_table, dict):
        out.append(Violation("manifest-well-formed", f"{AUDITS_PATH} declares no `[audits]` table"))
        return out

    entries: dict[tuple[str, str], dict] = {}
    for pkg_name, records in sorted(audits_table.items()):
        if not isinstance(records, list):
            out.append(
                Violation(
                    "manifest-well-formed",
                    f"{AUDITS_PATH} `audits.{pkg_name}` is not an array of tables "
                    "(expected `[[audits.{pkg_name}]]`)",
                )
            )
            continue
        for record in records:
            if not isinstance(record, dict):
                out.append(
                    Violation(
                        "manifest-well-formed",
                        f"{AUDITS_PATH} `audits.{pkg_name}` has a non-table entry",
                    )
                )
                continue
            version = record.get("version")
            if not isinstance(version, str) or not version:
                out.append(
                    Violation(
                        "manifest-well-formed",
                        f"{AUDITS_PATH} `audits.{pkg_name}` has an entry with no `version`",
                    )
                )
                continue
            key = (pkg_name, version)
            if key in entries:
                out.append(
                    Violation(
                        "no-duplicate-entries",
                        f"{AUDITS_PATH} `audits.{pkg_name}` declares version {version!r} twice",
                    )
                )
                continue
            entries[key] = record

    for name, version in sorted(set(locked) - set(entries)):
        out.append(
            Violation(
                "every-external-dependency-is-audited",
                f"`{name}` {version} is resolved in {CARGO_LOCK} from an external source with no "
                f"matching `audits.{name}` entry for version {version!r} in {AUDITS_PATH}; a new or "
                f"upgraded dependency is untrusted until it is vetted ({THREAT_MODEL} T12)",
            )
        )

    for name, version in sorted(set(entries) - set(locked)):
        out.append(
            Violation(
                "no-stale-entries",
                f"{AUDITS_PATH} `audits.{name}` records version {version!r}, which {CARGO_LOCK} no "
                "longer resolves from an external source",
            )
        )

    for name, version in sorted(set(entries) & set(locked)):
        record = entries[(name, version)]
        pkg = locked[(name, version)]

        who = record.get("who")
        if not isinstance(who, str) or len(who.strip()) < MIN_WHO:
            out.append(
                Violation(
                    "who-is-stated",
                    f"{AUDITS_PATH} `audits.{name}` version {version!r} names no usable reviewer "
                    f"(needs at least {MIN_WHO} characters)",
                )
            )

        notes = record.get("notes")
        if not isinstance(notes, str) or len(notes.strip()) < MIN_NOTES:
            out.append(
                Violation(
                    "notes-are-stated",
                    f"{AUDITS_PATH} `audits.{name}` version {version!r} has no usable review notes "
                    f"(needs at least {MIN_NOTES} characters saying what was checked)",
                )
            )

        raw_criteria = record.get("criteria")
        if isinstance(raw_criteria, str):
            criteria = [raw_criteria]
        elif isinstance(raw_criteria, list):
            criteria = raw_criteria
        else:
            criteria = []
        if not criteria or any(c not in CRITERIA_VOCAB for c in criteria):
            out.append(
                Violation(
                    "criteria-is-recognized",
                    f"{AUDITS_PATH} `audits.{name}` version {version!r} has criteria {raw_criteria!r}; "
                    f"expected a non-empty subset of {list(CRITERIA_VOCAB)}",
                )
            )

        checksum = record.get("checksum")
        if not isinstance(checksum, str) or not CHECKSUM_RE.match(checksum):
            out.append(
                Violation(
                    "checksum-matches-lockfile",
                    f"{AUDITS_PATH} `audits.{name}` version {version!r} has no well-formed 64-hex-digit "
                    "checksum to bind the audit to the exact bytes Cargo resolves",
                )
            )
        elif pkg.checksum and checksum != pkg.checksum:
            out.append(
                Violation(
                    "checksum-matches-lockfile",
                    f"{AUDITS_PATH} `audits.{name}` version {version!r} records checksum {checksum}, "
                    f"but {CARGO_LOCK} resolves that version to {pkg.checksum} — the audited bytes are "
                    f"not the bytes cargo will fetch ({THREAT_MODEL} T12)",
                )
            )

    return sorted(out)


# ============================================================================
# Fixtures
# ============================================================================


@dataclass(frozen=True)
class Fixture:
    fid: str
    check: str
    description: str
    overlay: dict[str, str]
    removed: tuple[str, ...]


def load_fixtures() -> tuple[list[Fixture], list[str]]:
    """Read every fixture description. Fixtures are inert data, never resolved."""
    fixtures: list[Fixture] = []
    problems: list[str] = []
    base = ROOT / FIXTURE_DIR
    if not base.is_dir():
        return fixtures, [f"no fixtures under {FIXTURE_DIR}"]

    for directory in sorted(base.iterdir()):
        if not directory.is_dir():
            continue
        spec_path = directory / "fixture.json"
        if not spec_path.is_file():
            problems.append(f"{directory}: no fixture.json")
            continue
        try:
            spec = json.loads(spec_path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            problems.append(f"{spec_path}: invalid JSON ({exc})")
            continue

        overlay: dict[str, str] = {}
        ok = True
        for target, payload in sorted((spec.get("overlay") or {}).items()):
            payload_path = directory / payload
            if not payload_path.is_file():
                problems.append(f"{spec_path}: overlay payload {payload!r} is missing")
                ok = False
                continue
            overlay[target] = payload_path.read_text(encoding="utf-8")

        removed = tuple(spec.get("remove") or ())
        for target in removed:
            # The real file must exist for "remove" to mean anything.
            if target not in overlay and not (ROOT / target).is_file():
                problems.append(f"{spec_path}: remove target {target!r} does not exist")
                ok = False

        if not ok:
            continue
        fixtures.append(
            Fixture(
                fid=spec.get("id", directory.name),
                check=spec["check"],
                description=spec["description"],
                overlay=overlay,
                removed=removed,
            )
        )
    return fixtures, problems


def self_test(tree: Tree) -> tuple[bool, dict[str, object]]:
    """Replay every fixture through the real rule; fail if any goes uncaught."""
    fixtures, problems = load_fixtures()
    failures: list[str] = list(problems)

    baseline = rule_dependency_audit(tree)
    if baseline:
        failures.append(
            "the real repository already violates the rule; fixtures prove nothing on it: "
            + "; ".join(v.render() for v in baseline)
        )

    covered: dict[str, list[str]] = {}
    for fixture in sorted(fixtures, key=lambda f: f.fid):
        found = rule_dependency_audit(tree.with_changes(fixture.overlay, fixture.removed))
        if not any(v.check == fixture.check for v in found):
            failures.append(
                f"{fixture.fid}: expected check {fixture.check!r} was not triggered "
                f"(fired: {sorted({v.check for v in found}) or 'nothing'})"
            )
            continue
        covered.setdefault(fixture.check, []).append(fixture.fid)

    all_checks = (
        "manifest-present",
        "manifest-well-formed",
        "no-duplicate-entries",
        "every-external-dependency-is-audited",
        "no-stale-entries",
        "who-is-stated",
        "notes-are-stated",
        "criteria-is-recognized",
        "checksum-matches-lockfile",
    )
    for check in all_checks:
        if not covered.get(check):
            failures.append(f"check {check!r} has no violating fixture; it could pass vacuously")

    report = {
        "status": "fail" if failures else "pass",
        "fixtures": len(fixtures),
        "checks_covered": sorted(covered),
        "fixtures_by_check": {k: sorted(v) for k, v in sorted(covered.items())},
        "failures": sorted(failures),
    }
    return (not failures), report


# ============================================================================
# Evidence
# ============================================================================


def build_evidence(tree: Tree, violations: list[Violation], st: dict[str, object]) -> dict:
    """A deterministic function of the repository — no clock, no commit id.

    Re-running the check on an unchanged tree rewrites this file byte-for-byte,
    so a diff in `evidence/dependency-audit.json` always means the audit
    posture moved.
    """
    lock_doc = tree.read_toml(CARGO_LOCK) or {}
    locked = externally_sourced_packages(lock_doc)
    return {
        "artifact": "continuum.governance.evidence/dependency-audit",
        "produced_by": f"{GOVERNANCE}/check_dependency_audit.py",
        "reproduce": f"python3 {GOVERNANCE}/check_dependency_audit.py --evidence {EVIDENCE}",
        "authority": {
            "threat_model": f"{THREAT_MODEL} T12, control 3 (cargo-vet/advisory scanning)",
            "manifest": AUDITS_PATH,
        },
        "determinism": (
            "This file records no timestamp, commit id, or absolute path: it is a function of the "
            "repository contents alone, so an unchanged tree reproduces it byte-for-byte. It makes no "
            "network access."
        ),
        "scope": {
            "in_scope": "Cargo.lock packages carrying a `source` (externally fetched)",
            "out_of_scope": "workspace-internal crates (no `source`); GOV-1-07 covers those",
            "externally_sourced_packages": sorted(f"{n} {v}" for n, v in locked),
        },
        "rule": RULE,
        "checks": [
            "manifest-present",
            "manifest-well-formed",
            "no-duplicate-entries",
            "every-external-dependency-is-audited",
            "no-stale-entries",
            "who-is-stated",
            "notes-are-stated",
            "criteria-is-recognized",
            "checksum-matches-lockfile",
        ],
        "real_run": {
            "status": "fail" if violations else "pass",
            "violations": [v.render() for v in violations],
        },
        "self_test": st,
        "boundary": (
            "Manifest-vs-lockfile only. This does not query a live advisory database — doing so from a "
            "gate would be nondeterministic (network- and time-dependent), so it is out of scope by "
            "design; see the module docstring. It proves a human recorded a review of the exact version "
            "and checksum Cargo.lock resolves today, and that nothing changed under it unnoticed — not "
            "that the reviewed crate is free of a since-published CVE."
        ),
        "status": "fail" if (violations or st.get("status") != "pass") else "pass",
    }


# ============================================================================
# Entry point
# ============================================================================


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="replay every violating fixture through the rule and fail if any goes uncaught",
    )
    parser.add_argument(
        "--evidence",
        nargs="?",
        const=str(ROOT / EVIDENCE),
        default=None,
        metavar="PATH",
        help="run the self-test and the real check, then write the evidence JSON to PATH",
    )
    parser.add_argument("--quiet", action="store_true", help="print only the status line")
    parser.add_argument("--root", default=str(ROOT), help="repository root to check")
    args = parser.parse_args(argv)

    root = Path(args.root).resolve()
    tree = Tree(root)

    if args.self_test and not args.evidence:
        ok, report = self_test(tree)
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0 if ok else 1

    st: dict[str, object] = {"status": "not-run"}
    st_ok = True
    if args.evidence:
        st_ok, st = self_test(tree)

    violations = rule_dependency_audit(tree)

    if args.evidence:
        evidence = build_evidence(tree, violations, st)
        path = Path(args.evidence)
        if not path.is_absolute():
            path = root / path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    report = {
        "rule": RULE,
        "checks": 9,
        "violations": [v.render() for v in violations],
        "self_test": st.get("status"),
        "evidence": args.evidence,
        "status": "fail" if (violations or not st_ok) else "pass",
    }
    if args.quiet:
        print(report["status"])
    else:
        print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if (violations or not st_ok) else 0


if __name__ == "__main__":
    raise SystemExit(main())
