#!/usr/bin/env python3
"""The shared harness for executable policy obligations (GOV §4 onward, TEST §2 onward).

Source of truth: `notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md` (GOV-n-mm
ids) and `notes/plan/docs/19_TEST_STRATEGY.md` (TEST-n-mm ids), carried by
`notes/plan/notes/PLAN_REQUIREMENTS.json`. The first obligation set built on it
is `obligations/gov4_semantic.py` (GOV-4-01 … GOV-4-06, bn-37b1).

Why a harness
-------------

GOV §1–§3 each shipped one checker script with its own fixture loader, its own
self-test, and its own evidence writer. GOV §4 has 21 obligations over four
change classes, and docs/19 §2–§9 add some sixty more. One script per slice
would copy the same loader, runner, regime logic, and evidence shape a dozen
times, and one shared script would make every sibling Bone edit the same lines.
This file is the part they share; an **obligation set** is the part each Bone
owns.

The extension point
-------------------

An obligation set is one Python file in `tools/governance/obligations/` whose
name does not start with `_`. The harness discovers every such file (sorted by
name) and reads one attribute from it, `SET`, an `ObligationSet`. A sibling Bone
adds a file there, a fixture directory under
`tools/governance/fixtures/obligations/<set name>/`, and an evidence file
named by its set. It does not edit this file, the Justfile, the CI workflow,
or another set: `just check` already runs every discovered set. See
`tools/governance/README-gov4.md` for the full contract.

Two kinds of rule
-----------------

- **tree** rules read one revision — the working tree — and run on every
  invocation. Their real-run result is recorded in the evidence file.
- **delta** rules read two revisions — a base (default: the merge base with
  the trunk) and the working tree — and judge the *change*. They reuse
  `check_revision_delta.py`'s base resolution and delta computation unchanged,
  so "what changed" has one definition in the repository. Their verdict is a
  function of history, so it goes to stdout and CI and is never written into a
  committed evidence file.

Regime: a set comes into force for branches cut after it landed
-----------------------------------------------------------------

A new obligation set must not fail every branch already in flight for a rule
that did not exist when the branch was cut. The harness therefore reads the
set's own module file *at the base*: present → the set's delta rules are in
force; absent → their findings are **deferred** (printed, reported, not
failures). This is a function of the base alone and needs no configuration.
The fixture `…-before-the-set-landed` of every set that has delta rules proves
the deferral path.

Review records
--------------

Several obligations say "a change of class X requires Y". The artifact that
carries Y for one change is a **review record**:
`tools/governance/reviews/<name>.toml`, added or modified by the change. The
harness owns its envelope — it parses, its `[change]` table names the Bone,
the author, and a summary, and every other top-level table is a section some
obligation set declares — and each set owns its own section's keys. Two sets
may declare keys in the same section (GOV-4-07 "claim impact" extends
`[semantic]`); the allowed keys are the union.

Self-test
---------

Every fixture under `fixtures/obligations/<set>/<id>/fixture.json` is a
base→head pair overlaid onto the real tree: the listed paths differ, everything
else reads from the repository, so a fixture rots loudly when the tree it
leans on moves. A fixture names the requirement, rule, and sub-check it must
trip (or `expect: clean|deferred`). The self-test fails if a violating fixture
is not caught by its named sub-check, if a clean fixture trips anything, or if
any declared sub-check of any rule has no violating fixture — a check that
cannot be shown to fire could pass vacuously.

Determinism (INV-005): no clock, no environment read, no network. The only
subprocess is `git`, through `check_revision_delta.Git`. Stdlib only.

Usage:

    python3 tools/governance/check_obligations.py --self-test
    python3 tools/governance/check_obligations.py
    python3 tools/governance/check_obligations.py --base origin/main --require-base
    python3 tools/governance/check_obligations.py --evidence
    python3 tools/governance/check_obligations.py --set gov-4-semantic --self-test

Exit 0 when every enforced rule holds (or the delta half skipped without
`--require-base`), 1 otherwise.
"""

from __future__ import annotations

import argparse
import dataclasses
import importlib.util
import json
import re
import sys
import tomllib
from collections.abc import Callable, Iterable, Sequence
from dataclasses import dataclass, field
from pathlib import Path
from types import ModuleType

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_code_policy as policy  # noqa: E402  (sibling import, stdlib-only)
import check_revision_delta as delta_mod  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
GOVERNANCE = "tools/governance"
OBLIGATIONS_DIR = f"{GOVERNANCE}/obligations"
FIXTURE_ROOT = f"{GOVERNANCE}/fixtures/obligations"
REVIEWS_DIR = f"{GOVERNANCE}/reviews"
REQUIREMENTS_JSON = "notes/plan/notes/PLAN_REQUIREMENTS.json"

Finding = delta_mod.Finding
COMPATIBILITY_VERDICTS = delta_mod.COMPATIBILITY_VERDICTS

# ============================================================================
# The obligation-set contract
# ============================================================================

TREE = "tree"
DELTA = "delta"


