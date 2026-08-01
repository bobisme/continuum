#!/usr/bin/env python3
"""Mechanically enforce Continuum's GOV §2 ADR decision-process obligations.

Source of truth:

- `notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md` §2 "Decision process".
- `notes/plan/notes/PLAN_REQUIREMENTS.json`, ids `GOV-2-01` .. `GOV-2-14`.

§2 states two rules, split here into fourteen traceable requirement ids:

1. "ADRs have statuses: proposed; accepted; superseded; rejected;
   experimental." -> a CLOSED five-value set, nothing else admitted.
   `GOV-2-01`..`GOV-2-05`, one id per canonical value.
2. "An ADR must include: context; decision; formal consequences;
   alternatives; compatibility; security; performance hypothesis;
   validation plan; rollback." -> nine required sections.
   `GOV-2-06`..`GOV-2-14`, one id per section, in that order.

Both rules are enforced by the same scan of `notes/plan/adr/*.md`, so this one
script covers all fourteen ids even though `GOV-2-07`..`GOV-2-12` are also
tracked under a sibling Bone's requirement range.

Status extraction accepts either house style found in the real corpus:

- inline bold: a line matching `**Status:** <value>` (ADR-0001..0030, 0053);
- heading style: a `## Status` heading whose first non-blank line is the
  value (ADR-0031..0052).

The extracted value is normalized to its leading alphabetic word, lowercased
(e.g. "Accepted as target architecture" -> "accepted", "Accepted in
principle; deferred" -> "accepted"). docs/12 §2 lists the five status *words*;
it does not forbid an ADR from qualifying its status with explanatory prose
after the word, and every real ADR as scanned already does exactly that or
nothing at all. Membership in the closed set is checked against the
normalized leading word. An ADR with no status field at all is a separate,
always-fatal violation (there is nothing to check against the set).

Section presence is checked against `##`-level headings (a `###` subsection
never counts, so a subsection nested inside e.g. "## Decision" cannot
masquerade as its own top-level section) with a tolerant-but-honest match per
section, chosen by reading the real corpus rather than guessed:

- context            -> heading == "context"
- decision           -> heading == "decision"
- formal_consequences -> heading == "consequences" or "formal consequences"
                         (the house style always writes plain "Consequences";
                         no real ADR spells out "Formal consequences")
- alternatives       -> heading contains the word "alternative(s)"
                         (covers "Alternatives considered" and "Rejected
                         alternatives", both used in the real corpus)
- compatibility      -> heading == "compatibility"
- security           -> heading == "security"
- performance_hypothesis -> heading == "performance" or "performance
                         hypothesis"
- validation_plan    -> heading == "validation", "validation plan", or
                         "validation and rollback" (the house style's combined
                         heading, used through ADR-0029/0053, satisfies both
                         validation_plan and rollback at once)
- rollback           -> heading == "rollback" or "validation and rollback"

Honesty about the real corpus: as scanned, 52 of the 53 real ADRs predate the
nine-section discipline and are missing one or more required sections (only
ADR-0053 is fully compliant). `adr-grandfathered.toml` in this directory
names, per ADR, per missing section, exactly which gaps are exempted. The
check still fails a grandfathered ADR for anything not named in its entry,
fails outright on a stale entry (naming a section that is not actually
missing) or an entry for a file that does not exist, and grants no exemption
whatsoever to a file not listed there — a new ADR must be fully compliant
from the day it lands. No status-value exemptions exist; as scanned, every
real ADR's status already normalizes into the closed set.

Stdlib only (Python 3.11+ for `tomllib`). Exit 0 when the real scan and
`--self-test` both hold, 1 otherwise.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import tomllib
from dataclasses import dataclass, field

HERE = pathlib.Path(__file__).resolve().parent  # tools/governance
ROOT = HERE.parents[1]  # repository root
ADR_DIR = ROOT / "notes" / "plan" / "adr"
GRANDFATHER_FILE = HERE / "adr-grandfathered.toml"
FIXTURES_DIR = HERE / "fixtures" / "adr"
DEFAULT_EVIDENCE_FILE = HERE / "evidence" / "gov-2.json"

# --- the closed status set (GOV-2-01..05) --------------------------------------

STATUS_IDS: tuple[tuple[str, str], ...] = (
    ("GOV-2-01", "proposed"),
    ("GOV-2-02", "accepted"),
    ("GOV-2-03", "superseded"),
    ("GOV-2-04", "rejected"),
    ("GOV-2-05", "experimental"),
)
CLOSED_STATUS_SET = frozenset(value for _gid, value in STATUS_IDS)

# --- the nine required sections (GOV-2-06..14) ----------------------------------

# (requirement id, canonical key, heading-match regex, human rule description)
SECTION_IDS: tuple[tuple[str, str, str, str], ...] = (
    ("GOV-2-06", "context", r"^context$", 'heading == "Context"'),
    ("GOV-2-07", "decision", r"^decision$", 'heading == "Decision"'),
    (
        "GOV-2-08",
        "formal_consequences",
        r"^(formal\s+)?consequences$",
        'heading == "Consequences" or "Formal consequences"',
    ),
    (
        "GOV-2-09",
        "alternatives",
        r"\balternatives?\b",
        'heading contains the word "alternative(s)"',
    ),
    ("GOV-2-10", "compatibility", r"^compatibility$", 'heading == "Compatibility"'),
    ("GOV-2-11", "security", r"^security$", 'heading == "Security"'),
    (
        "GOV-2-12",
        "performance_hypothesis",
        r"^performance(\s+hypothesis)?$",
        'heading == "Performance" or "Performance hypothesis"',
    ),
    (
        "GOV-2-13",
        "validation_plan",
        r"^(validation(\s+plan)?|validation\s+and\s+rollback)$",
        'heading == "Validation", "Validation plan", or "Validation and rollback"',
    ),
    (
        "GOV-2-14",
        "rollback",
        r"^(rollback|validation\s+and\s+rollback)$",
        'heading == "Rollback" or "Validation and rollback"',
    ),
)
SECTION_KEYS: tuple[str, ...] = tuple(key for _gid, key, _pat, _desc in SECTION_IDS)

HEADING_RE = re.compile(r"^##\s+(.+?)\s*$", re.MULTILINE)
INLINE_STATUS_RE = re.compile(r"^\*\*Status:\*\*\s*(.+?)\s*$", re.MULTILINE)
HEADING_STATUS_RE = re.compile(
    r"^##\s+Status\s*\n+(.*?)(?:\n\s*\n|\n##\s|\Z)", re.MULTILINE | re.DOTALL
)
LEADING_WORD_RE = re.compile(r"[A-Za-z]+")


class Violation(str):
    """A single rule failure, rendered as a human-readable line."""


@dataclass
class AdrDoc:
    name: str
    raw_status: str | None
    normalized_status: str | None
    section_present: dict[str, bool] = field(default_factory=dict)


def extract_status(text: str) -> str | None:
    m = INLINE_STATUS_RE.search(text)
    if m:
        return m.group(1).strip()
    m = HEADING_STATUS_RE.search(text)
    if m:
        block = m.group(1).strip()
        if block:
            first_line = block.splitlines()[0].strip()
            return first_line or None
    return None


def normalize_status(raw: str | None) -> str | None:
    if not raw:
        return None
    m = LEADING_WORD_RE.match(raw)
    if not m:
        return None
    return m.group(0).lower()


def parse_adr(name: str, text: str) -> AdrDoc:
    headings = [h.strip() for h in HEADING_RE.findall(text)]
    headings = [h for h in headings if h.lower() != "status"]
    present: dict[str, bool] = {}
    for _gid, key, pattern, _desc in SECTION_IDS:
        rx = re.compile(pattern, re.IGNORECASE)
        present[key] = any(rx.search(h) for h in headings)
    raw_status = extract_status(text)
    return AdrDoc(
        name=name,
        raw_status=raw_status,
        normalized_status=normalize_status(raw_status),
        section_present=present,
    )


def evaluate_adr(doc: AdrDoc) -> list[Violation]:
    """Every rule violation for one ADR, with no grandfathering applied."""
    violations: list[Violation] = []
    if doc.normalized_status is None:
        violations.append(
            Violation(
                f"[status-required] {doc.name} -- no ADR status found (docs/12 §2 "
                "requires one of the five closed statuses)"
            )
        )
    elif doc.normalized_status not in CLOSED_STATUS_SET:
        violations.append(
            Violation(
                f"[status-closed-set] {doc.name} -- status {doc.raw_status!r} is not "
                f"a member of the closed set {sorted(CLOSED_STATUS_SET)} (docs/12 §2)"
            )
        )
    for gid, key, _pattern, _desc in SECTION_IDS:
        if not doc.section_present.get(key):
            violations.append(
                Violation(
                    f"[section-missing:{key}] {doc.name} -- missing required section "
                    f"'{key}' ({gid}; docs/12 §2)"
                )
            )
    return violations


# --- grandfather allowlist -------------------------------------------------------


def load_grandfathered(path: pathlib.Path) -> dict[str, set[str]]:
    if not path.exists():
        return {}
    data = tomllib.loads(path.read_text(encoding="utf-8"))
    result: dict[str, set[str]] = {}
    for entry in data.get("grandfathered", []):
        adr = entry["adr"]
        sections = set(entry.get("missing_sections", []))
        unknown = sections - set(SECTION_KEYS)
        if unknown:
            raise SystemExit(
                f"adr-grandfathered.toml: entry for {adr!r} names unknown section "
                f"key(s) {sorted(unknown)}; canonical keys are {list(SECTION_KEYS)}"
            )
        result[adr] = sections
    return result


def load_real_adrs() -> dict[str, AdrDoc]:
    docs: dict[str, AdrDoc] = {}
    for f in sorted(ADR_DIR.glob("*.md")):
        if f.name == "README.md":
            continue
        docs[f.name] = parse_adr(f.name, f.read_text(encoding="utf-8"))
    return docs


@dataclass
class ScanResult:
    adr_count: int
    violations: list[Violation]
    grandfathered_used: list[tuple[str, str]]  # (adr, section) actually exempted
    status_counts: dict[str, int]  # normalized status -> count, real corpus only
    section_missing_counts: dict[str, int]  # key -> count missing, real corpus
    section_grandfathered_counts: dict[str, int]  # key -> count exempted


def scan_real_corpus(grandfather: dict[str, set[str]]) -> ScanResult:
    docs = load_real_adrs()
    violations: list[Violation] = []
    grandfathered_used: list[tuple[str, str]] = []
    status_counts: dict[str, int] = {}
    section_missing_counts: dict[str, int] = {key: 0 for key in SECTION_KEYS}
    section_grandfathered_counts: dict[str, int] = {key: 0 for key in SECTION_KEYS}

    for name, doc in docs.items():
        if doc.normalized_status:
            status_counts[doc.normalized_status] = (
                status_counts.get(doc.normalized_status, 0) + 1
            )
        missing_keys = {key for key in SECTION_KEYS if not doc.section_present[key]}
        for key in missing_keys:
            section_missing_counts[key] += 1

        exempt = grandfather.get(name, set())
        # A stale exemption -- naming a section that is not actually missing --
        # is itself a failure, so the allowlist cannot drift from the truth.
        stale = exempt - missing_keys
        if stale:
            violations.append(
                Violation(
                    f"[grandfather-stale] {name} -- adr-grandfathered.toml exempts "
                    f"{sorted(stale)} but the section(s) are present; remove the "
                    "stale exemption"
                )
            )

        for v in evaluate_adr(doc):
            matched_key = None
            for key in SECTION_KEYS:
                if v.startswith(f"[section-missing:{key}]"):
                    matched_key = key
                    break
            if matched_key is not None and matched_key in exempt:
                grandfathered_used.append((name, matched_key))
                section_grandfathered_counts[matched_key] += 1
                continue
            violations.append(v)

    known_files = set(docs.keys())
    for adr_name in grandfather:
        if adr_name not in known_files:
            violations.append(
                Violation(
                    f"[grandfather-unknown-adr] adr-grandfathered.toml references "
                    f"a file that does not exist under notes/plan/adr/: {adr_name}"
                )
            )

    return ScanResult(
        adr_count=len(docs),
        violations=violations,
        grandfathered_used=grandfathered_used,
        status_counts=status_counts,
        section_missing_counts=section_missing_counts,
        section_grandfathered_counts=section_grandfathered_counts,
    )


# --- self-test -------------------------------------------------------------------
#
# Every fixture under fixtures/adr/ is a near-complete ADR (valid status, all
# nine sections) with exactly one deliberate defect. The self-test parses each
# fixture directly -- no grandfathering, fixtures are not real ADRs -- and
# asserts the expected violation tag appears. It also asserts a clean,
# fully-compliant ADR produces zero violations, so the rules cannot be passing
# vacuously.

CLEAN_FIXTURE_TEXT = """# ADR-9000: Fixture -- fully compliant baseline

