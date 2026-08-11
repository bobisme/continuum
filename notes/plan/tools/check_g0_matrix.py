#!/usr/bin/env python3
"""Mechanical acceptance checker for release gate G0-01.

G0-01 (`notes/plan/docs/52_RELEASE_GATES_REV3.md`, plan §22) says:

    All load-bearing experiments in `../notes/G0_SPIKE_MATRIX.md` have evidence or an
    explicit redesign decision, recorded in the matrix itself. A failed or unexecuted
    freeze-blocking item blocks interface freeze. [...] Items whose experiments require
    later subsystems are re-homed to the gates that own them [...] with the Phase A spike
    results for DX-04, 05, 07, and 08 recorded as artifact-shape evidence only, and each
    re-homing recorded in the matrix as that item's explicit decision. [...] The matrix
    carries Status, Evidence, and Decision columns; plan §0.3's counts are derived from it,
    not asserted beside it.

This script re-derives that criterion from the artifacts instead of trusting prose about
them. It is deliberately NOT wired into the `Justfile` or `just check`: it audits the
dossier's own gate bookkeeping, it reads across `crates/` and `notes/plan/` together, and a
gate acceptance is a thing a reviewer runs and reads, not a thing that turns a build red.
Invoke it by hand:

    python3 notes/plan/tools/check_g0_matrix.py            # audit the tree
    python3 notes/plan/tools/check_g0_matrix.py --self-test # negative controls
    python3 notes/plan/tools/check_g0_matrix.py --matrix /tmp/perturbed.md

Standard library only, no new dependencies.

# Findings, and what they mean

Two severities, and the distinction is INV-008's:

* `ERROR` — the criterion is false as written. Exit code 1.
* `WARN`  — the checker cannot decide, or decided something a human must read. These are
  recorded absences (INV-007), not passes and not failures. They do not change the exit
  code, because a checker that failed on everything it could not decide would be a checker
  nobody could read.

# Rules

* `R1-rows`        — the matrix carries exactly the 15 rows `G0-DX-01`…`G0-DX-15`, once each.
* `R2-columns`     — every row has a non-empty `Status`, `Evidence`, and `Decision` cell.
* `R3-freeze`      — every freeze-blocking row is executed-with-evidence or carries an
                     explicit recorded decision. An open/unexecuted/failed freeze-blocker
                     with no decision is the counterexample the criterion names.
* `R4-rehome`      — every re-homed row names its target gate in *both* Status and Decision,
                     and the gate matches `docs/52`'s assignment.
* `R5-artifact`    — DX-04/05/07/08 cite a Phase A spike result and mark it artifact-shape
                     evidence only.
* `R6-pointers`    — every artifact pointer in Evidence and Decision resolves to a file that
                     exists, and every `<file.md> §N` citation resolves to a section N in
                     that file.
* `R7-substance`   — a cited Rust test file under `crates/*/tests/` contains at least one
                     `#[test]`, and (reported, not fatal) any `#[ignore]` in it.
* `R8-plan-counts` — plan §0.3's G0 status bullet agrees with what this script counts from
                     the matrix: which items carry evidence, which are re-homed and where,
                     and whether any freeze-blocking item is open.
* `R9-bones`       — (opt-in, `--bones-db`) every `bn-*` cited in the matrix exists.
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import re
import sys
from dataclasses import dataclass, field

# --------------------------------------------------------------------------------------
# The criterion's own constants, transcribed from docs/52_RELEASE_GATES_REV3.md.
# --------------------------------------------------------------------------------------

ALL_ITEMS = [f"G0-DX-{n:02d}" for n in range(1, 16)]

#: "the freeze-blocking subset DX-01-03, 10, 12, 13, 14"
FREEZE_BLOCKING = {
    "G0-DX-01",
    "G0-DX-02",
    "G0-DX-03",
    "G0-DX-10",
    "G0-DX-12",
    "G0-DX-13",
    "G0-DX-14",
}

#: "DX-04 (causal debugger) -> G4, DX-05 (incrementality) -> G5, DX-06 -> G4,
#: DX-07 -> G7, DX-08 -> G6, DX-09 -> G8, DX-11 -> G6, DX-15 -> G9"
REHOMED = {
    "G0-DX-04": "G4",
    "G0-DX-05": "G5",
    "G0-DX-06": "G4",
    "G0-DX-07": "G7",
    "G0-DX-08": "G6",
    "G0-DX-09": "G8",
    "G0-DX-11": "G6",
    "G0-DX-15": "G9",
}

#: "the Phase A spike results for DX-04, 05, 07, and 08 recorded as artifact-shape
#: evidence only"
ARTIFACT_SHAPE_ONLY = {"G0-DX-04", "G0-DX-05", "G0-DX-07", "G0-DX-08"}

MATRIX_RELPATH = "notes/plan/notes/G0_SPIKE_MATRIX.md"
PLAN_RELPATH = "notes/plan/plan.md"

#: Words that mean a row's experiment did not run, or ran and failed.
UNEXECUTED_MARKERS = ("open", "unexecuted", "not run", "pending", "todo", "tbd", "planned")
FAILED_MARKERS = ("failed", "fails", "falsified")
EVIDENCE_MARKERS = ("evidence", "closed", "pass")

# --------------------------------------------------------------------------------------
# Findings
# --------------------------------------------------------------------------------------

ERROR = "ERROR"
WARN = "WARN"


@dataclass
class Finding:
    severity: str
    rule: str
    item: str
    message: str

    def render(self) -> str:
        return f"{self.severity:5} {self.rule:14} {self.item:9} {self.message}"


@dataclass
class Row:
    item: str
    cells: list[str]

    @property
    def status(self) -> str:
        return self.cells[5]

    @property
    def evidence(self) -> str:
        return self.cells[6]

    @property
    def decision(self) -> str:
        return self.cells[7]


# --------------------------------------------------------------------------------------
# Parsing
# --------------------------------------------------------------------------------------

HEADER_COLUMNS = [
    "ID",
    "Question",
    "Required experiment",
    "Pass condition",
    "Failure consequence",
    "Status",
    "Evidence",
    "Decision",
]


def split_row(line: str) -> list[str]:
    """Split one GitHub-flavoured table row into its cells."""
    body = line.strip()
    if body.startswith("|"):
        body = body[1:]
    if body.endswith("|"):
        body = body[:-1]
    return [cell.strip() for cell in body.split("|")]


def parse_matrix(text: str) -> tuple[list[Row], list[Finding]]:
    """Parse the falsification matrix into rows, checking the column contract first."""
    findings: list[Finding] = []
    rows: list[Row] = []
    header: list[str] | None = None

    for line in text.splitlines():
        stripped = line.strip()
        if not stripped.startswith("|"):
            continue
        cells = split_row(stripped)
        if header is None:
            if cells and cells[0] == "ID":
                header = cells
                if cells != HEADER_COLUMNS:
                    findings.append(
                        Finding(
                            ERROR,
                            "R2-columns",
                            "-",
                            f"table header is {cells}, expected {HEADER_COLUMNS}",
                        )
                    )
            continue
        if not cells or not re.fullmatch(r"G0-DX-\d\d", cells[0]):
            continue
        if len(cells) != len(HEADER_COLUMNS):
            findings.append(
                Finding(
                    ERROR,
                    "R2-columns",
                    cells[0],
                    f"row has {len(cells)} cells, expected {len(HEADER_COLUMNS)}",
                )
            )
            cells = (cells + [""] * len(HEADER_COLUMNS))[: len(HEADER_COLUMNS)]
        rows.append(Row(item=cells[0], cells=cells))

    if header is None:
        findings.append(Finding(ERROR, "R2-columns", "-", "no matrix table header found"))
    return rows, findings


# --------------------------------------------------------------------------------------
# Pointer extraction and resolution
# --------------------------------------------------------------------------------------

FILE_EXTENSIONS = "rs|md|py|txt|json|toml|lean|ctm|idl|jsonl"

PATH_RE = re.compile(
    r"(?<![A-Za-z0-9_./-])((?:[A-Za-z0-9_.-]+/)+[A-Za-z0-9_.-]+\.(?:" + FILE_EXTENSIONS + r"))\b"
)

TOP_LEVEL_DIRS = ("crates", "notes", "lean", "tools", "corpus", "spikes", "docs")

DIR_RE = re.compile(
    r"(?<![A-Za-z0-9_./-])((?:" + "|".join(TOP_LEVEL_DIRS) + r")/[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)*/)"
)

#: `spikes/R3_SPIKE_REPORT.md §6`, `spikes/R3_SPIKE_REPORT.md §6 (causal debugger)`
SECTION_RE = re.compile(
    r"((?:[A-Za-z0-9_.-]+/)*[A-Za-z0-9_.-]+\.md)\s*§\s*(\d+(?:\.\d+)*)"
)

BONE_RE = re.compile(r"\bbn-[a-z0-9]{3,6}\b")


@dataclass
class Resolution:
    pointer: str
    ok: bool
    base: str = ""
    bases: list[str] = field(default_factory=list)


class Tree:
    """The repository tree a pointer resolves against."""

    def __init__(self, root: str):
        self.root = os.path.abspath(root)
        self.crate_dirs = sorted(
            os.path.basename(p)
            for p in glob.glob(os.path.join(self.root, "crates", "*"))
            if os.path.isdir(p)
        )

    def bases(self) -> list[tuple[str, str]]:
        out = [("root", ""), ("plan", os.path.join("notes", "plan"))]
        out += [(f"crate:{c}", os.path.join("crates", c)) for c in self.crate_dirs]
        return out

    def resolve(self, pointer: str) -> Resolution:
        hits: list[str] = []
        for label, prefix in self.bases():
            candidate = os.path.join(self.root, prefix, pointer)
            if os.path.exists(candidate):
                hits.append(label)
        if not hits:
            return Resolution(pointer, ok=False)
        return Resolution(pointer, ok=True, base=hits[0], bases=hits)

    def path_for(self, pointer: str, base: str) -> str:
        prefix = dict(self.bases())[base]
        return os.path.join(self.root, prefix, pointer)


def pointers_in(text: str) -> list[str]:
    """Every artifact pointer a cell cites, de-duplicated, in first-seen order."""
    seen: list[str] = []
    for match in list(PATH_RE.finditer(text)) + list(DIR_RE.finditer(text)):
        pointer = match.group(1)
        if pointer not in seen:
            seen.append(pointer)
    return seen


def doc_refs_in(text: str) -> list[str]:
    """`docs/53`-style dossier references, which name a file by number, not by name."""
    return sorted(set(re.findall(r"\bdocs/(\d{2})\b", text)))


# --------------------------------------------------------------------------------------
# Rules
# --------------------------------------------------------------------------------------


def normalise(cell: str) -> str:
    return re.sub(r"[`*_]", "", cell).lower()


def rule_rows(rows: list[Row]) -> list[Finding]:
    findings: list[Finding] = []
    seen = [row.item for row in rows]
    for item in ALL_ITEMS:
        count = seen.count(item)
        if count == 0:
            findings.append(Finding(ERROR, "R1-rows", item, "missing from the matrix"))
        elif count > 1:
            findings.append(Finding(ERROR, "R1-rows", item, f"appears {count} times"))
    for item in seen:
        if item not in ALL_ITEMS:
            findings.append(Finding(ERROR, "R1-rows", item, "unexpected row id"))
    return findings


def rule_columns(rows: list[Row]) -> list[Finding]:
    findings: list[Finding] = []
    for row in rows:
        for name, value in (
            ("Status", row.status),
            ("Evidence", row.evidence),
            ("Decision", row.decision),
        ):
            if not value.strip():
                findings.append(
                    Finding(ERROR, "R2-columns", row.item, f"{name} cell is empty")
                )
    return findings


def rule_freeze(rows: list[Row]) -> list[Finding]:
    """A freeze-blocking row must be executed with evidence, or carry a decision."""
    findings: list[Finding] = []
    for row in rows:
        if row.item not in FREEZE_BLOCKING:
            continue
        status = normalise(row.status)
        evidence = row.evidence.strip()
        decision = row.decision.strip()
        has_evidence = bool(evidence) and evidence.lower() not in {"none", "n/a", "-"}
        has_decision = bool(decision) and decision.lower() not in {"none", "n/a", "-"}
        unexecuted = any(marker in status for marker in UNEXECUTED_MARKERS)
        failed = any(marker in status for marker in FAILED_MARKERS)
        claims_evidence = any(marker in status for marker in EVIDENCE_MARKERS)

        if unexecuted and not has_decision:
            findings.append(
                Finding(
                    ERROR,
                    "R3-freeze",
                    row.item,
                    f"freeze-blocking and unexecuted ({row.status!r}) with no recorded decision",
                )
            )
        if failed and not has_decision:
            findings.append(
                Finding(
                    ERROR,
                    "R3-freeze",
                    row.item,
                    f"freeze-blocking and failed ({row.status!r}) with no recorded decision",
                )
            )
        if not has_evidence and not has_decision:
            findings.append(
                Finding(
                    ERROR,
                    "R3-freeze",
                    row.item,
                    "freeze-blocking with neither evidence nor a decision",
                )
            )
        if not claims_evidence and not unexecuted and not failed:
            findings.append(
                Finding(
                    WARN,
                    "R3-freeze",
                    row.item,
                    f"Status {row.status!r} is neither an evidence claim nor a named failure;"
                    " a human must read it",
                )
            )
        if failed and has_decision:
            findings.append(
                Finding(
                    WARN,
                    "R3-freeze",
                    row.item,
                    "Status records a FAILURE with a decision; the failure consequence in the"
                    " row must be in force — read the Decision cell",
                )
            )
    return findings


def rule_rehome(rows: list[Row]) -> list[Finding]:
    findings: list[Finding] = []
    by_item = {row.item: row for row in rows}
    for item, gate in REHOMED.items():
        row = by_item.get(item)
        if row is None:
            continue
        status = normalise(row.status)
        decision = normalise(row.decision)
        if "re-homed" not in status and "rehomed" not in status:
            findings.append(
                Finding(
                    ERROR,
                    "R4-rehome",
                    item,
                    f"expected a re-homed Status, found {row.status!r}",
                )
            )
        status_gates = set(re.findall(r"\bg(\d+)\b", status))
        decision_gates = set(re.findall(r"\bg(\d+)\b", decision))
        want = gate[1:]
        if want not in status_gates:
            findings.append(
                Finding(
                    ERROR,
                    "R4-rehome",
                    item,
                    f"Status does not name target gate {gate}: {row.status!r}",
                )
            )
        elif status_gates != {want}:
            findings.append(
                Finding(
                    ERROR,
                    "R4-rehome",
                    item,
                    f"Status names gates {sorted(status_gates)}, expected only {gate}",
                )
            )
        if want not in decision_gates:
            findings.append(
                Finding(
                    ERROR,
                    "R4-rehome",
                    item,
                    "the re-homing is not recorded as this item's explicit Decision"
                    f" (Decision does not name {gate}): {row.decision!r}",
                )
            )
    for row in rows:
        if row.item in REHOMED:
            continue
        status = normalise(row.status)
        if "re-homed" in status or "rehomed" in status:
            findings.append(
                Finding(
                    ERROR,
                    "R4-rehome",
                    row.item,
                    f"row is re-homed but docs/52 does not re-home it: {row.status!r}",
                )
            )
    return findings


def rule_artifact_shape(rows: list[Row]) -> list[Finding]:
    findings: list[Finding] = []
    by_item = {row.item: row for row in rows}
    for item in sorted(ARTIFACT_SHAPE_ONLY):
        row = by_item.get(item)
        if row is None:
            continue
        if "spike" not in normalise(row.evidence):
            findings.append(
                Finding(
                    ERROR,
                    "R5-artifact",
                    item,
                    "docs/52 records a Phase A spike result for this item;"
                    f" Evidence does not cite one: {row.evidence!r}",
                )
            )
        if "artifact shape" not in normalise(row.decision) and "artifact-shape" not in normalise(
            row.decision
        ):
            findings.append(
                Finding(
                    ERROR,
                    "R5-artifact",
                    item,
                    "Decision does not record the spike as artifact-shape evidence only:"
                    f" {row.decision!r}",
                )
            )
    for row in rows:
        if row.item in REHOMED and row.item not in ARTIFACT_SHAPE_ONLY:
            if "spike" in normalise(row.evidence):
                findings.append(
                    Finding(
                        WARN,
                        "R5-artifact",
                        row.item,
                        "cites a spike result but docs/52 lists no Phase A spike for it",
                    )
                )
    return findings


def rule_pointers(rows: list[Row], tree: Tree) -> tuple[list[Finding], dict[str, list[str]]]:
    findings: list[Finding] = []
    resolved: dict[str, list[str]] = {}
    for row in rows:
        cited: list[str] = []
        for column, cell in (("Evidence", row.evidence), ("Decision", row.decision)):
            for pointer in pointers_in(cell):
                resolution = tree.resolve(pointer)
                if not resolution.ok:
                    findings.append(
                        Finding(
                            ERROR,
                            "R6-pointers",
                            row.item,
                            f"{column} cites {pointer!r}, which does not exist in the tree",
                        )
                    )
                    continue
                cited.append(tree.path_for(pointer, resolution.base))
                if resolution.base != "root":
                    findings.append(
                        Finding(
                            WARN,
                            "R6-pointers",
                            row.item,
                            f"{column} cites {pointer!r}, which resolves only relative to"
                            f" {resolution.base} (bases: {resolution.bases})",
                        )
                    )
            for md, section in SECTION_RE.findall(cell):
                resolution = tree.resolve(md)
                if not resolution.ok:
                    continue  # already reported by the path rule
                path = tree.path_for(md, resolution.base)
                try:
                    with open(path, encoding="utf-8") as handle:
                        body = handle.read()
                except OSError as exc:  # pragma: no cover - unreadable cited file
                    findings.append(
                        Finding(ERROR, "R6-pointers", row.item, f"cannot read {md}: {exc}")
                    )
                    continue
                pattern = re.compile(
                    r"^#{1,6}\s+" + re.escape(section) + r"[.\s]", re.MULTILINE
                )
                if not pattern.search(body):
                    findings.append(
                        Finding(
                            ERROR,
                            "R6-pointers",
                            row.item,
                            f"{column} cites {md} §{section}, which has no such section",
                        )
                    )
            for number in doc_refs_in(cell):
                if not glob.glob(os.path.join(tree.root, "notes", "plan", "docs", f"{number}_*.md")):
                    findings.append(
                        Finding(
                            ERROR,
                            "R6-pointers",
                            row.item,
                            f"{column} cites docs/{number}, which does not exist",
                        )
                    )
        resolved[row.item] = cited
    return findings, resolved


def rule_substance(resolved: dict[str, list[str]]) -> list[Finding]:
    """A cited test file must actually contain tests, and ignored tests are reported."""
    findings: list[Finding] = []
    for item, paths in resolved.items():
        for path in paths:
            if not path.endswith(".rs"):
                continue
            if f"{os.sep}tests{os.sep}" not in path:
                continue
            try:
                with open(path, encoding="utf-8") as handle:
                    body = handle.read()
            except OSError as exc:  # pragma: no cover
                findings.append(Finding(ERROR, "R7-substance", item, f"cannot read {path}: {exc}"))
                continue
            tests = len(re.findall(r"^\s*#\[test\]", body, re.MULTILINE))
            ignored = len(re.findall(r"^\s*#\[ignore", body, re.MULTILINE))
            rel = os.path.basename(path)
            if tests == 0:
                findings.append(
                    Finding(
                        ERROR,
                        "R7-substance",
                        item,
                        f"cited test file {rel} contains no #[test] function",
                    )
                )
            if ignored:
                findings.append(
                    Finding(
                        ERROR,
                        "R7-substance",
                        item,
                        f"cited test file {rel} contains {ignored} #[ignore] attribute(s);"
                        " an ignored test does not run in the default suite",
                    )
                )
    return findings


# --------------------------------------------------------------------------------------
# plan §0.3 derivation
# --------------------------------------------------------------------------------------


def extract_plan_g0_bullet(plan_text: str) -> str | None:
    match = re.search(r"^- G0 status:(.*?)(?=^- [A-Z])", plan_text, re.MULTILINE | re.DOTALL)
    if not match:
        return None
    return " ".join(match.group(1).split())


def rule_plan_counts(rows: list[Row], plan_text: str) -> list[Finding]:
    findings: list[Finding] = []
    bullet = extract_plan_g0_bullet(plan_text)
    if bullet is None:
        findings.append(
            Finding(WARN, "R8-plan-counts", "-", "could not locate plan §0.3's 'G0 status:' bullet")
        )
        return findings

    def items_in(fragment: str) -> set[str]:
        return {f"G0-DX-{n}" for n in re.findall(r"DX-(\d\d)", fragment)}

    # Derived from the matrix, by counting.
    matrix_evidence = {
        row.item for row in rows if normalise(row.status).startswith("evidence")
    }
    matrix_rehomed = {
        row.item
        for row in rows
        if "re-homed" in normalise(row.status) or "rehomed" in normalise(row.status)
    }
    matrix_open_freeze = {
        row.item
        for row in rows
        if row.item in FREEZE_BLOCKING
        and any(marker in normalise(row.status) for marker in UNEXECUTED_MARKERS)
    }

    claim_evidence = re.search(r"(DX-\d\d(?:[^.]*?))carry spike\s*evidence", bullet)
    if claim_evidence:
        claimed = items_in(claim_evidence.group(1))
        if claimed != matrix_evidence:
            findings.append(
                Finding(
                    ERROR,
                    "R8-plan-counts",
                    "-",
                    f"plan §0.3 says {sorted(claimed)} carry evidence; the matrix says"
                    f" {sorted(matrix_evidence)}",
                )
            )
    else:
        findings.append(
            Finding(WARN, "R8-plan-counts", "-", "plan §0.3 has no 'carry spike evidence' clause")
        )

    claim_rehomed = re.search(r"(DX-\d\d[^.]*?)are\s+re-homed", bullet)
    if claim_rehomed:
        claimed = items_in(claim_rehomed.group(1))
        if claimed != matrix_rehomed:
            findings.append(
                Finding(
                    ERROR,
                    "R8-plan-counts",
                    "-",
                    f"plan §0.3 re-homes {sorted(claimed)}; the matrix re-homes"
                    f" {sorted(matrix_rehomed)}",
                )
            )
        gates = re.search(r"re-homed to the gates owning their machinery \(([^)]*)\)", bullet)
        if gates:
            named = re.findall(r"G(\d+)", gates.group(1))
            expected = [REHOMED[item][1:] for item in sorted(claimed)]
            if named[: len(expected)] != expected:
                findings.append(
                    Finding(
                        ERROR,
                        "R8-plan-counts",
                        "-",
                        f"plan §0.3's gate list {named} does not match docs/52's"
                        f" assignment {expected} for {sorted(claimed)}",
                    )
                )
    else:
        findings.append(
            Finding(WARN, "R8-plan-counts", "-", "plan §0.3 has no 're-homed' clause")
        )

    says_none_open = "no item is open and freeze-blocking" in bullet.lower()
    if says_none_open and matrix_open_freeze:
        findings.append(
            Finding(
                ERROR,
                "R8-plan-counts",
                "-",
                "plan §0.3 says no freeze-blocking item is open; the matrix has"
                f" {sorted(matrix_open_freeze)}",
            )
        )
    if not says_none_open and not matrix_open_freeze:
        findings.append(
            Finding(
                WARN,
                "R8-plan-counts",
                "-",
                "plan §0.3 does not state the open-freeze-blocker count the matrix supports",
            )
        )
    return findings


def rule_bones(rows: list[Row], bones_db: str) -> list[Finding]:
    import sqlite3

    findings: list[Finding] = []
    try:
        connection = sqlite3.connect(f"file:{bones_db}?mode=ro", uri=True)
        present = {row[0] for row in connection.execute("select item_id from items")}
    except sqlite3.Error as exc:
        return [Finding(WARN, "R9-bones", "-", f"cannot read {bones_db}: {exc}")]
    for row in rows:
        for bone in sorted(set(BONE_RE.findall(row.evidence + " " + row.decision))):
            if bone not in present:
                findings.append(
                    Finding(ERROR, "R9-bones", row.item, f"cites {bone}, which is not a bone")
                )
    return findings


# --------------------------------------------------------------------------------------
# Driver
# --------------------------------------------------------------------------------------


def audit(
    matrix_text: str,
    plan_text: str | None,
    tree: Tree | None,
    bones_db: str | None = None,
) -> list[Finding]:
    rows, findings = parse_matrix(matrix_text)
    findings += rule_rows(rows)
    findings += rule_columns(rows)
    findings += rule_freeze(rows)
    findings += rule_rehome(rows)
    findings += rule_artifact_shape(rows)
    if tree is not None:
        pointer_findings, resolved = rule_pointers(rows, tree)
        findings += pointer_findings
        findings += rule_substance(resolved)
    if plan_text is not None:
        findings += rule_plan_counts(rows, plan_text)
    if bones_db:
        findings += rule_bones(rows, bones_db)
    return findings


def find_root(start: str) -> str:
    current = os.path.abspath(start)
    while True:
        if os.path.isdir(os.path.join(current, "crates")) and os.path.isfile(
            os.path.join(current, MATRIX_RELPATH)
        ):
            return current
        parent = os.path.dirname(current)
        if parent == current:
            raise SystemExit("cannot locate the repository root; pass --root")
        current = parent


# --------------------------------------------------------------------------------------
# Self-test: the negative controls
# --------------------------------------------------------------------------------------

GOOD_HEADER = (
    "| ID | Question | Required experiment | Pass condition | Failure consequence |"
    " Status | Evidence | Decision |\n"
    "|---|---|---|---|---|---|---|---|\n"
)


def synthetic_matrix(overrides: dict[str, tuple[str, str, str]] | None = None) -> str:
    """A minimal well-formed matrix, with per-item (Status, Evidence, Decision) overrides."""
    overrides = overrides or {}
    lines = [GOOD_HEADER]
    for item in ALL_ITEMS:
        if item in REHOMED:
            gate = REHOMED[item]
            phase = f"Re-homed → {gate} (Phase X)"
            if item in ARTIFACT_SHAPE_ONLY:
                default = (phase, "spikes/R3_SPIKE_REPORT.md", f"artifact shape adopted; {gate}")
            else:
                default = (phase, "none", f"runs as the {gate} gate")
        else:
            default = ("Evidence (reference implementation)", "some evidence", "held")
        status, evidence, decision = overrides.get(item, default)
        lines.append(
            f"| {item} | q | e | p | c | {status} | {evidence} | {decision} |\n"
        )
    return "".join(lines)


def rules_fired(findings: list[Finding], severity: str = ERROR) -> set[str]:
    return {f.rule for f in findings if f.severity == severity}


def self_test() -> int:
    """Exercise every negative control. Returns a process exit code."""
    failures: list[str] = []

    def case(name: str, findings: list[Finding], expect_rule: str, expect_item: str | None = None):
        fired = [f for f in findings if f.severity == ERROR and f.rule == expect_rule]
        if expect_item is not None:
            fired = [f for f in fired if f.item == expect_item]
        if not fired:
            failures.append(
                f"{name}: expected an {ERROR} from {expect_rule}"
                + (f" on {expect_item}" if expect_item else "")
                + f"; got {[f.render() for f in findings if f.severity == ERROR]}"
            )
        else:
            print(f"  detected  {name}: {fired[0].render()}")

    def clean(name: str, findings: list[Finding]):
        errs = [f.render() for f in findings if f.severity == ERROR]
        if errs:
            failures.append(f"{name}: expected no ERROR, got {errs}")
        else:
            print(f"  clean     {name}")

    print("self-test: positive control")
    clean("well-formed synthetic matrix", audit(synthetic_matrix(), None, None))

    print("self-test: negative controls")

    # NC1 — freeze-blocker with an empty Decision cell.
    case(
        "NC1 freeze-blocker with empty Decision",
        audit(
            synthetic_matrix({"G0-DX-13": ("Evidence (reference implementation)", "ev", "")}),
            None,
            None,
        ),
        "R2-columns",
        "G0-DX-13",
    )

    # NC2 — freeze-blocker open and undecided.
    case(
        "NC2 freeze-blocker Open with no decision",
        audit(
            synthetic_matrix({"G0-DX-10": ("Open — freeze-blocking (Phase A)", "ev", "")}),
            None,
            None,
        ),
        "R3-freeze",
        "G0-DX-10",
    )

    # NC3 — freeze-blocker failed with no decision.
    case(
        "NC3 freeze-blocker failed with no decision",
        audit(
            synthetic_matrix({"G0-DX-02": ("Failed as measured", "ev", "")}),
            None,
            None,
        ),
        "R3-freeze",
        "G0-DX-02",
    )

    # NC4 — re-homed item sent to the wrong gate.
    case(
        "NC4 re-homing to the wrong gate",
        audit(
            synthetic_matrix(
                {"G0-DX-05": ("Re-homed → G9", "spikes/R3_SPIKE_REPORT.md", "artifact shape; G9")}
            ),
            None,
            None,
        ),
        "R4-rehome",
        "G0-DX-05",
    )

    # NC5 — re-homing not recorded as the item's Decision.
    case(
        "NC5 re-homing absent from the Decision cell",
        audit(
            synthetic_matrix({"G0-DX-11": ("Re-homed → G6", "none", "deferred")}),
            None,
            None,
        ),
        "R4-rehome",
        "G0-DX-11",
    )

    # NC6 — artifact-shape item whose Decision claims engine evidence.
    case(
        "NC6 spike promoted past artifact shape",
        audit(
            synthetic_matrix(
                {"G0-DX-07": ("Re-homed → G7", "spikes/R3_SPIKE_REPORT.md", "engine evidence; G7")}
            ),
            None,
            None,
        ),
        "R5-artifact",
        "G0-DX-07",
    )

    # NC7 — a missing row.
    text = synthetic_matrix()
    text = "\n".join(line for line in text.splitlines() if "G0-DX-06" not in line) + "\n"
    case("NC7 missing row", audit(text, None, None), "R1-rows", "G0-DX-06")

    # NC8 — a dangling evidence pointer, checked against a real tree.
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        os.makedirs(os.path.join(tmp, "crates", "demo", "tests"))
        os.makedirs(os.path.join(tmp, "notes", "plan", "docs"))
        os.makedirs(os.path.join(tmp, "notes", "plan", "spikes"))
        with open(
            os.path.join(tmp, "notes", "plan", "spikes", "R3_SPIKE_REPORT.md"), "w"
        ) as handle:
            handle.write("# spikes\n")
        with open(os.path.join(tmp, "crates", "demo", "tests", "real.rs"), "w") as handle:
            handle.write("#[test]\nfn t() {}\n")
        with open(os.path.join(tmp, "crates", "demo", "tests", "empty.rs"), "w") as handle:
            handle.write("// no tests here\n")
        tree = Tree(tmp)
        case(
            "NC8 dangling evidence pointer",
            audit(
                synthetic_matrix(
                    {"G0-DX-01": ("Evidence", "crates/demo/tests/ghost.rs", "held")}
                ),
                None,
                tree,
            ),
            "R6-pointers",
            "G0-DX-01",
        )
        case(
            "NC9 cited docs/NN that does not exist",
            audit(
                synthetic_matrix({"G0-DX-03": ("Evidence", "per docs/99", "held")}),
                None,
                tree,
            ),
            "R6-pointers",
            "G0-DX-03",
        )
        case(
            "NC10 cited test file with no #[test]",
            audit(
                synthetic_matrix(
                    {"G0-DX-12": ("Evidence", "crates/demo/tests/empty.rs", "held")}
                ),
                None,
                tree,
            ),
            "R7-substance",
            "G0-DX-12",
        )
        case(
            "NC11 cited test file with an ignored test",
            audit(
                synthetic_matrix(
                    {"G0-DX-14": ("Evidence", "crates/demo/tests/ign.rs", "held")}
                ),
                None,
                _tree_with_ignored(tmp),
            ),
            "R7-substance",
            "G0-DX-14",
        )
        clean(
            "positive control against a real tree",
            audit(
                synthetic_matrix(
                    {"G0-DX-01": ("Evidence", "crates/demo/tests/real.rs", "held")}
                ),
                None,
                tree,
            ),
        )

    # NC12 — a §N citation into a section the file does not have.
    with tempfile.TemporaryDirectory() as tmp:
        os.makedirs(os.path.join(tmp, "crates"))
        os.makedirs(os.path.join(tmp, "notes", "plan", "spikes"))
        with open(os.path.join(tmp, "notes", "plan", "spikes", "S.md"), "w") as handle:
            handle.write("# S\n\n## 1. one\n\n## 2. two\n")
        tree = Tree(tmp)
        case(
            "NC12 spike section citation that does not resolve",
            audit(
                synthetic_matrix({"G0-DX-02": ("Evidence", "spikes/S.md §9", "held")}),
                None,
                tree,
            ),
            "R6-pointers",
            "G0-DX-02",
        )

    # NC13 — plan §0.3 asserting counts the matrix does not support.
    plan_bad = (
        "- G0 status: DX-01 and DX-02 carry spike evidence. No item is open and\n"
        "  freeze-blocking (Phase A). DX-04, DX-05, DX-06, DX-07, DX-08, DX-09,\n"
        "  DX-11, and DX-15 are re-homed to the gates owning their machinery (G4,\n"
        "  G5, G4, G7, G6, G8, G6, G9 respectively).\n"
        "- Program status: READY\n"
    )
    case(
        "NC13 plan §0.3 counts asserted beside the matrix, not derived from it",
        audit(synthetic_matrix(), plan_bad, None),
        "R8-plan-counts",
    )

    plan_good = (
        "- G0 status: DX-01, DX-02, DX-03, DX-04, DX-05, DX-06, DX-07, DX-08, DX-09,\n"
        "  DX-10, DX-11, DX-12, DX-13, DX-14, and DX-15 carry spike evidence.\n"
        "- Program status: READY\n"
    )
    # Only the seven non-re-homed rows claim evidence in the synthetic matrix, so the
    # all-fifteen claim above must be refused too.
    case(
        "NC14 plan §0.3 over-counting evidence rows",
        audit(synthetic_matrix(), plan_good, None),
        "R8-plan-counts",
    )

    print()
    if failures:
        for failure in failures:
            print(f"SELF-TEST FAILURE: {failure}")
        return 1
    print("self-test: all negative controls detected, positive controls clean")
    return 0


def _tree_with_ignored(tmp: str) -> Tree:
    path = os.path.join(tmp, "crates", "demo", "tests", "ign.rs")
    with open(path, "w") as handle:
        handle.write("#[test]\n#[ignore = \"slow\"]\nfn t() {}\n")
    return Tree(tmp)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--root", help="repository root (default: discovered from this file)")
    parser.add_argument("--matrix", help=f"matrix path (default: <root>/{MATRIX_RELPATH})")
    parser.add_argument("--plan", help=f"plan path (default: <root>/{PLAN_RELPATH})")
    parser.add_argument("--bones-db", help="optional .bones/bones.db, to resolve cited bone ids")
    parser.add_argument("--self-test", action="store_true", help="run the negative controls")
    parser.add_argument("--format", choices=("text", "json"), default="text")
    args = parser.parse_args(argv)

    if args.self_test:
        return self_test()

    root = args.root or find_root(os.path.dirname(os.path.abspath(__file__)))
    matrix_path = args.matrix or os.path.join(root, MATRIX_RELPATH)
    plan_path = args.plan or os.path.join(root, PLAN_RELPATH)

    with open(matrix_path, encoding="utf-8") as handle:
        matrix_text = handle.read()
    plan_text = None
    if os.path.isfile(plan_path):
        with open(plan_path, encoding="utf-8") as handle:
            plan_text = handle.read()

    findings = audit(matrix_text, plan_text, Tree(root), args.bones_db)
    errors = [f for f in findings if f.severity == ERROR]
    warns = [f for f in findings if f.severity == WARN]

    if args.format == "json":
        print(
            json.dumps(
                {
                    "root": root,
                    "matrix": matrix_path,
                    "errors": len(errors),
                    "warnings": len(warns),
                    "verdict": "FAILED" if errors else "SATISFIED",
                    "findings": [f.__dict__ for f in findings],
                },
                indent=2,
            )
        )
    else:
        print(f"G0-01 matrix audit: {matrix_path}")
        print(f"tree: {root}")
        print()
        for finding in findings:
            print(finding.render())
        if findings:
            print()
        print(f"{len(errors)} error(s), {len(warns)} warning(s)")
        print("verdict: " + ("FAILED" if errors else "SATISFIED (mechanical rules only)"))
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
