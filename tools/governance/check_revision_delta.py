#!/usr/bin/env python3
"""Enforce GOV-1-08/09 across two revisions — the delta half of the GOV §1 constitution.

Source of truth:

- `notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md` §1 "Repository constitution",
  semantic policy: "semantic changes require ADR" (`GOV-1-08`) and "every breaking
  change increments semantic epoch" (`GOV-1-09`);
- `notes/plan/schemas/README.md`, the normative home of the schema identity and
  epoch convention ("What advances a schema epoch", the PR 5 freeze, and the
  `Preserved | Revalidate | Incompatible` compatibility statement).

Why this file exists
--------------------

`tools/governance/check_code_policy.py` enforces all twelve GOV §1 obligations
against **one** revision. Two of them are not one-revision properties at all:
"semantic changes require ADR" and "every breaking change increments semantic
epoch" are statements about a *change*, and no program reading a single tree can
see that a change happened. Those two rules therefore enforced an artifact-level
proxy and said so in `evidence/gov-1.json`'s `boundary` field.

This script is the missing half: it reads **two** revisions — a base (default:
the merge base with the trunk) and a head (default: the working tree) — and
enforces the two rules on the delta between them. The single-revision rules keep
running; nothing here replaces them. Together they are the obligation:
`check_code_policy.py` proves the *state* is coherent, this proves the *change*
was authorized.

The two delta rules
-------------------

    semantic-change-adr-delta   (GOV-1-08)
        semantic-change-cites-a-decision-record
            A semantic-tier source file whose code changed between base and
            head must, at head, cite a numbered decision record (`ADR-NNNN` or
            `RFC NNNN`) — unless the delta itself adds or modifies a decision
            record, which is the other form the constitution accepts: the
            change ships with the decision that authorized it.
        added-semantic-source-cites-a-decision-record
            A semantic-tier source file *added* by the delta must cite a
            numbered decision record at head. A new semantic module names the
            record that authorized it; an unrelated ADR edit elsewhere in the
            delta does not excuse it.

    epoch-discipline-delta      (GOV-1-09)
        schema-bytes-changed-without-epoch-advance
            A schema document whose bytes changed between base and head while
            its `schema_epoch` stayed put. Past the PR 5 freeze this is the
            mechanical form of "a breaking change that did not increment the
            epoch"; before the freeze `schemas/README.md` explicitly permits
            in-place edits at epoch 1, so the finding is *deferred* — recorded
            and printed, never silently dropped (see "Freeze regime" below).
        published-schema-document-deleted
            A schema document present at base and absent at head. Past the
            freeze a published document is immutable, and deleting one
            invalidates every instance pinned to it. Freeze-gated like the
            check above.
        schema-epoch-never-retreats
            A schema whose `schema_epoch` went *down*. An epoch is a
            monotone breaking-change counter (schemas/README.md, plan §4.6);
            this is enforced in every regime, before and after the freeze.
        epoch-advance-publishes-a-compatibility-statement
            A schema whose epoch advanced, with no markdown in the same delta
            naming that artifact class together with one of the typed verdicts
            `Preserved | Revalidate | Incompatible`. plan §4.6 requires the
            statement to be published *before* the advance is applied, and
            there is no pre-freeze exemption for an actual advance — only for
            in-place edits at epoch 1.

"Semantic tier" is not invented here: it is `check_code_policy.SEMANTIC_CORE`,
the same 27-crate partition GOV-1-03/04/05 are enforced over, imported rather
than copied so the two halves cannot drift apart. A change is "code" when the
comment-stripped, whitespace-normalized text differs — a doc-comment or
formatting edit is not a semantic change and is not reported as one.

Base resolution and explicit degradation
----------------------------------------

The base rev is an explicit input with a default, never an ambient guess:

1. `--base <rev>` if given. An explicit base that does not resolve is a hard
   failure — a wrong input must not degrade into a pass.
2. otherwise `git merge-base HEAD main`, falling back to `origin/main`.

When no base is resolvable — outside a work tree, `git` unavailable, unborn
HEAD, no trunk ref, or a shallow checkout with no common ancestor — the runner
**skips**: it prints the reason on stderr and reports `"status": "skip"` with
the resolution trail, and the delta rules are reported as *not enforced*. It
never reports a pass it did not earn. The skip exits 0 so that a fresh clone or
a detached checkout does not fail the local gate; CI passes `--require-base`,
which turns the same skip into exit 1. Both exit paths are unit-tested by
`--self-test` against a stubbed git, so the degradation path is covered without
requiring any particular repository state.

Freeze regime
-------------

`schemas/README.md` states its own regime: "Until PR 5 freezes the interface
(plan §25), epoch 1 is a draft: the documents in this directory are edited in
place and no compatibility statement is owed for those edits. From the freeze
onward a published document is immutable." The two freeze-gated checks read
that clause at head and report accordingly:

- clause present  → pre-freeze. In-place schema edits are *deferred* findings:
  printed, counted, and written into the report, but not failures.
- clause absent   → the freeze is in force. Enforced.

Fail-closed in both directions of doubt: an absent or reworded README means
enforced. `--freeze in-force|pre-freeze` overrides the reading explicitly.

Self-test
---------

`--self-test` replays violating and passing fixture **pairs** —
`fixtures/delta/<id>/`, each a `base` map and a `head` map of paths to inert
payload files — through the same rule functions the real run uses, and fails if
a fixture that must trip does not, if a passing pair trips anyway, or if any
sub-check has no violating fixture (a check that cannot be shown to fire could
pass vacuously). It also drives nine base-resolution scenarios through a stubbed
git. Fixture pairs are hermetic: unlike `check_code_policy.py`'s overlays they
do not read the repository, because a delta rule's whole input *is* its pair.
They are still held to the tree in the one way that can rot — every
`crates/<name>/` path a fixture names must still be a crate
`check_code_policy.py` classifies, so a renamed or reclassified crate fails the
self-test loudly instead of silently testing nothing.

Determinism (INV-005). The base rev is an explicit input with a documented
default; nothing here reads a clock, an environment variable, or the network.
The only subprocess is `git`, invoked with `--no-renames` (rename detection is a
heuristic) and `core.quotepath=false` (path spelling must not depend on
configuration). The committed evidence artifact is deliberately a function of
the *repository* and not of git history — see `build_evidence`.

Stdlib only. Exit 0 when both rules hold on the delta (or the run skipped
without `--require-base`), 1 otherwise.

Usage:

    python3 tools/governance/check_revision_delta.py --self-test
    python3 tools/governance/check_revision_delta.py
    python3 tools/governance/check_revision_delta.py --base origin/main --require-base
    python3 tools/governance/check_revision_delta.py --evidence tools/governance/evidence/gov-1-delta.json
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections.abc import Callable, Sequence
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GOVERNANCE = "tools/governance"
FIXTURE_DIR = f"{GOVERNANCE}/fixtures/delta"
EVIDENCE = f"{GOVERNANCE}/evidence/gov-1-delta.json"

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_code_policy as policy  # noqa: E402  (path-relative sibling import, stdlib-only)

CONSTITUTION = policy.CONSTITUTION
SCHEMA_DIR = policy.SCHEMA_DIR
ADR_DIR = policy.ADR_DIR
RFC_DIR = policy.RFC_DIR
SCHEMA_README = f"{SCHEMA_DIR}/README.md"

TRUNK_REFS = ("main", "origin/main")

RULE_08 = "semantic-change-adr-delta"
RULE_09 = "epoch-discipline-delta"

CHECK_CITES = "semantic-change-cites-a-decision-record"
CHECK_ADDED = "added-semantic-source-cites-a-decision-record"
CHECK_INPLACE = "schema-bytes-changed-without-epoch-advance"
CHECK_DELETED = "published-schema-document-deleted"
CHECK_RETREAT = "schema-epoch-never-retreats"
CHECK_STATEMENT = "epoch-advance-publishes-a-compatibility-statement"

CHECKS: dict[str, tuple[str, ...]] = {
    RULE_08: (CHECK_CITES, CHECK_ADDED),
    RULE_09: (CHECK_INPLACE, CHECK_DELETED, CHECK_RETREAT, CHECK_STATEMENT),
}

# `schemas/README.md` declares its own regime; this is the clause that says the
# freeze has not happened yet. Its absence is read as "the freeze is in force".
FREEZE_DRAFT_CLAUSE = "Until PR 5 freezes the interface"
COMPATIBILITY_VERDICTS = ("Preserved", "Revalidate", "Incompatible")

SEMANTIC_SOURCE_RE = re.compile(r"crates/([^/]+)/src/.+\.rs")
DECISION_RECORD_RE = re.compile(rf"(?:{re.escape(ADR_DIR)}|{re.escape(RFC_DIR)})/\d{{4}}[^/]*\.md")
SCHEMA_ID_EPOCH_RE = re.compile(r"/schema/v(\d+)/")


# ============================================================================
# Findings
#
# A delta rule has three outcomes, not two. `enforced` separates a violation
# from a finding the current regime defers (today: an in-place schema edit
# before the PR 5 freeze, which `schemas/README.md` explicitly permits). A
# deferred finding is still printed and still written into the report — the one
# outcome this file must never produce is a silent pass.
# ============================================================================


@dataclass(frozen=True)
class Finding:
    rule: str
    check: str
    message: str
    enforced: bool = True
    deferred_reason: str | None = None

    @property
    def sort_key(self) -> tuple[str, str, str]:
        return (self.rule, self.check, self.message)

    def render(self) -> str:
        prefix = "" if self.enforced else "DEFERRED "
        suffix = "" if self.enforced else f" [not enforced: {self.deferred_reason}]"
        return f"{prefix}[{self.rule}/{self.check}] {self.message}{suffix}"


def _sorted(findings: Sequence[Finding]) -> list[Finding]:
    return sorted(findings, key=lambda f: f.sort_key)


# ============================================================================
# The delta — the only thing a rule may read
#
# Two readers and a changed-path set. The real run backs them with `git`; a
# fixture backs them with two dictionaries. The rule functions cannot tell the
# difference, so the code path a fixture proves is the code path CI runs.
# ============================================================================


@dataclass(frozen=True)
class Delta:
    changed: tuple[str, ...]
    read_base: Callable[[str], str | None]
    read_head: Callable[[str], str | None]
    base_label: str
    head_label: str

    def changed_matching(self, predicate: Callable[[str], bool]) -> list[str]:
        return sorted(p for p in self.changed if predicate(p))


def is_semantic_source(rel: str) -> bool:
    """A source file of a crate in `check_code_policy.SEMANTIC_CORE`.

    The tier partition is imported, never re-declared: GOV-1-03's
    `tier-partition-covers-plan-20` already fails if that table and plan §20
    disagree, so this rule inherits a classification that cannot drift.
    """
    match = SEMANTIC_SOURCE_RE.fullmatch(rel)
    return bool(match) and match.group(1) in policy.SEMANTIC_CORE


def is_schema_document(rel: str) -> bool:
    return rel.startswith(f"{SCHEMA_DIR}/") and rel.endswith(".schema.json") and "/examples/" not in rel


def is_decision_record(rel: str) -> bool:
    return bool(DECISION_RECORD_RE.fullmatch(rel))


def is_plan_markdown(rel: str) -> bool:
    return rel.startswith("notes/plan/") and rel.endswith(".md")


def schema_class(rel: str) -> str:
    """`notes/plan/schemas/context-pack.schema.json` -> `context-pack`."""
    return Path(rel).name[: -len(".schema.json")]


def semantic_text(source: str) -> str:
    """Code-bearing text: comments removed, whitespace normalized.

    A doc-comment rewrite or a `cargo fmt` pass is not a semantic change and
    must not be reported as one. `strip_rust_comments` is
    `check_code_policy.py`'s, so "what counts as a comment" has one definition
    in the workspace.
    """
    stripped = policy.strip_rust_comments(source)
    lines = [re.sub(r"[ \t]+", " ", line).strip() for line in stripped.splitlines()]
    return "\n".join(line for line in lines if line)


def citations(source: str) -> list[str]:
    adrs = [f"ADR-{n}" for n in sorted(set(policy.ADR_CITATION.findall(source)))]
    rfcs = [f"RFC {n}" for n in sorted(set(policy.RFC_CITATION.findall(source)))]
    return adrs + rfcs


def schema_epoch(text: str | None) -> int | None:
    """The document's epoch, read from `schema_epoch` and cross-read from `$id`.

    `check_code_policy.py`'s `schema-id-and-epoch-agree` already fails when the
    two disagree on one revision, so the delta rule reads `schema_epoch` first
    and only falls back to the `$id` when the header is absent or not an
    integer.
    """
    if text is None:
        return None
    try:
        doc = json.loads(text)
    except json.JSONDecodeError:
        return None
    if not isinstance(doc, dict):
        return None
    epoch = doc.get("schema_epoch")
    if isinstance(epoch, int) and not isinstance(epoch, bool):
        return epoch
    doc_id = doc.get("$id")
    match = SCHEMA_ID_EPOCH_RE.search(doc_id) if isinstance(doc_id, str) else None
    return int(match.group(1)) if match else None


# ============================================================================
# Freeze regime
# ============================================================================


@dataclass(frozen=True)
class Freeze:
    in_force: bool
    reason: str
    source: str

    def defer(self, finding: Finding) -> Finding:
        if self.in_force:
            return finding
        return Finding(finding.rule, finding.check, finding.message, False, self.reason)


def resolve_freeze(delta: Delta, override: str = "auto") -> Freeze:
    if override == "in-force":
        return Freeze(True, "the PR 5 interface freeze was declared in force by `--freeze in-force`", "flag")
    if override == "pre-freeze":
        return Freeze(
            False,
            "the PR 5 interface freeze was declared not-yet-in-force by `--freeze pre-freeze`",
            "flag",
        )
    readme = delta.read_head(SCHEMA_README)
    if readme is None:
        return Freeze(
            True,
            f"{SCHEMA_README} is absent at head, so the pre-freeze draft clause cannot be read; "
            "the freeze is assumed in force",
            SCHEMA_README,
        )
    if FREEZE_DRAFT_CLAUSE in readme:
        return Freeze(
            False,
            f"{SCHEMA_README} still carries the pre-freeze draft clause "
            f"({FREEZE_DRAFT_CLAUSE!r}): epoch 1 documents are edited in place and no compatibility "
            "statement is owed for those edits until PR 5 freezes the interface",
            SCHEMA_README,
        )
    return Freeze(
        True,
        f"{SCHEMA_README} no longer carries the pre-freeze draft clause "
        f"({FREEZE_DRAFT_CLAUSE!r}), so a published document is immutable",
        SCHEMA_README,
    )


# ============================================================================
# GOV-1-08 — a semantic change carries the decision record that authorized it
# ============================================================================


def rule_semantic_change_adr_delta(delta: Delta, freeze: Freeze) -> tuple[list[Finding], list[dict]]:
    """`freeze` is unused: GOV-1-08 has no regime gate.

    "Semantic changes require ADR" is in force in every revision of this
    repository. Only the schema-epoch rule below has a clause of its own that
    says when it starts applying, and it reads it rather than assuming it.
    """
    out: list[Finding] = []
    waivers: list[dict] = []

    # A decision record the delta *adds or modifies* is the second form the
    # constitution accepts: the change ships with the decision. A record the
    # delta deletes is not one.
    records = [p for p in delta.changed_matching(is_decision_record) if delta.read_head(p) is not None]

    for rel in delta.changed_matching(is_semantic_source):
        head = delta.read_head(rel)
        if head is None:
            # A deleted file cites nothing and needs nothing; the crate-level
            # single-revision rule still covers what remains.
            continue
        base = delta.read_base(rel)
        added = base is None
        if not added and semantic_text(base) == semantic_text(head):
            continue

        # The inline waiver is `check_code_policy.py`'s, with one deliberate
        # difference in scope: there a waiver covers its own line and the next,
        # because the unit of a source scan is a line; here the unit is the
        # changed *file*, so a waiver for this check covers the file it is
        # written in. The 12-character reason floor is inherited unchanged, and
        # every granted waiver is listed in the run's `waivers_granted`.
        covered, granted, _ = policy.collect_waivers(rel, head)
        cited = citations(head)

        if added:
            if cited:
                continue
            if covered.get(CHECK_ADDED):
                waivers.extend(_waiver_records(granted, CHECK_ADDED))
                continue
            out.append(
                Finding(
                    RULE_08,
                    CHECK_ADDED,
                    f"{rel} is a new semantic-tier source file and cites no `ADR-NNNN` or `RFC NNNN`; "
                    "a new semantic module names the numbered decision that authorized it "
                    f"({CONSTITUTION} §1, \"semantic changes require ADR\")",
                )
            )
            continue

        if cited:
            continue
        if records:
            continue
        if covered.get(CHECK_CITES):
            waivers.extend(_waiver_records(granted, CHECK_CITES))
            continue
        out.append(
            Finding(
                RULE_08,
                CHECK_CITES,
                f"{rel} changed between {delta.base_label} and {delta.head_label}, cites no "
                "`ADR-NNNN` or `RFC NNNN`, and the delta adds or modifies no decision record under "
                f"{ADR_DIR}/ or {RFC_DIR}/; a semantic change carries the decision that authorized it "
                f"({CONSTITUTION} §1). Cite the governing record in the file, land the ADR with the "
                f"change, or waive the line with `// continuum:allow({CHECK_CITES}): <reason>`",
            )
        )

    return _sorted(out), waivers


def _waiver_records(granted: list, check: str) -> list[dict]:
    return [
        {"path": w.path, "line": w.line, "check": w.check, "reason": w.reason}
        for w in granted
        if w.check == check
    ]


# ============================================================================
# GOV-1-09 — a schema that changed carries its epoch advance
# ============================================================================


def rule_epoch_discipline_delta(delta: Delta, freeze: Freeze) -> tuple[list[Finding], list[dict]]:
    out: list[Finding] = []

    statement_texts = {
        rel: delta.read_head(rel) or ""
        for rel in delta.changed_matching(is_plan_markdown)
        if delta.read_head(rel) is not None
    }

    for rel in delta.changed_matching(is_schema_document):
        base_text = delta.read_base(rel)
        head_text = delta.read_head(rel)
        if base_text is None:
            # A brand-new schema document mints its own first epoch; there is
            # no earlier contract for it to have broken.
            continue
        if head_text is None:
            out.append(
                freeze.defer(
                    Finding(
                        RULE_09,
                        CHECK_DELETED,
                        f"{rel} exists at {delta.base_label} and is gone at {delta.head_label}; past the "
                        "PR 5 freeze a published schema document is immutable and every instance pinned "
                        f"to it becomes unresolvable ({SCHEMA_README}, \"What advances a schema epoch\")",
                    )
                )
            )
            continue

        base_epoch = schema_epoch(base_text)
        head_epoch = schema_epoch(head_text)

        if base_epoch is not None and head_epoch is not None and head_epoch < base_epoch:
            out.append(
                Finding(
                    RULE_09,
                    CHECK_RETREAT,
                    f"{rel} moved from schema_epoch {base_epoch} to {head_epoch}; the epoch is a monotone "
                    "breaking-change counter and a retreat re-uses an identity that already named a "
                    f"different contract ({SCHEMA_README}, \"Identity\"; plan §4.6)",
                )
            )
            continue

        if base_epoch is None or head_epoch is None:
            # Same finding as an in-place edit, with less information: the
            # document changed and cannot be shown to have advanced. It is
            # freeze-gated for the same reason — the draft clause permits
            # in-place edits, and a retired `$id` form with no `schema_epoch`
            # header at all is the most in-place edit there is.
            out.append(
                freeze.defer(
                    Finding(
                        RULE_09,
                        CHECK_INPLACE,
                        f"{rel} changed between {delta.base_label} and {delta.head_label} and its epoch "
                        f"could not be read on both sides (base={base_epoch!r}, head={head_epoch!r}); a "
                        "schema whose epoch is unreadable cannot be shown to have advanced",
                    )
                )
            )
            continue

        if head_epoch > base_epoch:
            klass = schema_class(rel)
            where = _compatibility_statement(statement_texts, klass)
            if where is None:
                out.append(
                    Finding(
                        RULE_09,
                        CHECK_STATEMENT,
                        f"{rel} advances schema_epoch {base_epoch} -> {head_epoch}, but no markdown "
                        f"changed in this delta names the `{klass}` class together with one of the typed "
                        f"verdicts {list(COMPATIBILITY_VERDICTS)}; plan §4.6 requires the per-artifact-class "
                        "compatibility statement to be published *before* the advance is applied "
                        f"({SCHEMA_README}, \"Schema epochs and compatibility\")",
                    )
                )
            continue

        out.append(
            freeze.defer(
                Finding(
                    RULE_09,
                    CHECK_INPLACE,
                    f"{rel} changed between {delta.base_label} and {delta.head_label} with schema_epoch "
                    f"{head_epoch} unchanged; past the PR 5 freeze a published document is immutable, so a "
                    "change to it either advances the epoch or is a breaking change that did not "
                    f"({SCHEMA_README}, \"What advances a schema epoch\"; {CONSTITUTION} §1)",
                )
            )
        )

    return _sorted(out), []


def _compatibility_statement(texts: dict[str, str], klass: str) -> str | None:
    """The changed markdown that publishes this class's compatibility verdict.

    Coarse on purpose: it asks that some prose changed in the same delta names
    the artifact class and one of the three typed verdicts. Whether the verdict
    is the *right* one is a review judgement, and is stated as a boundary rather
    than pretended away.
    """
    for rel in sorted(texts):
        text = texts[rel]
        names_class = klass in text or f"https://continuum.dev/schema/{klass}.json" in text
        if names_class and any(v in text for v in COMPATIBILITY_VERDICTS):
            return rel
    return None


# ============================================================================
# Runner
# ============================================================================

RULES: dict[str, Callable[[Delta, Freeze], tuple[list[Finding], list[dict]]]] = {
    RULE_08: rule_semantic_change_adr_delta,
    RULE_09: rule_epoch_discipline_delta,
}
REQUIREMENT_OF = {RULE_08: "GOV-1-08", RULE_09: "GOV-1-09"}


def run_rules(delta: Delta, freeze: Freeze) -> tuple[list[Finding], list[dict]]:
    findings: list[Finding] = []
    waivers: list[dict] = []
    for rule in (RULE_08, RULE_09):
        rule_findings, rule_waivers = RULES[rule](delta, freeze)
        findings.extend(rule_findings)
        waivers.extend(rule_waivers)
    return _sorted(findings), waivers


# ============================================================================
# git
# ============================================================================


class Git:
    """Every `git` invocation this file makes, and nothing else.

    `--no-renames` and `core.quotepath=false` are set at the call site rather
    than inherited from configuration: rename detection is a similarity
    heuristic and path quoting is a preference, and a gate's answer may depend
    on neither (INV-005).
    """

    def __init__(self, root: Path) -> None:
        self.root = root

    def __call__(self, *args: str) -> tuple[int, str]:
        try:
            proc = subprocess.run(  # noqa: S603 — fixed argv, no shell, no user string interpolation
                ["git", "-c", "core.quotepath=false", "-C", str(self.root), *args],
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                check=False,
            )
        except OSError as exc:  # git is not installed / not executable
            return 127, f"{type(exc).__name__}: {exc}"
        return proc.returncode, proc.stdout


@dataclass(frozen=True)
class BaseResolution:
    rev: str | None
    how: str
    reason: str
    trail: tuple[str, ...] = ()
    fatal: bool = False

    @property
    def resolved(self) -> bool:
        return self.rev is not None


def resolve_base(git: Callable[..., tuple[int, str]], explicit: str | None = None,
                 trunks: Sequence[str] = TRUNK_REFS) -> BaseResolution:
    """Resolve the base revision, or say precisely why there is none."""
    rc, out = git("rev-parse", "--is-inside-work-tree")
    if rc != 0 or out.strip() != "true":
        return BaseResolution(
            None,
            "unresolved",
            "not inside a git work tree, or `git` is unavailable: "
            f"`git rev-parse --is-inside-work-tree` exited {rc}",
            ("rev-parse --is-inside-work-tree",),
        )

    rc, out = git("rev-parse", "--verify", "--quiet", "HEAD^{commit}")
    if rc != 0 or not out.strip():
        return BaseResolution(
            None,
            "unresolved",
            "HEAD does not name a commit (unborn branch or empty repository); there is no head "
            "revision to diff against",
            ("rev-parse HEAD",),
        )
    head = out.strip()

    if explicit is not None:
        rc, out = git("rev-parse", "--verify", "--quiet", f"{explicit}^{{commit}}")
        if rc != 0 or not out.strip():
            return BaseResolution(
                None,
                "explicit",
                f"--base {explicit!r} does not resolve to a commit; an explicit base that is wrong is a "
                "failure, not a reason to skip",
                (f"rev-parse {explicit}",),
                fatal=True,
            )
        return BaseResolution(out.strip(), f"explicit (--base {explicit})", f"--base {explicit} resolved")

    trail: list[str] = []
    for ref in trunks:
        rc, out = git("rev-parse", "--verify", "--quiet", f"{ref}^{{commit}}")
        if rc != 0 or not out.strip():
            trail.append(f"{ref}: unknown ref")
            continue
        rc, merged = git("merge-base", head, out.strip())
        if rc != 0 or not merged.strip():
            trail.append(f"{ref}: no common ancestor with HEAD (shallow checkout or unrelated history)")
            continue
        return BaseResolution(
            merged.strip(),
            f"merge-base with {ref}",
            f"`git merge-base HEAD {ref}` resolved",
            tuple(trail),
        )

    return BaseResolution(
        None,
        "unresolved",
        "no trunk base is resolvable: " + "; ".join(trail),
        tuple(trail),
    )


def git_delta(
    git: Callable[..., tuple[int, str]], root: Path, base: str, head: str | None
) -> tuple[Delta | None, str | None]:
    """The changed-path set and the two readers, backed by git.

    With no `--head`, head is the *working tree*: the state about to be
    committed is the state the gate judges, and an added-but-unstaged file is
    included (`ls-files --others`) rather than waiting for `git add` to make it
    visible.

    A git invocation that fails returns an explained `None` rather than an
    empty changed set. An empty delta and an uncomputable one are the same
    "no violations found" to a rule and must never be the same thing to a
    reader: the second is a skip with a reason, not a pass.
    """
    if head is None:
        rc, out = git("diff", "--name-only", "--no-renames", base)
        if rc != 0:
            return None, f"`git diff --name-only {base[:12]}` exited {rc}; the delta could not be computed"
        changed = set(_lines(out))
        rc_u, untracked = git("ls-files", "--others", "--exclude-standard")
        if rc_u != 0:
            return None, (
                f"`git ls-files --others` exited {rc_u}; untracked files could not be listed, so an "
                "added-but-unstaged source would be invisible to the delta rules"
            )
        changed |= set(_lines(untracked))
        head_label = "the working tree"

        def read_head(rel: str) -> str | None:
            path = root / rel
            if not path.is_file():
                return None
            return path.read_text(encoding="utf-8", errors="replace")
    else:
        rc, out = git("diff", "--name-only", "--no-renames", base, head)
        if rc != 0:
            return None, (
                f"`git diff --name-only {base[:12]} {head}` exited {rc}; the delta could not be computed"
            )
        changed = set(_lines(out))
        head_label = head[:12]

        def read_head(rel: str) -> str | None:
            code, text = git("show", f"{head}:{rel}")
            return text if code == 0 else None

    def read_base(rel: str) -> str | None:
        code, text = git("show", f"{base}:{rel}")
        return text if code == 0 else None

    return Delta(tuple(sorted(changed)), read_base, read_head, base[:12], head_label), None


def _lines(out: str) -> list[str]:
    return [line for line in out.splitlines() if line.strip()]


# ============================================================================
# Fixtures — violating and passing base->head pairs
# ============================================================================


@dataclass(frozen=True)
class Fixture:
    fid: str
    requirement: str
    rule: str
    check: str
    expect: str  # "violation" | "deferred" | "clean"
    description: str
    base: dict[str, str] = field(default_factory=dict)
    head: dict[str, str] = field(default_factory=dict)

    def delta(self) -> Delta:
        changed = tuple(sorted(p for p in set(self.base) | set(self.head) if self.base.get(p) != self.head.get(p)))
        return Delta(changed, self.base.get, self.head.get, "the base fixture", "the head fixture")


EXPECTATIONS = ("violation", "deferred", "clean")


def load_fixtures(root: Path) -> tuple[list[Fixture], list[str]]:
    """Read every fixture pair. Fixtures are inert data, never compiled."""
    fixtures: list[Fixture] = []
    problems: list[str] = []
    base_dir = root / FIXTURE_DIR
    if not base_dir.is_dir():
        return fixtures, [f"no fixtures under {FIXTURE_DIR}"]

    classified = policy.SEMANTIC_CORE | policy.BOUNDARY | policy.ADAPTERS

    for directory in sorted(p for p in base_dir.iterdir() if p.is_dir()):
        spec_path = directory / "fixture.json"
        if not spec_path.is_file():
            problems.append(f"{directory.name}: no fixture.json")
            continue
        try:
            spec = json.loads(spec_path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            problems.append(f"{spec_path.name}: invalid JSON ({exc})")
            continue

        sides: dict[str, dict[str, str]] = {}
        ok = True
        for side in ("base", "head"):
            files: dict[str, str] = {}
            for target, payload in sorted((spec.get(side) or {}).items()):
                payload_path = directory / payload
                if not payload_path.is_file():
                    problems.append(f"{directory.name}: {side} payload {payload!r} is missing")
                    ok = False
                    continue
                files[target] = payload_path.read_text(encoding="utf-8")
                match = SEMANTIC_SOURCE_RE.fullmatch(target)
                if match and match.group(1) not in classified:
                    # The one way a hermetic fixture can rot: it names a crate
                    # the policy no longer classifies.
                    problems.append(
                        f"{directory.name}: {side} path {target!r} names crate {match.group(1)!r}, which "
                        "check_code_policy.py does not classify into any tier"
                    )
                    ok = False
                if target.endswith(".schema.json") and not target.startswith(f"{SCHEMA_DIR}/"):
                    problems.append(f"{directory.name}: {side} schema path {target!r} is outside {SCHEMA_DIR}")
                    ok = False
            sides[side] = files

        expect = spec.get("expect")
        if expect not in EXPECTATIONS:
            problems.append(f"{directory.name}: expect {expect!r} is not one of {list(EXPECTATIONS)}")
            ok = False
        rule = spec.get("rule")
        if rule not in CHECKS:
            problems.append(f"{directory.name}: unknown rule {rule!r}")
            ok = False
        elif spec.get("check") not in CHECKS[rule]:
            problems.append(f"{directory.name}: check {spec.get('check')!r} is not a sub-check of {rule}")
            ok = False
        elif REQUIREMENT_OF[rule] != spec.get("requirement"):
            problems.append(
                f"{directory.name}: rule {rule!r} belongs to {REQUIREMENT_OF[rule]}, not {spec.get('requirement')}"
            )
            ok = False
        if not sides["base"] and not sides["head"]:
            problems.append(f"{directory.name}: neither side declares a file; the pair has no delta")
            ok = False

        if not ok:
            continue
        fixtures.append(
            Fixture(
                fid=spec.get("id", directory.name),
                requirement=spec["requirement"],
                rule=spec["rule"],
                check=spec["check"],
                expect=spec["expect"],
                description=spec["description"],
                base=sides["base"],
                head=sides["head"],
            )
        )
    return fixtures, problems


# ============================================================================
# Base-resolution degradation cases (stubbed git — no repository required)
# ============================================================================

_SHA = "0123456789abcdef0123456789abcdef01234567"
_MERGE = "89abcdef0123456789abcdef0123456789abcdef"

_OK_TREE = (("rev-parse", "--is-inside-work-tree"), (0, "true\n"))
_OK_HEAD = (("rev-parse", "--verify", "--quiet", "HEAD^{commit}"), (0, f"{_SHA}\n"))
_OK_MAIN = (("rev-parse", "--verify", "--quiet", "main^{commit}"), (0, f"{_SHA}\n"))
_OK_ORIGIN = (("rev-parse", "--verify", "--quiet", "origin/main^{commit}"), (0, f"{_SHA}\n"))
_OK_MERGE = (("merge-base",), (0, f"{_MERGE}\n"))


class StubGit:
    """A table-driven `git` that never runs one, so the degradation path is testable."""

    def __init__(self, table: Sequence[tuple[tuple[str, ...], tuple[int, str]]]) -> None:
        self.table = list(table)

    def __call__(self, *args: str) -> tuple[int, str]:
        for prefix, response in self.table:
            if args[: len(prefix)] == prefix:
                return response
        return 1, ""


def degradation_cases() -> list[dict[str, object]]:
    """Nine base-resolution scenarios, each asserting resolved-or-why-not."""
    return [
        {
            "case": "git-is-not-installed",
            "git": StubGit([(("rev-parse", "--is-inside-work-tree"), (127, "FileNotFoundError: git"))]),
            "explicit": None,
            "expect_resolved": False,
            "expect_fatal": False,
            "expect_in_reason": "git",
        },
        {
            "case": "not-a-git-work-tree",
            "git": StubGit([(("rev-parse", "--is-inside-work-tree"), (128, ""))]),
            "explicit": None,
            "expect_resolved": False,
            "expect_fatal": False,
            "expect_in_reason": "work tree",
        },
        {
            "case": "unborn-head",
            "git": StubGit([_OK_TREE, (("rev-parse", "--verify", "--quiet", "HEAD^{commit}"), (1, ""))]),
            "explicit": None,
            "expect_resolved": False,
            "expect_fatal": False,
            "expect_in_reason": "unborn",
        },
        {
            "case": "no-trunk-ref",
            "git": StubGit([_OK_TREE, _OK_HEAD]),
            "explicit": None,
            "expect_resolved": False,
            "expect_fatal": False,
            "expect_in_reason": "unknown ref",
        },
        {
            "case": "shallow-checkout-no-common-ancestor",
            "git": StubGit([_OK_TREE, _OK_HEAD, _OK_MAIN, _OK_ORIGIN, (("merge-base",), (1, ""))]),
            "explicit": None,
            "expect_resolved": False,
            "expect_fatal": False,
            "expect_in_reason": "no common ancestor",
        },
        {
            "case": "merge-base-with-main",
            "git": StubGit([_OK_TREE, _OK_HEAD, _OK_MAIN, _OK_MERGE]),
            "explicit": None,
            "expect_resolved": True,
            "expect_fatal": False,
            "expect_in_reason": "merge-base",
        },
        {
            "case": "falls-back-to-origin-main",
            "git": StubGit([_OK_TREE, _OK_HEAD, _OK_ORIGIN, _OK_MERGE]),
            "explicit": None,
            "expect_resolved": True,
            "expect_fatal": False,
            "expect_in_reason": "merge-base",
        },
        {
            "case": "explicit-base-does-not-resolve",
            "git": StubGit([_OK_TREE, _OK_HEAD]),
            "explicit": "no-such-rev",
            "expect_resolved": False,
            "expect_fatal": True,
            "expect_in_reason": "explicit base that is wrong is a failure",
        },
        {
            "case": "explicit-base-resolves",
            "git": StubGit([_OK_TREE, _OK_HEAD, (("rev-parse", "--verify", "--quiet", "v1^{commit}"), (0, _SHA))]),
            "explicit": "v1",
            "expect_resolved": True,
            "expect_fatal": False,
            "expect_in_reason": "resolved",
        },
    ]


def exit_code(status: str, require_base: bool) -> int:
    """The whole exit-code policy, in one testable function.

    A skip is loud but not fatal by default (a fresh clone must not fail the
    local gate) and fatal under `--require-base` (CI states that a base is
    owed).
    """
    if status == "fail":
        return 1
    if status == "skip":
        return 1 if require_base else 0
    return 0


def degradation_self_test() -> tuple[list[str], list[dict[str, object]]]:
    failures: list[str] = []
    recorded: list[dict[str, object]] = []
    for case in degradation_cases():
        resolution = resolve_base(case["git"], explicit=case["explicit"])  # type: ignore[arg-type]
        name = case["case"]
        if resolution.resolved != case["expect_resolved"]:
            failures.append(
                f"base-resolution case {name!r}: resolved={resolution.resolved}, expected "
                f"{case['expect_resolved']}"
            )
            continue
        if resolution.fatal != case["expect_fatal"]:
            failures.append(f"base-resolution case {name!r}: fatal={resolution.fatal}, expected {case['expect_fatal']}")
            continue
        if str(case["expect_in_reason"]) not in resolution.reason:
            failures.append(
                f"base-resolution case {name!r}: reason {resolution.reason!r} does not explain the outcome "
                f"(expected it to mention {case['expect_in_reason']!r})"
            )
            continue
        if not resolution.resolved and not resolution.reason.strip():
            failures.append(f"base-resolution case {name!r}: skipped with no printed reason — a silent pass")
            continue
        recorded.append(
            {
                "case": name,
                "resolved": resolution.resolved,
                "fatal": resolution.fatal,
                "reason": resolution.reason,
                "exit_code_default": exit_code("skip" if not resolution.resolved else "pass", False),
                "exit_code_require_base": exit_code("skip" if not resolution.resolved else "pass", True),
            }
        )

    # A resolved base is not yet a delta: if git cannot produce the changed-path
    # set, an empty result must not read as "nothing changed".
    for name, table, expect_in_reason in (
        (
            "diff-fails-after-the-base-resolves",
            [(("diff",), (128, "")), (("ls-files",), (0, ""))],
            "delta could not be computed",
        ),
        (
            "untracked-listing-fails",
            [(("diff",), (0, "")), (("ls-files",), (128, ""))],
            "added-but-unstaged source would be invisible",
        ),
    ):
        delta, reason = git_delta(StubGit(table), ROOT, _SHA, None)
        if delta is not None or not reason or expect_in_reason not in reason:
            failures.append(
                f"delta-availability case {name!r}: expected an explained failure mentioning "
                f"{expect_in_reason!r}, got delta={delta is not None} reason={reason!r}"
            )
            continue
        recorded.append(
            {
                "case": name,
                "resolved": True,
                "delta_available": False,
                "reason": reason,
                "exit_code_default": exit_code("skip", False),
                "exit_code_require_base": exit_code("skip", True),
            }
        )

    # The exit-code policy itself: a skip is never a silent pass under
    # `--require-base`, and a real failure is fatal in both modes.
    for status, require, expected in (
        ("skip", False, 0),
        ("skip", True, 1),
        ("fail", False, 1),
        ("fail", True, 1),
        ("pass", False, 0),
        ("pass", True, 0),
    ):
        if exit_code(status, require) != expected:
            failures.append(f"exit-code policy: exit_code({status!r}, {require}) != {expected}")
    return failures, recorded


# ============================================================================
# Self-test
# ============================================================================


def self_test(root: Path) -> tuple[bool, dict[str, object]]:
    fixtures, problems = load_fixtures(root)
    failures: list[str] = list(problems)

    covered: dict[str, list[str]] = {}
    passing_pairs: list[str] = []
    deferred_pairs: list[str] = []

    for fixture in sorted(fixtures, key=lambda f: f.fid):
        delta = fixture.delta()
        freeze = resolve_freeze(delta)
        findings, _ = run_rules(delta, freeze)
        enforced = [f for f in findings if f.enforced]
        named = [f for f in findings if f.check == fixture.check]

        if fixture.expect == "violation":
            if not any(f.enforced for f in named):
                failures.append(
                    f"{fixture.fid}: expected an enforced {fixture.check!r} finding; got "
                    f"{[f.render() for f in findings] or 'nothing'}"
                )
                continue
            covered.setdefault(fixture.check, []).append(fixture.fid)
        elif fixture.expect == "deferred":
            if not any(not f.enforced for f in named):
                failures.append(
                    f"{fixture.fid}: expected a deferred {fixture.check!r} finding; got "
                    f"{[f.render() for f in findings] or 'nothing'}"
                )
                continue
            if enforced:
                failures.append(
                    f"{fixture.fid}: expected the finding to be deferred, but the run enforced "
                    f"{[f.render() for f in enforced]}"
                )
                continue
            deferred_pairs.append(fixture.fid)
        else:  # clean
            if enforced:
                failures.append(
                    f"{fixture.fid}: this pair must not trip anything, but it tripped "
                    f"{[f.render() for f in enforced]}"
                )
                continue
            passing_pairs.append(fixture.fid)

    for rule, checks in sorted(CHECKS.items()):
        for check in checks:
            if not covered.get(check):
                failures.append(
                    f"check {check!r} ({rule}) has no violating fixture pair; it could pass vacuously"
                )

    resolution_failures, resolution_report = degradation_self_test()
    failures.extend(resolution_failures)

    report = {
        "status": "fail" if failures else "pass",
        "fixture_pairs": len(fixtures),
        "violating_pairs_by_check": {k: sorted(v) for k, v in sorted(covered.items())},
        "passing_pairs": sorted(passing_pairs),
        "deferred_pairs": sorted(deferred_pairs),
        "base_resolution_cases": resolution_report,
        "failures": sorted(failures),
    }
    return (not failures), report


# ============================================================================
# Evidence
# ============================================================================


def build_evidence(st: dict[str, object]) -> dict:
    """A deterministic function of the repository — no revision, no clock.

    The real run's result is a function of *two revisions*, not of the tree, so
    recording it here would make a committed file change whenever history
    moves, and a diff in it would stop meaning "the policy state moved". What
    is recorded is what the tree determines: the rules, their sub-checks, the
    fixture pairs proven caught in this same run, the base-resolution and
    degradation behaviour, and the boundary. The run's own verdict is stdout's
    and CI's — this file is the proof that the gate is not vacuous.
    """
    return {
        "artifact": "continuum.governance.evidence/gov-1-delta",
        "produced_by": f"{GOVERNANCE}/check_revision_delta.py",
        "reproduce": f"python3 {GOVERNANCE}/check_revision_delta.py --evidence {EVIDENCE}",
        "authority": {
            "constitution": f"{CONSTITUTION} §1 (semantic policy: GOV-1-08, GOV-1-09)",
            "epoch_convention": f"{SCHEMA_README} (Identity; What advances a schema epoch)",
            "requirement_ids": "notes/plan/notes/PLAN_REQUIREMENTS.json",
        },
        "determinism": (
            "This file records no timestamp, commit id, or absolute path: it is a function of the "
            "repository contents alone, so an unchanged tree reproduces it byte-for-byte. The real "
            "run's verdict is deliberately not recorded here — it is a function of two revisions, not "
            "of the tree — and is reported to stdout and to CI instead."
        ),
        "requirements": {
            "GOV-1-08": {
                "rule": RULE_08,
                "checks": list(CHECKS[RULE_08]),
                "enforces": (
                    "A semantic-tier source file whose code changed between the base and the head must "
                    "cite a numbered decision record at head, unless the delta itself adds or modifies "
                    "one; a source file the delta *adds* must cite one regardless. `Semantic tier` is "
                    "check_code_policy.SEMANTIC_CORE, imported rather than re-declared. Comment-only and "
                    "formatting-only edits are not semantic changes."
                ),
                "single_revision_half": "check_code_policy.py rule `semantic-change-adr`",
            },
            "GOV-1-09": {
                "rule": RULE_09,
                "checks": list(CHECKS[RULE_09]),
                "enforces": (
                    "A schema document whose bytes changed with `schema_epoch` unchanged, or which was "
                    "deleted, is a violation once the PR 5 freeze is in force; a `schema_epoch` that "
                    "retreats, and an advance published without its typed "
                    "`Preserved | Revalidate | Incompatible` statement in the same delta, are violations "
                    "in every regime."
                ),
                "single_revision_half": "check_code_policy.py rule `epoch-discipline`",
            },
        },
        "base_resolution": {
            "default": "git merge-base HEAD main, falling back to origin/main",
            "explicit": "--base <rev>; an explicit base that does not resolve is a hard failure",
            "head_default": "the working tree (untracked files included); --head <rev> for a revision",
            "degradation": (
                "No resolvable base means SKIP: the reason is printed on stderr, the report carries "
                "\"status\": \"skip\" and the resolution trail, and the delta rules are reported as not "
                "enforced. Exit 0 by default so a fresh clone does not fail the local gate; exit 1 under "
                "--require-base, which is how CI states that a base is owed. There is no path on which "
                "an unenforced run reports a pass."
            ),
        },
        "freeze_regime": {
            "read_from": SCHEMA_README,
            "clause": FREEZE_DRAFT_CLAUSE,
            "pre_freeze": (
                "While the clause is present, epoch 1 is a draft edited in place: "
                f"`{CHECK_INPLACE}` and `{CHECK_DELETED}` findings are DEFERRED — printed and reported, "
                "not failures."
            ),
            "in_force": "Clause absent (or --freeze in-force): the same findings are violations.",
            "fail_closed": "An absent or reworded schemas/README.md is read as `in force`.",
        },
        "self_test": st,
        "boundary": (
            "Two revisions, not a history: this compares a base to a head and does not look at the "
            "commits in between, so an edit made and reverted inside a branch is correctly invisible and "
            "a squashed force-push is judged on its result. `Semantic change` is approximated by "
            "`the comment-stripped code of a semantic-tier file changed`, which over-approximates (a "
            "local rename counts) and under-approximates in one narrow corner (a change confined to "
            "run-of-spaces inside a string literal). Whether the cited decision record actually governs "
            "the change, and whether a published compatibility verdict is the correct one, are review "
            "judgements no diff can make: this proves the record and the statement exist and shipped "
            "with the change. The epoch rule reads schema documents only — an IDL or RFC edit that "
            "breaks a contract without touching a schema file is out of its scope."
        ),
        "status": "fail" if st.get("status") != "pass" else "pass",
    }


# ============================================================================
# Entry point
# ============================================================================


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--base", metavar="REV", default=None, help="base revision (default: merge base with main)")
    parser.add_argument("--head", metavar="REV", default=None, help="head revision (default: the working tree)")
    parser.add_argument(
        "--require-base",
        action="store_true",
        help="fail when no base revision is resolvable, instead of skipping (CI)",
    )
    parser.add_argument(
        "--freeze",
        choices=("auto", "in-force", "pre-freeze"),
        default="auto",
        help="override the PR 5 freeze regime read from schemas/README.md",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="replay every fixture pair and every base-resolution case; fail if any goes uncaught",
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

    if args.self_test and not args.evidence:
        ok, report = self_test(root)
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0 if ok else 1

    st: dict[str, object] = {"status": "not-run"}
    st_ok = True
    if args.evidence:
        st_ok, st = self_test(root)
        evidence = build_evidence(st)
        path = Path(args.evidence)
        if not path.is_absolute():
            path = root / path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    git = Git(root)
    resolution = resolve_base(git, explicit=args.base)

    if not resolution.resolved:
        status = "fail" if resolution.fatal else "skip"
        report = {
            "status": status,
            "base_resolution": {
                "resolved": False,
                "how": resolution.how,
                "reason": resolution.reason,
                "trail": list(resolution.trail),
            },
            "enforced": False,
            "rules": sorted(CHECKS),
            "self_test": st.get("status"),
            "evidence": args.evidence,
        }
        banner = (
            f"continuum: GOV-1-08/09 delta gate {'FAILED' if resolution.fatal else 'SKIPPED'} — "
            f"{resolution.reason}. The delta rules were NOT enforced."
        )
        print(banner, file=sys.stderr)
        print(report["status"] if args.quiet else json.dumps(report, indent=2, sort_keys=True))
        code = exit_code(status, args.require_base)
        return 1 if (code or not st_ok) else 0

    delta, delta_error = git_delta(git, root, resolution.rev or "", args.head)
    if delta is None:
        report = {
            "status": "skip",
            "base_resolution": {
                "resolved": True,
                "base": resolution.rev,
                "how": resolution.how,
                "reason": resolution.reason,
                "trail": list(resolution.trail),
            },
            "enforced": False,
            "skip_reason": delta_error,
            "rules": sorted(CHECKS),
            "self_test": st.get("status"),
            "evidence": args.evidence,
        }
        print(
            f"continuum: GOV-1-08/09 delta gate SKIPPED — {delta_error}. The delta rules were NOT enforced.",
            file=sys.stderr,
        )
        print(report["status"] if args.quiet else json.dumps(report, indent=2, sort_keys=True))
        return 1 if (exit_code("skip", args.require_base) or not st_ok) else 0

    freeze = resolve_freeze(delta, args.freeze)
    findings, waivers = run_rules(delta, freeze)
    violations = [f for f in findings if f.enforced]
    deferred = [f for f in findings if not f.enforced]

    status = "fail" if violations else "pass"
    report = {
        "status": status,
        "base_resolution": {
            "resolved": True,
            "base": resolution.rev,
            "how": resolution.how,
            "reason": resolution.reason,
            "trail": list(resolution.trail),
        },
        "head": delta.head_label,
        "freeze": {"in_force": freeze.in_force, "reason": freeze.reason, "read_from": freeze.source},
        "delta": {
            "changed_files": len(delta.changed),
            "semantic_sources": delta.changed_matching(is_semantic_source),
            "schema_documents": delta.changed_matching(is_schema_document),
            "decision_records": delta.changed_matching(is_decision_record),
        },
        "rules": sorted(CHECKS),
        "violations": [f.render() for f in violations],
        "deferred": [f.render() for f in deferred],
        "waivers_granted": waivers,
        "self_test": st.get("status"),
        "evidence": args.evidence,
    }
    if deferred:
        print(
            f"continuum: {len(deferred)} GOV-1-09 finding(s) recorded but NOT enforced — {freeze.reason}",
            file=sys.stderr,
        )
    print(report["status"] if args.quiet else json.dumps(report, indent=2, sort_keys=True))
    return 1 if (violations or not st_ok) else 0


if __name__ == "__main__":
    raise SystemExit(main())