**Status:** Accepted

## Context

Fixture context paragraph.

## Decision

Fixture decision paragraph.

## Consequences

Fixture consequences paragraph.

## Alternatives considered

Fixture alternatives paragraph.

## Compatibility

Fixture compatibility paragraph.

## Security

Fixture security paragraph.

## Performance hypothesis

Fixture performance-hypothesis paragraph.

## Validation plan

Fixture validation-plan paragraph.

## Rollback

Fixture rollback paragraph.
"""

# fixture filename -> expected violation tag(s); every listed tag must appear
# among the fixture's violations, caught with no grandfathering.
FIXTURE_EXPECTATIONS: dict[str, tuple[str, ...]] = {
    "status-out-of-set.md": ("status-closed-set",),
    "status-missing.md": ("status-required",),
    "missing-context.md": ("section-missing:context",),
    "missing-decision.md": ("section-missing:decision",),
    "missing-formal-consequences.md": ("section-missing:formal_consequences",),
    "missing-alternatives.md": ("section-missing:alternatives",),
    "missing-compatibility.md": ("section-missing:compatibility",),
    "missing-security.md": ("section-missing:security",),
    "missing-performance-hypothesis.md": ("section-missing:performance_hypothesis",),
    "missing-validation-plan.md": ("section-missing:validation_plan",),
    "missing-rollback.md": ("section-missing:rollback",),
}


def self_test() -> tuple[int, dict]:
    failures: list[str] = []

    clean_doc = parse_adr("clean-fixture.md", CLEAN_FIXTURE_TEXT)
    clean_violations = evaluate_adr(clean_doc)
    if clean_violations:
        failures.append(
            f"clean fixture reported violations (vacuous rule?): {clean_violations}"
        )

    on_disk = (
        {p.name for p in FIXTURES_DIR.glob("*.md")} if FIXTURES_DIR.exists() else set()
    )
    expected_names = set(FIXTURE_EXPECTATIONS)
    missing_on_disk = sorted(expected_names - on_disk)
    unexpected_on_disk = sorted(on_disk - expected_names)
    for name in missing_on_disk:
        failures.append(f"expected fixture file is missing from disk: {name}")
    for name in unexpected_on_disk:
        failures.append(
            f"fixture file on disk has no expectation entry (update "
            f"FIXTURE_EXPECTATIONS or remove the file): {name}"
        )

    fixture_results: dict[str, dict] = {}
    for name, expected_tags in sorted(FIXTURE_EXPECTATIONS.items()):
        path = FIXTURES_DIR / name
        if not path.exists():
            fixture_results[name] = {"caught": False, "reason": "file missing"}
            continue
        doc = parse_adr(name, path.read_text(encoding="utf-8"))
        violations = evaluate_adr(doc)
        tags_present = []
        for v in violations:
            m = re.match(r"^\[([^\]]+)\]", v)
            if m:
                tags_present.append(m.group(1))
        caught = all(tag in tags_present for tag in expected_tags)
        fixture_results[name] = {
            "caught": caught,
            "expected_tags": list(expected_tags),
            "tags_found": tags_present,
        }
        if not caught:
            failures.append(
                f"{name}: expected tag(s) {expected_tags} not all found in "
                f"{tags_present}"
            )

    report = {
        "self_test": "fail" if failures else "pass",
        "clean_fixture_ok": not clean_violations,
        "fixture_cases": len(FIXTURE_EXPECTATIONS),
        "fixture_results": fixture_results,
        "failures": failures,
    }
    return (1 if failures else 0, report)


# --- evidence ----------------------------------------------------------------


def build_evidence(scan: ScanResult, self_test_report: dict) -> dict:
    grandfather_by_adr: dict[str, list[str]] = {}
    for adr, key in scan.grandfathered_used:
        grandfather_by_adr.setdefault(adr, []).append(key)
    for adr in grandfather_by_adr:
        grandfather_by_adr[adr].sort()

    real_scan_status = "pass" if not scan.violations else "fail"
    overall_status = (
        "pass"
        if real_scan_status == "pass" and self_test_report["self_test"] == "pass"
        else "fail"
    )

    scan_scope = f"{scan.adr_count} real ADRs under notes/plan/adr/*.md (README.md excluded)"

    requirements: dict[str, dict] = {}

    # GOV-2-01..05: the closed status set.
    status_fixtures = ["status-out-of-set.md", "status-missing.md"]
    status_result = "pass" if real_scan_status == "pass" else "fail"
    for gid, value in STATUS_IDS:
        requirements[gid] = {
            "obligation": (
                f'ADR status value "{value}" is a member of the closed five-value '
                "set {proposed, accepted, superseded, rejected, experimental}; "
                "no other status value is admitted (docs/12 §2)"
            ),
            "rule": (
                "status extracted from an inline '**Status:** X' line or a "
                "'## Status' heading block, normalized to its leading alphabetic "
                "word (lowercased); the normalized word must be in the closed set, "
                "and a status must be present at all"
            ),
            "scan_scope": scan_scope,
            "real_count_using_value": scan.status_counts.get(value, 0),
            "result": status_result,
            "fixtures_caught": status_fixtures,
            "grandfathered": [],
        }

    # GOV-2-06..14: the nine required sections.
    for gid, key, _pattern, desc in SECTION_IDS:
        ungrandfathered_failures = [
            str(v)
            for v in scan.violations
            if v.startswith(f"[section-missing:{key}]")
        ]
        fixture_name = {
            "context": "missing-context.md",
            "decision": "missing-decision.md",
            "formal_consequences": "missing-formal-consequences.md",
            "alternatives": "missing-alternatives.md",
            "compatibility": "missing-compatibility.md",
            "security": "missing-security.md",
            "performance_hypothesis": "missing-performance-hypothesis.md",
            "validation_plan": "missing-validation-plan.md",
            "rollback": "missing-rollback.md",
        }[key]
        requirements[gid] = {
            "obligation": f'ADR includes the "{key}" section (docs/12 §2)',
            "rule": desc,
            "scan_scope": scan_scope,
            "real_missing_count": scan.section_missing_counts[key],
            "real_grandfathered_count": scan.section_grandfathered_counts[key],
            "real_ungrandfathered_failures": len(ungrandfathered_failures),
            "result": "pass" if not ungrandfathered_failures else "fail",
            "fixtures_caught": [fixture_name],
            "grandfathered": sorted(
                adr for adr, keys in grandfather_by_adr.items() if key in keys
            ),
        }

    return {
        "script": "tools/governance/check_adr_process.py",
        "authoritative_text": [
            "notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md §2",
            "notes/plan/notes/PLAN_REQUIREMENTS.json (GOV-2-01..GOV-2-14)",
        ],
        "grandfather_file": "tools/governance/adr-grandfathered.toml",
        "grandfathered_adr_count": len(grandfather_by_adr),
        "grandfathered_adr_names": sorted(grandfather_by_adr),
        "self_test": self_test_report,
        "real_scan": {
            "status": real_scan_status,
            "adr_count": scan.adr_count,
            "violation_count": len(scan.violations),
            "violations": [str(v) for v in scan.violations],
        },
        "status": overall_status,
        "requirements": requirements,
    }


# --- CLI ---------------------------------------------------------------------


def run_self_test_cli() -> int:
    code, report = self_test()
    print(json.dumps(report, indent=2, sort_keys=True))
    return code


def run_real_scan_cli() -> int:
    grandfather = load_grandfathered(GRANDFATHER_FILE)
    scan = scan_real_corpus(grandfather)
    grandfather_by_adr: dict[str, list[str]] = {}
    for adr, key in scan.grandfathered_used:
        grandfather_by_adr.setdefault(adr, []).append(key)
    report = {
        "adr_count": scan.adr_count,
        "status_counts": scan.status_counts,
        "section_missing_counts": scan.section_missing_counts,
        "grandfathered_adr_count": len(grandfather_by_adr),
        "grandfathered_exemptions_used": sum(
            len(v) for v in grandfather_by_adr.values()
        ),
        "violations": [str(v) for v in scan.violations],
        "status": "pass" if not scan.violations else "fail",
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if scan.violations else 0


def run_evidence_cli(out_path: pathlib.Path) -> int:
    grandfather = load_grandfathered(GRANDFATHER_FILE)
    scan = scan_real_corpus(grandfather)
    self_test_code, self_test_report = self_test()
    evidence = build_evidence(scan, self_test_report)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(evidence, indent=2, sort_keys=True))
    real_scan_failed = evidence["real_scan"]["status"] != "pass"
    return 1 if (self_test_code or real_scan_failed) else 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="run every violating fixture and fail if any goes uncaught",
    )
    parser.add_argument(
        "--evidence",
        nargs="?",
        const=str(DEFAULT_EVIDENCE_FILE),
        default=None,
        metavar="PATH",
        help=(
            "run self-test and the real scan, then write evidence JSON keyed by "
            f"GOV-2-01..14 to PATH (default: {DEFAULT_EVIDENCE_FILE})"
        ),
    )
    args = parser.parse_args(argv)

    if args.evidence is not None:
        return run_evidence_cli(pathlib.Path(args.evidence))
    if args.self_test:
        return run_self_test_cli()
    return run_real_scan_cli()


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