@dataclass(frozen=True)
class Rule:
    """One rule: named sub-checks, one or more requirement ids, one function.

    `fn(ctx)` returns findings whose `rule` is this rule's name and whose
    `check` is one of `checks`. A tree rule may not read `ctx.base` or
    `ctx.changed`; a delta rule may read both.
    """

    name: str
    requirements: tuple[str, ...]
    checks: tuple[str, ...]
    mode: str
    enforces: str
    boundary: str | None
    fn: Callable[[Context], list[Finding]]


@dataclass(frozen=True)
class ObligationSet:
    name: str
    title: str
    source: str
    requirements: dict[str, str]  # id -> the source bullet text it binds
    rules: tuple[Rule, ...]
    evidence: str
    # Review-record keys this set owns, per section: {"semantic": ("covers", ...)}.
    record_keys: dict[str, tuple[str, ...]] = field(default_factory=dict)
    notes: tuple[str, ...] = ()
    # Requirement ids this set enforces only in part, with the reason the
    # substance is missing. They must not carry a `(delivered: …)` record.
    undelivered: dict[str, str] = field(default_factory=dict)
    module_path: str = ""  # filled in by discovery; the regime anchor


# ============================================================================
# Views
# ============================================================================


class Reader:
    """Anything with `read_text(rel) -> str | None`."""

    def __init__(self, fn: Callable[[str], str | None]) -> None:
        self._fn = fn

    def read_text(self, rel: str) -> str | None:
        return self._fn(rel)


@dataclass(frozen=True)
class Record:
    path: str
    data: dict | None
    error: str | None


class Context:
    """What a rule may read. Built identically for the real run and for fixtures."""

    def __init__(
        self,
        head: policy.Tree,
        base: Reader | None,
        changed: tuple[str, ...] | None,
        base_label: str = "",
        head_label: str = "",
    ) -> None:
        self.head = head
        self.base = base
        self.changed = changed
        self.base_label = base_label
        self.head_label = head_label
        self._workspace: policy.Workspace | None = None

    # -- delta ------------------------------------------------------------------

    @property
    def has_delta(self) -> bool:
        return self.base is not None and self.changed is not None

    def changed_matching(self, predicate: Callable[[str], bool]) -> list[str]:
        return sorted(p for p in (self.changed or ()) if predicate(p))

    def semantic_changes(self) -> list[str]:
        """Semantic-tier source files whose *code* changed, or that the delta added.

        The same predicate GOV-1-08's delta rule uses: `check_code_policy`'s
        semantic tier, comment-stripped and whitespace-normalized text. A
        deleted file is not listed — there is nothing at head to review.
        """
        out: list[str] = []
        for rel in self.changed_matching(delta_mod.is_semantic_source):
            head = self.head.read_text(rel)
            if head is None:
                continue
            base = self.base.read_text(rel) if self.base else None
            if base is None or delta_mod.semantic_text(base) != delta_mod.semantic_text(head):
                out.append(rel)
        return out

    # -- review records -----------------------------------------------------------

    def records_in_delta(self) -> list[Record]:
        """Review records the delta adds or modifies, present at head."""
        return [r for r in self.head_records() if r.path in set(self.changed or ())]

    def head_records(self) -> list[Record]:
        out: list[Record] = []
        for rel in self.head.glob(f"{REVIEWS_DIR}/*.toml"):
            text = self.head.read_text(rel)
            if text is None:
                continue
            try:
                data = tomllib.loads(text)
            except tomllib.TOMLDecodeError as exc:
                out.append(Record(rel, None, str(exc)))
                continue
            out.append(Record(rel, data, None))
        return out

    # -- workspace ----------------------------------------------------------------

    def workspace(self) -> policy.Workspace:
        if self._workspace is None:
            self._workspace = policy.load_workspace(self.head)
        return self._workspace

    def crate_of(self, rel: str) -> str | None:
        """`crates/<dir>/…` -> the package name declared by that directory's manifest."""
        m = re.match(r"crates/([^/]+)/", rel)
        if not m:
            return None
        manifest = self.head.read_toml(f"crates/{m.group(1)}/Cargo.toml")
        name = (manifest or {}).get("package", {}).get("name")
        return name if isinstance(name, str) else None

    def reaches(self, crate: str, targets: Iterable[str]) -> bool:
        """`crate` is one of `targets` or depends on one, transitively.

        The first hop counts every dependency kind (a crate's own tests see its
        dev-dependencies); later hops count normal and build edges only.
        """
        wanted = set(targets)
        if crate in wanted:
            return True
        ws = self.workspace()
        for first in ws.deps_of(crate, ("normal", "build", "dev")):
            if first in wanted:
                return True
            if first in ws.members and wanted & set(ws.closure(first)):
                return True
        return False


# ============================================================================
# Shared helpers for obligation sets
# ============================================================================

TEST_REF_RE = re.compile(r"(crates/[^/]+/(?:tests|src)/[^:]+\.rs)::([A-Za-z_][A-Za-z0-9_]*)")


@dataclass(frozen=True)
class TestFn:
    path: str
    name: str
    attrs: str  # the attribute and comment block directly above `fn`
    body: str  # from the opening brace to the matching close, raw text


def _match_brace(code: str, start: int) -> int:
    """Index just past the brace matching `code[start] == "{"`, or -1.

    `code` must be comment-stripped (`policy.strip_rust_comments`); string and
    char literals are skipped here so a brace inside one is not counted.
    """
    depth, i, n = 0, start, len(code)
    while i < n:
        c = code[i]
        if c == '"':
            i += 1
            while i < n and code[i] != '"':
                i += 2 if code[i] == "\\" else 1
        elif c == "r" and (m := re.match(r'r(#*)"', code[i:i + 260])) and (i == 0 or not code[i - 1].isalnum()):
            close = '"' + m.group(1)
            end = code.find(close, i + m.end())
            i = n if end == -1 else end + len(close) - 1
        elif c == "'" and (m := re.match(r"'(?:\\.|[^\\'])'", code[i:i + 12])):
            i += len(m.group(0)) - 1
        elif c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return -1


def find_fn(text: str, name: str) -> TestFn | None:
    """Locate `fn <name>` in Rust source: its attribute block and its body."""
    code = policy.strip_rust_comments(text)
    m = re.search(rf"\bfn\s+{re.escape(name)}\s*[<(]", code)
    if not m:
        return None
    open_brace = code.find("{", m.end())
    if open_brace == -1:
        return None
    close = _match_brace(code, open_brace)
    if close == -1:
        return None
    # The block above: the contiguous attribute and comment lines directly over
    # the `fn` line (a blank line ends it), plus any qualifiers on that line.
    line_start = text.rfind("\n", 0, m.start()) + 1
    lines = text[:line_start].splitlines()
    block: list[str] = []
    for line in reversed(lines):
        stripped = line.strip()
        if stripped.startswith(("#[", "//", "*", "/*")) or stripped.endswith(")]"):
            block.append(line)
            continue
        break
    attrs = "\n".join(reversed(block)) + "\n" + text[line_start:m.start()]
    return TestFn("", name, attrs, text[open_brace:close])


def resolve_test(ctx: Context, ref: object) -> tuple[TestFn | None, str | None]:
    """A `crates/<c>/(tests|src)/<file>.rs::<fn>` reference, resolved at head.

    It resolves when the file exists, declares `fn <name>`, that fn carries
    `#[test]`, and it is not `#[ignore]`d — so `cargo test --workspace`, which
    `just check` runs, executes it.
    """
    if not isinstance(ref, str) or not (m := TEST_REF_RE.fullmatch(ref)):
        return None, f"{ref!r} is not a `crates/<crate>/(tests|src)/<file>.rs::<fn>` reference"
    path, name = m.group(1), m.group(2)
    text = ctx.head.read_text(path)
    if text is None:
        return None, f"{ref}: {path} does not exist at head"
    found = find_fn(text, name)
    if found is None:
        return None, f"{ref}: {path} declares no `fn {name}` with a body"
    attrs_code = policy.strip_rust_comments(found.attrs)
    if not re.search(r"#\[\s*test\s*\]", attrs_code):
        return None, f"{ref}: `fn {name}` carries no `#[test]` attribute, so the gate never runs it"
    if re.search(r"#\[\s*ignore\b", attrs_code):
        return None, f"{ref}: `fn {name}` is `#[ignore]`d, so `just check` never runs it"
    return TestFn(path, name, found.attrs, found.body), None


def test_is_fresh(ctx: Context, test: TestFn) -> bool:
    """The test is new in the delta, or its body's code changed."""
    if ctx.base is None:
        return False
    base_text = ctx.base.read_text(test.path)
    if base_text is None:
        return True
    old = find_fn(base_text, test.name)
    if old is None:
        return True
    return delta_mod.semantic_text(old.body) != delta_mod.semantic_text(test.body)


def normalize(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip().lower()


def markdown_list(text: str, heading: str, lead: str | None = None) -> list[str] | None:
    """The first bullet list under `heading` (and after the line `lead`, if given).

    Items are returned with `(delivered: …)` records, trailing `;`/`.`, and
    surrounding whitespace removed. `None` when the heading or lead is absent.
    """
    lines = text.splitlines()
    try:
        i = next(k for k, line in enumerate(lines) if line.strip() == heading)
    except StopIteration:
        return None
    level = len(heading) - len(heading.lstrip("#"))
    j = i + 1
    if lead is not None:
        while j < len(lines) and lines[j].strip() != lead:
            if lines[j].startswith("#") and len(lines[j]) - len(lines[j].lstrip("#")) <= level:
                return None
            j += 1
        if j == len(lines):
            return None
        j += 1
    items: list[str] = []
    started = False
    while j < len(lines):
        line = lines[j]
        if line.startswith("- "):
            started = True
            item = re.sub(r"\s*\(delivered:[^)]*\)", "", line[2:]).strip()
            items.append(item.rstrip(";.").strip())
        elif started:
            break
        elif line.startswith("#"):
            break
        j += 1
    return items


def requirement_summaries(ctx: Context) -> dict[str, str]:
    doc = ctx.head.read_json(REQUIREMENTS_JSON)
    rows = doc.get("requirements", []) if isinstance(doc, dict) else doc or []
    return {r["id"]: r.get("summary", "") for r in rows if isinstance(r, dict) and "id" in r}


# ============================================================================
# Harness-owned review-record envelope
# ============================================================================

HARNESS_RULE = "review-record"
CHECK_PARSES = "review-record-parses"
CHECK_SECTIONS = "review-record-sections-closed"
CHECK_CHANGE = "review-record-change-table"
DELIVERY_RULE = "delivery-link"
CHECK_DELIVERY = "delivery-agrees-with-registry"
HARNESS_CHECKS = (CHECK_PARSES, CHECK_SECTIONS, CHECK_CHANGE, CHECK_DELIVERY)
HARNESS_RULES = {HARNESS_RULE: (CHECK_PARSES, CHECK_SECTIONS, CHECK_CHANGE), DELIVERY_RULE: (CHECK_DELIVERY,)}
CHANGE_KEYS = ("bone", "author", "summary")
BONE_RE = re.compile(r"bn-[0-9a-z]+")
MIN_SUMMARY = 24


def record_keys(sets: Sequence[ObligationSet]) -> dict[str, set[str]]:
    keys: dict[str, set[str]] = {"change": set(CHANGE_KEYS)}
    for s in sets:
        for section, names in s.record_keys.items():
            keys.setdefault(section, set()).update(names)
    return keys


def check_record_envelope(ctx: Context, sets: Sequence[ObligationSet]) -> list[Finding]:
    """Every review record at head: parses, closed sections and keys, a `[change]` table."""
    allowed = record_keys(sets)
    out: list[Finding] = []
    for rec in ctx.head_records():
        if rec.data is None:
            out.append(Finding(HARNESS_RULE, CHECK_PARSES, f"{rec.path} is not valid TOML: {rec.error}"))
            continue
        for section, value in sorted(rec.data.items()):
            if section not in allowed:
                out.append(
                    Finding(
                        HARNESS_RULE,
                        CHECK_SECTIONS,
                        f"{rec.path}: top-level `{section}` is not a section any obligation set declares "
                        f"({sorted(allowed)}); an unknown section is an error, not an extension",
                    )
                )
                continue
            if not isinstance(value, dict):
                out.append(Finding(HARNESS_RULE, CHECK_SECTIONS, f"{rec.path}: `{section}` is not a table"))
                continue
            for key in sorted(value):
                if key not in allowed[section]:
                    out.append(
                        Finding(
                            HARNESS_RULE,
                            CHECK_SECTIONS,
                            f"{rec.path}: `{section}.{key}` is not a key any obligation set declares for "
                            f"`[{section}]` ({sorted(allowed[section])})",
                        )
                    )
        change = rec.data.get("change")
        if not isinstance(change, dict):
            out.append(Finding(HARNESS_RULE, CHECK_CHANGE, f"{rec.path}: no `[change]` table"))
            continue
        bone = change.get("bone")
        if not isinstance(bone, str) or not BONE_RE.fullmatch(bone):
            out.append(Finding(HARNESS_RULE, CHECK_CHANGE, f"{rec.path}: `change.bone` {bone!r} is not a Bone id"))
        author = change.get("author")
        if not isinstance(author, str) or not author.strip():
            out.append(Finding(HARNESS_RULE, CHECK_CHANGE, f"{rec.path}: `change.author` is missing or empty"))
        summary = change.get("summary")
        if not isinstance(summary, str) or len(normalize(summary)) < MIN_SUMMARY:
            out.append(
                Finding(
                    HARNESS_RULE,
                    CHECK_CHANGE,
                    f"{rec.path}: `change.summary` is missing or shorter than {MIN_SUMMARY} characters",
                )
            )
    return out


def requirement_statuses(ctx: Context) -> dict[str, str]:
    doc = ctx.head.read_json(REQUIREMENTS_JSON)
    rows = doc.get("requirements", []) if isinstance(doc, dict) else doc or []
    return {r["id"]: r.get("status", "") for r in rows if isinstance(r, dict) and "id" in r}


def check_delivery(ctx: Context, sets: Sequence[ObligationSet]) -> list[Finding]:
    """The requirement id -> evidence link agrees in both directions.

    A set that enforces an id in full needs the source bullet's `(delivered: …)`
    record, which the generated registry reads as `satisfied`; an id the set
    declares `undelivered` must not carry one. Either disagreement means the
    registry and the evidence file tell different stories about the same id.
    """
    statuses = requirement_statuses(ctx)
    out: list[Finding] = []
    for s in sets:
        for rid in sorted(s.requirements):
            status = statuses.get(rid)
            if status is None:
                out.append(Finding(DELIVERY_RULE, CHECK_DELIVERY, f"{s.name}: {rid} is not in {REQUIREMENTS_JSON}"))
            elif rid in s.undelivered and status == "satisfied":
                out.append(Finding(
                    DELIVERY_RULE, CHECK_DELIVERY,
                    f"{s.name}: {rid} is `satisfied` in {REQUIREMENTS_JSON}, but the set declares it not delivered: "
                    f"{s.undelivered[rid]}",
                ))
            elif rid not in s.undelivered and status != "satisfied":
                out.append(Finding(
                    DELIVERY_RULE, CHECK_DELIVERY,
                    f"{s.name}: {rid} is enforced in full by {s.module_path}, but {REQUIREMENTS_JSON} has it "
                    f"`{status}`; annotate its source bullet `(delivered: bn-…)` and regenerate the registry "
                    "(`just traceability`), or list it in the set's `undelivered`",
                ))
    return out


def harness_findings(ctx: Context, sets: Sequence[ObligationSet]) -> list[Finding]:
    return [*check_record_envelope(ctx, sets), *check_delivery(ctx, sets)]


# ============================================================================
# Discovery
# ============================================================================


def discover(root: Path, only: str | None = None) -> tuple[list[ObligationSet], list[str]]:
    sets: list[ObligationSet] = []
    problems: list[str] = []
    directory = root / OBLIGATIONS_DIR
    if not directory.is_dir():
        return sets, [f"no obligation sets under {OBLIGATIONS_DIR}"]
    seen: dict[str, str] = {}
    owners: dict[str, str] = {}
    for path in sorted(directory.glob("*.py")):
        if path.name.startswith("_"):
            continue
        rel = path.relative_to(root).as_posix()
        spec = importlib.util.spec_from_file_location(f"continuum_obligation_{path.stem}", path)
        if spec is None or spec.loader is None:
            problems.append(f"{rel}: cannot be imported")
            continue
        module: ModuleType = importlib.util.module_from_spec(spec)
        try:
            spec.loader.exec_module(module)
        except Exception as exc:  # noqa: BLE001 — a broken set is a reported failure, not a crash
            problems.append(f"{rel}: import failed: {type(exc).__name__}: {exc}")
            continue
        s = getattr(module, "SET", None)
        if not isinstance(s, ObligationSet):
            problems.append(f"{rel}: defines no `SET: ObligationSet`")
            continue
        s = dataclasses.replace(s, module_path=rel)
        if s.name in seen:
            problems.append(f"{rel}: set name {s.name!r} is already used by {seen[s.name]}")
            continue
        seen[s.name] = rel
        for rid in s.requirements:
            if rid in owners:
                problems.append(f"{rel}: requirement {rid} is already owned by {owners[rid]}")
            owners[rid] = rel
        for rule in s.rules:
            if rule.mode not in (TREE, DELTA):
                problems.append(f"{rel}: rule {rule.name!r} has mode {rule.mode!r}")
            for rid in rule.requirements:
                if rid not in s.requirements:
                    problems.append(f"{rel}: rule {rule.name!r} names {rid}, which the set does not own")
        for rid in s.requirements:
            if not any(rid in r.requirements for r in s.rules):
                problems.append(f"{rel}: requirement {rid} has no rule")
        sets.append(s)
    if only is not None:
        sets = [s for s in sets if s.name == only]
        if not sets:
            problems.append(f"--set {only!r} names no discovered obligation set")
    return sets, problems


# ============================================================================
# Running one set
# ============================================================================


def in_force(s: ObligationSet, ctx: Context, regime: str = "auto") -> tuple[bool, str]:
    if regime == "in-force":
        return True, "`--regime in-force` declared every set's delta rules in force"
    if regime == "deferred":
        return False, "`--regime deferred` declared every set's delta rules not yet in force"
    if ctx.base is None:
        return True, "no base: delta rules not run"
    if ctx.base.read_text(s.module_path) is not None:
        return True, f"{s.module_path} exists at the base, so its delta rules are in force"
    return False, (
        f"{s.module_path} does not exist at the base: this branch was cut before the set landed, so its "
        "delta rules are reported, not enforced"
    )


def run_set(
    s: ObligationSet, ctx: Context, run_delta: bool, regime: str = "auto"
) -> tuple[list[Finding], list[str]]:
    """Findings from every rule of `s`, deferred where the regime says so.

    Returns (findings, problems). A problem is a rule that produced a finding
    outside its declared vocabulary — a harness contract violation.
    """
    findings: list[Finding] = []
    problems: list[str] = []
    active, reason = in_force(s, ctx, regime)
    for rule in s.rules:
        if rule.mode == DELTA and not run_delta:
            continue
        for f in rule.fn(ctx):
            if f.rule != rule.name or f.check not in rule.checks:
                problems.append(f"{s.name}: rule {rule.name!r} emitted undeclared {f.rule}/{f.check}")
                continue
            if rule.mode == DELTA and not active:
                f = Finding(f.rule, f.check, f.message, False, reason)
            findings.append(f)
    return delta_mod._sorted(findings), problems


# ============================================================================
# Fixtures
# ============================================================================

EXPECTATIONS = ("violation", "deferred", "clean")


@dataclass(frozen=True)
class Fixture:
    fid: str
    set_name: str
    requirement: str
    rule: str
    check: str
    expect: str
    description: str
    base: dict[str, str | None]
    head: dict[str, str | None]

    def context(self, root: Path) -> Context:
        def view(side: dict[str, str | None]) -> policy.Tree:
            overlay = {k: v for k, v in side.items() if v is not None}
            removed = [k for k, v in side.items() if v is None]
            return policy.Tree(root, overlay, removed)

        head, base = view(self.head), view(self.base)
        paths = set(self.head) | set(self.base)
        changed = tuple(sorted(p for p in paths if head.read_text(p) != base.read_text(p)))
        return Context(head, Reader(base.read_text), changed, "the base fixture", "the head fixture")


def _side(root: Path, directory: Path, spec: dict, side: str, problems: list[str]) -> dict[str, str | None]:
    """A side's files: payload files, `null` (absent), and anchored substitutions."""
    real = policy.Tree(root)
    out: dict[str, str | None] = {}
    for target, payload in sorted((spec.get(side) or {}).items()):
        if payload is None:
            out[target] = None
            continue
        p = directory / payload
        if not p.is_file():
            problems.append(f"{directory.name}: {side} payload {payload!r} is missing")
            continue
        out[target] = p.read_text(encoding="utf-8")
    for target, edits in sorted((spec.get(f"{side}_substitute") or {}).items()):
        text = out.get(target) if target in out else real.read_text(target)
        if text is None:
            problems.append(f"{directory.name}: {side}_substitute target {target!r} does not exist")
            continue
        for edit in edits:
            count = text.count(edit["find"])
            if count != 1:
                problems.append(
                    f"{directory.name}: {side}_substitute anchor for {target!r} matches {count} times, "
                    f"expected 1 — the fixture is stale: {edit['find'][:60]!r}"
                )
                break
            text = text.replace(edit["find"], edit["replace"], 1)
        out[target] = text
    return out


def load_fixtures(root: Path, s: ObligationSet) -> tuple[list[Fixture], list[str]]:
    fixtures: list[Fixture] = []
    problems: list[str] = []
    directory = root / FIXTURE_ROOT / s.name
    if not directory.is_dir():
        return fixtures, [f"{s.name}: no fixtures under {FIXTURE_ROOT}/{s.name}"]
    rules = {r.name: r for r in s.rules}
    for d in sorted(p for p in directory.iterdir() if p.is_dir()):
        spec_path = d / "fixture.json"
        if not spec_path.is_file():
            problems.append(f"{d.name}: no fixture.json")
            continue
        try:
            spec = json.loads(spec_path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            problems.append(f"{d.name}: invalid JSON ({exc})")
            continue
        before = len(problems)
        base = _side(root, d, spec, "base", problems)
        head = _side(root, d, spec, "head", problems)
        rule_name, check = spec.get("rule"), spec.get("check")
        if spec.get("expect") not in EXPECTATIONS:
            problems.append(f"{d.name}: expect {spec.get('expect')!r} is not one of {list(EXPECTATIONS)}")
        if rule_name in HARNESS_RULES:
            if check not in HARNESS_RULES[rule_name]:
                problems.append(f"{d.name}: {check!r} is not a harness check")
        elif rule_name not in rules:
            problems.append(f"{d.name}: unknown rule {rule_name!r}")
        elif check not in rules[rule_name].checks:
            problems.append(f"{d.name}: check {check!r} is not a sub-check of {rule_name}")
        elif spec.get("requirement") not in rules[rule_name].requirements:
            problems.append(f"{d.name}: rule {rule_name!r} does not bind {spec.get('requirement')}")
        if not base and not head:
            problems.append(f"{d.name}: neither side declares a file; the pair has no delta")
        if len(problems) != before:
            continue
        fixtures.append(
            Fixture(
                fid=spec.get("id", d.name),
                set_name=s.name,
                requirement=spec.get("requirement", ""),
                rule=rule_name,
                check=check,
                expect=spec["expect"],
                description=spec.get("description", ""),
                base=base,
                head=head,
            )
        )
    return fixtures, problems


def run_all(ctx: Context, s: ObligationSet, sets: Sequence[ObligationSet]) -> tuple[list[Finding], list[str]]:
    findings, problems = run_set(s, ctx, run_delta=ctx.has_delta)
    return delta_mod._sorted([*findings, *harness_findings(ctx, sets)]), problems


def self_test_set(root: Path, s: ObligationSet, sets: Sequence[ObligationSet]) -> tuple[bool, dict[str, object]]:
    fixtures, problems = load_fixtures(root, s)
    failures: list[str] = list(problems)
    covered: dict[str, list[str]] = {}
    clean: list[str] = []
    deferred: list[str] = []
    by_requirement: dict[str, list[str]] = {rid: [] for rid in s.requirements}

    for fx in sorted(fixtures, key=lambda f: f.fid):
        findings, contract = run_all(fx.context(root), s, sets)
        failures.extend(f"{fx.fid}: {p}" for p in contract)
        enforced = [f for f in findings if f.enforced]
        named = [f for f in findings if f.check == fx.check]
        if fx.expect == "violation":
            if not any(f.enforced for f in named):
                failures.append(
                    f"{fx.fid}: expected an enforced {fx.check!r} finding; got "
                    f"{[f.render() for f in findings] or 'nothing'}"
                )
                continue
            covered.setdefault(fx.check, []).append(fx.fid)
            if fx.requirement in by_requirement:
                by_requirement[fx.requirement].append(fx.fid)
        elif fx.expect == "deferred":
            if not any(not f.enforced for f in named) or enforced:
                failures.append(
                    f"{fx.fid}: expected only deferred findings including {fx.check!r}; got "
                    f"{[f.render() for f in findings] or 'nothing'}"
                )
                continue
            deferred.append(fx.fid)
        else:
            if enforced:
                failures.append(f"{fx.fid}: must not trip anything, but tripped {[f.render() for f in enforced]}")
                continue
            clean.append(fx.fid)

    for rule in s.rules:
        for check in rule.checks:
            if not covered.get(check):
                failures.append(f"{s.name}: check {check!r} ({rule.name}) has no violating fixture")
    for rid, fids in by_requirement.items():
        if not fids:
            failures.append(f"{s.name}: requirement {rid} has no violating fixture of its own")

    report = {
        "status": "fail" if failures else "pass",
        "fixtures": len(fixtures),
        "violating_by_check": {k: sorted(v) for k, v in sorted(covered.items())},
        "violating_by_requirement": {k: sorted(v) for k, v in sorted(by_requirement.items())},
        "clean": sorted(clean),
        "deferred": sorted(deferred),
        "failures": sorted(failures),
    }
    return not failures, report


def self_test_harness(root: Path, sets: Sequence[ObligationSet]) -> tuple[bool, dict[str, object]]:
    """The envelope checks, proven by `fixtures/obligations/_harness/`."""
    s = ObligationSet(
        name="_harness",
        title="review-record envelope",
        source=f"{GOVERNANCE}/check_obligations.py",
        requirements={},
        rules=(),
        evidence="",
    )
    fixtures, problems = load_fixtures(root, s)
    failures = list(problems)
    covered: dict[str, list[str]] = {}
    for fx in fixtures:
        found = harness_findings(fx.context(root), sets)
        if fx.expect == "violation":
            if not any(f.check == fx.check for f in found):
                failures.append(f"{fx.fid}: expected {fx.check!r}; got {[f.render() for f in found] or 'nothing'}")
                continue
            covered.setdefault(fx.check, []).append(fx.fid)
        elif found:
            failures.append(f"{fx.fid}: must not trip anything, but tripped {[f.render() for f in found]}")
    for check in HARNESS_CHECKS:
        if not covered.get(check):
            failures.append(f"harness check {check!r} has no violating fixture")
    return not failures, {
        "status": "fail" if failures else "pass",
        "fixtures": len(fixtures),
        "violating_by_check": {k: sorted(v) for k, v in sorted(covered.items())},
        "failures": sorted(failures),
    }


# ============================================================================
# Evidence
# ============================================================================


def tree_results(s: ObligationSet, ctx: Context) -> dict[str, list[str]]:
    out: dict[str, list[str]] = {}
    for rule in s.rules:
        if rule.mode == TREE:
            out[rule.name] = [f.render() for f in rule.fn(ctx)]
    return out


def build_evidence(
    s: ObligationSet, st: dict[str, object], harness_st: dict[str, object], tree: dict[str, list[str]],
    envelope: list[str],
) -> dict:
    """A function of the repository alone: no clock, commit id, or absolute path."""
    by_req = st.get("violating_by_requirement", {})
    by_check = st.get("violating_by_check", {})
    requirements: dict[str, object] = {}
    for rid, bullet in sorted(s.requirements.items()):
        rules = [r for r in s.rules if rid in r.requirements]
        requirements[rid] = {
            "source_bullet": bullet,
            "source": s.source,
            "rules": [
                {
                    "rule": r.name,
                    "mode": r.mode,
                    "checks": list(r.checks),
                    "shared_with": sorted(x for x in r.requirements if x != rid),
                    "enforces": r.enforces,
                    "boundary": r.boundary,
                    "fixtures_proven_caught": {c: list(by_check.get(c, [])) for c in r.checks},
                    "real_run": (
                        {"result": "fail" if tree.get(r.name) else "pass", "violations": tree.get(r.name, [])}
                        if r.mode == TREE
                        else "delta rule: the verdict is a function of two revisions and is reported to "
                        "stdout and CI, never recorded here"
                    ),
                }
                for r in rules
            ],
            "own_violating_fixtures": list(by_req.get(rid, [])),
            "delivery": (
                {"status": "not-delivered", "reason": s.undelivered[rid]}
                if rid in s.undelivered
                else {
                    "status": "delivered",
                    "link": f"the source bullet carries `(delivered: …)`; {REQUIREMENTS_JSON} reads it `satisfied`; "
                    f"`{CHECK_DELIVERY}` fails if the two disagree",
                }
            ),
        }
    failed = st.get("status") != "pass" or harness_st.get("status") != "pass" or any(tree.values()) or envelope
    return {
        "artifact": f"continuum.governance.evidence/{s.name}",
        "title": s.title,
        "produced_by": f"{GOVERNANCE}/check_obligations.py ({s.module_path})",
        "reproduce": f"python3 {GOVERNANCE}/check_obligations.py --set {s.name} --evidence",
        "authority": {"source": s.source, "requirement_ids": REQUIREMENTS_JSON},
        "determinism": (
            "No timestamp, commit id, or absolute path: an unchanged tree reproduces this file "
            "byte-for-byte. Delta-rule verdicts are functions of history and are not recorded."
        ),
        "regime": (
            f"The delta rules are in force for a delta whose base contains {s.module_path}; for an older "
            "base their findings are deferred (printed, reported, not failures)."
        ),
        "requirements": requirements,
        "harness_at_head": {
            "checks": "review-record envelope over every record at head; delivery link for every discovered set",
            "result": "fail" if envelope else "pass",
            "violations": envelope,
        },
        "notes": list(s.notes),
        "self_test": st,
        "harness_self_test": harness_st,
        "status": "fail" if failed else "pass",
    }


# ============================================================================
# Entry point
# ============================================================================


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--base", metavar="REV", default=None, help="base revision (default: merge base with main)")
    parser.add_argument("--require-base", action="store_true", help="fail when no base is resolvable (CI)")
    parser.add_argument("--self-test", action="store_true", help="replay every fixture; fail if any goes uncaught")
    parser.add_argument("--evidence", action="store_true", help="self-test, then write each set's evidence file")
    parser.add_argument(
        "--regime",
        choices=("auto", "in-force", "deferred"),
        default="auto",
        help="override the per-set regime (default: in force when the set's module exists at the base)",
    )
    parser.add_argument("--set", dest="only", default=None, metavar="NAME", help="run one obligation set")
    parser.add_argument("--root", default=str(ROOT), help="repository root")
    args = parser.parse_args(argv)
    root = Path(args.root).resolve()

    sets, problems = discover(root, args.only)
    all_sets, _ = discover(root)
    if problems:
        for p in problems:
            print(f"obligations: {p}", file=sys.stderr)
        return 1

    if args.self_test or args.evidence:
        ok = True
        harness_ok, harness_st = self_test_harness(root, all_sets)
        ok &= harness_ok
        reports: dict[str, object] = {"_harness": harness_st}
        head_ctx = Context(policy.Tree(root), None, None)
        envelope = [f.render() for f in harness_findings(head_ctx, all_sets)]
        for s in sets:
            set_ok, st = self_test_set(root, s, all_sets)
            ok &= set_ok
            reports[s.name] = st
            if args.evidence:
                evidence = build_evidence(s, st, harness_st, tree_results(s, head_ctx), envelope)
                path = root / s.evidence
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        if args.self_test:
            print(json.dumps({"status": "pass" if ok else "fail", "sets": reports}, indent=2, sort_keys=True))
        if not ok or args.self_test:
            return 0 if ok else 1

    # The real run: tree rules and the envelope always; delta rules when a base resolves.
    git = delta_mod.Git(root)
    resolution = delta_mod.resolve_base(git, explicit=args.base)
    delta = None
    skip_reason = None
    if resolution.resolved:
        delta, skip_reason = delta_mod.git_delta(git, root, resolution.rev, None)
    else:
        skip_reason = resolution.reason
    head = policy.Tree(root)
    if delta is not None:
        ctx = Context(head, Reader(delta.read_base), delta.changed, delta.base_label, delta.head_label)
    else:
        ctx = Context(head, None, None)

    all_findings: list[Finding] = []
    contract: list[str] = []
    for s in sets:
        found, prob = run_set(s, ctx, run_delta=ctx.has_delta, regime=args.regime)
        all_findings.extend(found)
        contract.extend(prob)
    all_findings.extend(harness_findings(ctx, all_sets))
    all_findings = delta_mod._sorted(all_findings)

    for f in all_findings:
        print(f.render(), file=sys.stderr)
    for p in contract:
        print(f"obligations: {p}", file=sys.stderr)
    enforced = [f for f in all_findings if f.enforced]
    if resolution.fatal:
        status = "fail"
    elif enforced or contract:
        status = "fail"
    elif delta is None:
        status = "skip"
    else:
        status = "pass"
    report = {
        "status": status,
        "sets": [s.name for s in sets],
        "base": resolution.rev[:12] if resolution.rev else None,
        "base_resolution": resolution.how,
        "delta_rules": (
            "run; each set's regime decides enforced or deferred"
            if delta is not None
            else f"NOT run: {skip_reason}"
        ),
        "semantic_changes": ctx.semantic_changes() if ctx.has_delta else None,
        "violations": [f.render() for f in enforced],
        "deferred": [f.render() for f in all_findings if not f.enforced],
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    if status == "skip":
        print(f"obligations: delta rules skipped: {skip_reason}", file=sys.stderr)
    return delta_mod.exit_code(status, args.require_base)


if __name__ == "__main__":
    # Obligation sets `import check_obligations`; make that name this module, so
    # the `ObligationSet` they build is the class discovery checks against.
    sys.modules.setdefault("check_obligations", sys.modules[__name__])
    sys.exit(main())
