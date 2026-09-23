"""GOV-4-19, GOV-4-20, GOV-4-21 — kernel changes: mutation tests, code-size
report, no unchecked optimization (bn-2b4e).

Source: `notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md` §4 "Kernel changes /
Require:" — the third, fourth and fifth items. The first two, two reviewers
(`GOV-4-17`) and fuzz corpus (`GOV-4-18`), plus the remaining Pack-changes
obligations (`GOV-4-13`..`GOV-4-16`), are sibling Bone bn-4x90's.

A new change class, distinct from GOV-1-08's semantic tier and from
`gov4_impact_reduction_pack`'s POR/pack tiers: a **kernel change** is a source
file of one of the four `continuum-kernel-*` crates — plan §20's trusted
checking base, and `notes/plan/docs/03_ASSURANCE_AND_TCB.md` §5's
<15,000-non-test-line covenant, already enforced tree-wide (every invocation,
not just a delta) by `tools/check_kernel_covenant.py` (KCOV-01..09) — whose
comment-stripped code changed, or that the delta added. It carries its own
review-record section, `[kernel]`:

    [kernel]
    covers = ["crates/continuum-kernel-sat/src/solver.rs"]   # paths, or dir prefixes ending in /

    [kernel.mutation_tests]
    tests = ["crates/continuum-kernel-sat/tests/x.rs::rejects_flipped_polarity"]
    classes = ["encoding.injectivity"]      # tools/kernel-covenant/mutation-matrix.toml [[class]] id

    [kernel.code_size]
    total = 11009      # live non-test lines over all four kernel crates, KCOV-01's own count
    budget = 15000     # docs/03 §5's covenant, read live from tools/check_kernel_covenant.py
    note = "continuum-kernel-sat grew by five lines for the new watch-list index"

    [kernel.no_unchecked_optimization]
    is_optimization = true
    verified_by = "crates/continuum-kernel-sat/tests/x.rs::fast_path_matches_reference"

A change that is not an optimization states that instead:

    [kernel.no_unchecked_optimization]
    is_optimization = false
    reason = "pure refactor: renames a field, no behavior change"

Why `code_size` is checked by recomputation, not by trusting the number: the
covenant is over the whole trusted checking base, not a per-crate budget, and
`tools/kernel-covenant/evidence/kernel-covenant.json` is a committed artifact
that can be stale relative to the tree under review — INV-004's "no
self-certification" spirit applies here even though this is not a semantic
claim: a report that is not re-derivable from the tree it describes is not
evidence. `tools/check_kernel_covenant.count_crate` reads a tree through
`.glob`/`.read_text` only, the same shape as `check_code_policy.Tree`, so an
obligation `Context`'s own tree view (`ctx.head`), fixture overlays included,
works as its argument unchanged — reused, not reimplemented.
"""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path

from check_obligations import (
    DELTA,
    TREE,
    Context,
    Finding,
    ObligationSet,
    Record,
    Rule,
    markdown_list,
    normalize,
    requirement_summaries,
    resolve_test,
    test_is_fresh,
)
import check_revision_delta as delta_mod

# `tools/check_kernel_covenant.py` lives one level above `tools/governance/`;
# it is a GOV-1-tier tool with its own self-test and evidence file, not a
# sibling obligation set, and this file only imports its pure, read-only
# counting function — no shared line of it is edited here.
_TOOLS_DIR = Path(__file__).resolve().parents[2]
if str(_TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(_TOOLS_DIR))
import check_kernel_covenant as kcov  # noqa: E402

CONSTITUTION = "notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md"
MATRIX = "tools/kernel-covenant/mutation-matrix.toml"

KERNEL_HEADING = "### Kernel changes"
REQUIRE_LEAD = "Require:"
# The whole five-item list, so the tree rule also catches drift in the two
# items GOV-4-17/18 own (`gov4_impact_reduction_pack.PACK_SECTION_ITEMS` does
# the same for the Pack-changes items a later Bone owns).
KERNEL_SECTION_ITEMS = (
    "two reviewers",
    "fuzz corpus",
    "mutation tests",
    "code-size report",
    "no unchecked optimization",
)

# plan §20's four trusted-checking-base crates. A local classification, not an
# import of `check_code_policy.KERNEL` or `.SEMANTIC_CORE`: a kernel change is
# its own change class here, the same way `gov4_impact_reduction_pack`'s
# `POR_REDUCTION_CORE` and `PACK_CORE` are their own, even though every kernel
# crate also sits inside `SEMANTIC_CORE`. It is, however, the exact same tuple
# `tools/check_kernel_covenant.py` (KCOV-01..09) already classifies as the
# trusted checking base, so it is read from there rather than typed a third
# time.
KERNEL_CORE = frozenset(kcov.KERNEL_CRATES)

REQUIREMENTS = {
    "GOV-4-19": "mutation tests",
    "GOV-4-20": "code-size report",
    "GOV-4-21": "no unchecked optimization",
}
ALL = tuple(REQUIREMENTS)

NOTE_MIN = 40
REASON_MIN = 24

# ---------------------------------------------------------------------------
# Rule and check names
# ---------------------------------------------------------------------------

R_RECORD = "kernel-review-record"
C_HAS_RECORD = "kernel-change-has-review-record"
C_COVERS = "kernel-review-record-covers-a-kernel-change"

R_MUTATION = "kernel-mutation-tests"
C_MUT_NAMED = "kernel-mutation-tests-named"
C_MUT_RESOLVES = "kernel-mutation-test-resolves"
C_MUT_EXERCISES = "kernel-mutation-test-exercises-the-change"
C_MUT_FRESH = "kernel-mutation-test-is-fresh"
C_MUT_CLASS = "kernel-mutation-class-registered"

R_CODE_SIZE = "kernel-code-size-report"
C_SIZE_PRESENT = "kernel-code-size-report-present"
C_SIZE_MATCHES = "kernel-code-size-report-matches-live-count"
C_SIZE_NAMES = "kernel-code-size-report-names-the-change"

R_OPTIMIZATION = "kernel-no-unchecked-optimization"
C_OPT_DECLARED = "kernel-optimization-declared"
C_OPT_VERIFIED = "kernel-optimization-verified-by-resolves"
C_OPT_FRESH = "kernel-optimization-verified-by-is-fresh"
C_OPT_EXERCISES = "kernel-optimization-verified-by-exercises-the-change"
C_OPT_REASON = "kernel-non-optimization-reason-present"

R_LIST = "kernel-review-list"
C_LIST = "kernel-review-list-registered"
C_VOCAB = "kernel-mutation-vocabulary-present"


# ---------------------------------------------------------------------------
# Shared helpers
# ---------------------------------------------------------------------------


def _covers(entry: str, rel: str) -> bool:
    return rel == entry or (entry.endswith("/") and rel.startswith(entry))


def _crates(ctx: Context, paths: list[str]) -> set[str]:
    return {c for c in (ctx.crate_of(p) for p in paths) if c}


def _names_a_crate(text: str, crates: set[str]) -> bool:
    return any(c in text or c.replace("continuum-", "") in text.split() for c in crates)


def kernel_changes(ctx: Context) -> list[str]:
    """Source files of a `continuum-kernel-*` crate whose comment-stripped code
    changed, or that the delta added. Same shape as `Context.semantic_changes()`
    and `gov4_impact_reduction_pack._tier_source_changes()`, restricted to
    `KERNEL_CORE` instead of `SEMANTIC_CORE` or a POR/pack tier.
    """
    out: list[str] = []
    for rel in ctx.changed_matching(lambda p: bool(
        (m := delta_mod.SEMANTIC_SOURCE_RE.fullmatch(p)) and m.group(1) in KERNEL_CORE
    )):
        head = ctx.head.read_text(rel)
        if head is None:
            continue
        base = ctx.base.read_text(rel) if ctx.base else None
        if base is None or delta_mod.semantic_text(base) != delta_mod.semantic_text(head):
            out.append(rel)
    return out


def kernel_records(ctx: Context) -> list[tuple[Record, dict, list[str]]]:
    """(record, its `[kernel]` table, the kernel changes it covers) for every
    record the delta adds or modifies that names the section."""
    changes = kernel_changes(ctx)
    out: list[tuple[Record, dict, list[str]]] = []
    for rec in ctx.records_in_delta():
        if rec.data is None or not isinstance(rec.data.get("kernel"), dict):
            continue
        table = rec.data["kernel"]
        entries = (
            [e for e in table.get("covers", []) if isinstance(e, str)]
            if isinstance(table.get("covers"), list)
            else []
        )
        covered = [c for c in changes if any(_covers(e, c) for e in entries)]
        out.append((rec, table, covered))
    return out


def mutation_class_vocabulary(ctx: Context) -> list[str]:
    """`tools/kernel-covenant/mutation-matrix.toml`'s `[[class]] id`s, read live —
    the same file KCOV-08 already indexes every kernel `#[test]` against."""
    text = ctx.head.read_text(MATRIX) or ""
    try:
        data = tomllib.loads(text)
    except tomllib.TOMLDecodeError:
        return []
    return [
        c["id"] for c in data.get("class", []) if isinstance(c, dict) and isinstance(c.get("id"), str)
    ]


def live_kernel_total(ctx: Context) -> int:
    """Non-test lines over all four kernel crates, KCOV-01's own counting
    method, recomputed against `ctx.head` — including any fixture overlay."""
    return sum(kcov.count_crate(ctx.head, c).total for c in kcov.KERNEL_CRATES)


# ---------------------------------------------------------------------------
# Every kernel change has a review record (shared by all three ids)
# ---------------------------------------------------------------------------


def rule_kernel_record(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    changes = kernel_changes(ctx)
    records = kernel_records(ctx)
    covered_all = {c for _, _, cs in records for c in cs}
    for rel in changes:
        if rel not in covered_all:
            out.append(Finding(
                R_RECORD, C_HAS_RECORD,
                f"{rel} is a kernel change ({ctx.base_label} -> {ctx.head_label}) and no review record the "
                f"delta adds or modifies under tools/governance/reviews/ covers it in `[kernel].covers`; "
                f"{CONSTITUTION} §4 requires mutation tests, a code-size report and a no-unchecked-optimization "
                "statement for it",
            ))
    for rec, table, _ in records:
        entries = table.get("covers")
        if not isinstance(entries, list) or not entries or not all(isinstance(e, str) for e in entries):
            out.append(Finding(R_RECORD, C_COVERS, f"{rec.path}: `kernel.covers` must be a non-empty list of paths"))
            continue
        for entry in entries:
            if not any(_covers(entry, c) for c in changes):
                out.append(Finding(
                    R_RECORD, C_COVERS,
                    f"{rec.path}: `kernel.covers` entry {entry!r} matches no kernel change in the delta; a "
                    "record claims only what the change actually touched",
                ))
    return out


# ---------------------------------------------------------------------------
# GOV-4-19 — mutation tests
# ---------------------------------------------------------------------------


def rule_mutation_tests(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    vocab = set(mutation_class_vocabulary(ctx))
    for rec, table, covered in kernel_records(ctx):
        if not covered:
            continue
        crates = _crates(ctx, covered)
        mt = table.get("mutation_tests")
        entries = mt.get("tests") if isinstance(mt, dict) else None
        classes = mt.get("classes") if isinstance(mt, dict) else None
        if (
            not isinstance(mt, dict)
            or not isinstance(entries, list) or not entries or not all(isinstance(e, str) for e in entries)
            or not isinstance(classes, list) or not classes or not all(isinstance(e, str) for e in classes)
        ):
            out.append(Finding(
                R_MUTATION, C_MUT_NAMED,
                f"{rec.path}: `[kernel.mutation_tests]` with non-empty `tests` and `classes` lists is missing "
                f"or malformed; a kernel change over {sorted(crates)} requires mutation tests ({CONSTITUTION} §4)",
            ))
            continue
        fresh_any = exercised_any = False
        bad = False
        for ref in entries:
            test, problem = resolve_test(ctx, ref)
            if test is None:
                out.append(Finding(R_MUTATION, C_MUT_RESOLVES, f"{rec.path}: {problem}"))
                bad = True
                continue
            fresh_any |= test_is_fresh(ctx, test)
            test_crate = ctx.crate_of(test.path)
            exercised_any |= bool(test_crate) and ctx.reaches(test_crate, crates)
        if bad:
            continue
        if not exercised_any:
            out.append(Finding(
                R_MUTATION, C_MUT_EXERCISES,
                f"{rec.path}: no test in `kernel.mutation_tests.tests` lives in, or reaches through its "
                f"dependencies, a changed kernel crate {sorted(crates)}",
            ))
        if not fresh_any:
            out.append(Finding(
                R_MUTATION, C_MUT_FRESH,
                f"{rec.path}: no test in `kernel.mutation_tests.tests` is new or has a changed body in this "
                "delta",
            ))
        if not any(c in vocab for c in classes):
            out.append(Finding(
                R_MUTATION, C_MUT_CLASS,
                f"{rec.path}: `kernel.mutation_tests.classes` names none of {MATRIX}'s registered classes "
                f"{sorted(vocab)}; a mutation test names the mutation class it applies",
            ))
    return out


# ---------------------------------------------------------------------------
# GOV-4-20 — code-size report
# ---------------------------------------------------------------------------


def rule_code_size(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    live_total = live_kernel_total(ctx)
    for rec, table, covered in kernel_records(ctx):
        if not covered:
            continue
        crates = _crates(ctx, covered)
        size = table.get("code_size")
        total, budget, note = (
            (size.get("total"), size.get("budget"), size.get("note")) if isinstance(size, dict) else (None, None, None)
        )
        if (
            not isinstance(size, dict)
            or not isinstance(total, int) or isinstance(total, bool)
            or not isinstance(budget, int) or isinstance(budget, bool)
            or not isinstance(note, str)
        ):
            out.append(Finding(
                R_CODE_SIZE, C_SIZE_PRESENT,
                f"{rec.path}: `[kernel.code_size]` with an integer `total`, an integer `budget` and a `note` "
                f"is missing or malformed; a kernel change requires a code-size report ({CONSTITUTION} §4)",
            ))
            continue
        if total != live_total or budget != kcov.LINE_BUDGET:
            out.append(Finding(
                R_CODE_SIZE, C_SIZE_MATCHES,
                f"{rec.path}: `kernel.code_size` reports total={total!r}, budget={budget!r}, but the live "
                f"count over {list(kcov.KERNEL_CRATES)} (KCOV-01's own method, tools/check_kernel_covenant.py) "
                f"is total={live_total}, budget={kcov.LINE_BUDGET} — a report that does not match what it "
                "claims to measure is not evidence",
            ))
            continue
        text = normalize(note)
        if len(text) < NOTE_MIN or not _names_a_crate(text, crates):
            out.append(Finding(
                R_CODE_SIZE, C_SIZE_NAMES,
                f"{rec.path}: `kernel.code_size.note` must be at least {NOTE_MIN} characters and name a "
                f"changed kernel crate {sorted(crates)}",
            ))
    return out


# ---------------------------------------------------------------------------
# GOV-4-21 — no unchecked optimization
# ---------------------------------------------------------------------------


def rule_no_unchecked_optimization(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    for rec, table, covered in kernel_records(ctx):
        if not covered:
            continue
        crates = _crates(ctx, covered)
        opt = table.get("no_unchecked_optimization")
        is_opt = opt.get("is_optimization") if isinstance(opt, dict) else None
        if not isinstance(opt, dict) or not isinstance(is_opt, bool):
            out.append(Finding(
                R_OPTIMIZATION, C_OPT_DECLARED,
                f"{rec.path}: `[kernel.no_unchecked_optimization]` with a boolean `is_optimization` is missing "
                f"or malformed; a kernel change states whether it changes an optimized path, even when the "
                f"answer is no ({CONSTITUTION} §4)",
            ))
            continue
        if is_opt:
            ref = opt.get("verified_by")
            test, problem = resolve_test(ctx, ref)
            if test is None:
                out.append(Finding(
                    R_OPTIMIZATION, C_OPT_VERIFIED,
                    f"{rec.path}: `kernel.no_unchecked_optimization.verified_by`: {problem} — an optimization "
                    "ships a test that checks it, never an unchecked shortcut",
                ))
                continue
            if not test_is_fresh(ctx, test):
                out.append(Finding(
                    R_OPTIMIZATION, C_OPT_FRESH,
                    f"{rec.path}: `kernel.no_unchecked_optimization.verified_by` {ref!r} is neither new nor "
                    "changed in this delta",
                ))
            test_crate = ctx.crate_of(test.path)
            if not (test_crate and ctx.reaches(test_crate, crates)):
                out.append(Finding(
                    R_OPTIMIZATION, C_OPT_EXERCISES,
                    f"{rec.path}: `kernel.no_unchecked_optimization.verified_by` {ref!r} does not live in, or "
                    f"reach through its dependencies, a changed kernel crate {sorted(crates)}",
                ))
        else:
            reason = opt.get("reason")
            if not isinstance(reason, str) or len(normalize(reason)) < REASON_MIN:
                out.append(Finding(
                    R_OPTIMIZATION, C_OPT_REASON,
                    f"{rec.path}: `kernel.no_unchecked_optimization.is_optimization` is false and `reason` is "
                    f"missing or shorter than {REASON_MIN} characters; `not an optimization` is a claim that "
                    "carries its reason",
                ))
    return out


# ---------------------------------------------------------------------------
# Tree rule: the source list and the mutation-class vocabulary still hold
# ---------------------------------------------------------------------------


def rule_list(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    text = ctx.head.read_text(CONSTITUTION) or ""
    items = markdown_list(text, KERNEL_HEADING, REQUIRE_LEAD)
    if items is None or tuple(items) != KERNEL_SECTION_ITEMS:
        out.append(Finding(
            R_LIST, C_LIST,
            f"{CONSTITUTION} §4 `{KERNEL_HEADING}` / `{REQUIRE_LEAD}` lists {items}, not "
            f"{list(KERNEL_SECTION_ITEMS)}; the GOV-4-19..21 ids this set binds are positional",
        ))
    summaries = requirement_summaries(ctx)
    for rid, bullet in REQUIREMENTS.items():
        got = summaries.get(rid)
        if got is None or got.rstrip(";.").strip() != bullet:
            out.append(Finding(
                R_LIST, C_LIST,
                f"notes/plan/notes/PLAN_REQUIREMENTS.json {rid} summary is {got!r}, not {bullet!r}",
            ))
    return out


def rule_vocabulary(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    if len(mutation_class_vocabulary(ctx)) < 2:
        out.append(Finding(
            R_LIST, C_VOCAB,
            f"{MATRIX} no longer lists mutation classes GOV-4-19 names its tests by",
        ))
    return out


def rule_list_and_vocabulary(ctx: Context) -> list[Finding]:
    """One `Rule` carries one `fn`; `C_LIST` and `C_VOCAB` share this rule."""
    return [*rule_list(ctx), *rule_vocabulary(ctx)]


# ---------------------------------------------------------------------------
# The set
# ---------------------------------------------------------------------------

_TEST_BOUNDARY = (
    "Proves the named test exists at head, is a non-ignored `#[test]` that `just check` runs, lives in or "
    "reaches a changed kernel crate, and that at least one named test is new or changed in the delta. It does "
    "not judge whether the assertions pin the mutation or the checked equivalence they claim."
)
_NOTE_BOUNDARY = (
    "Proves a typed field exists and a note of minimum length names a changed kernel crate. Whether the note "
    "is accurate is a review judgement, the same limit every other note-bearing obligation in this harness "
    "states."
)

SET = ObligationSet(
    name="gov-4-kernel",
    title="GOV §4 kernel changes — mutation tests, code-size report, no unchecked optimization (GOV-4-19..21)",
    source=f"{CONSTITUTION} §4 Review requirements",
    requirements=REQUIREMENTS,
    evidence="tools/governance/evidence/gov-4-kernel.json",
    record_keys={
        "kernel": ("covers", "mutation_tests", "code_size", "no_unchecked_optimization"),
    },
    rules=(
        Rule(
            R_RECORD, ALL, (C_HAS_RECORD, C_COVERS), DELTA,
            "Every kernel change (a `continuum-kernel-*` source file whose comment-stripped code changed or "
            "that the delta added) is covered by a review record the delta adds or modifies; every "
            "`kernel.covers` entry matches such a change.",
            None,
            rule_kernel_record,
        ),
        Rule(
            R_MUTATION, ("GOV-4-19",),
            (C_MUT_NAMED, C_MUT_RESOLVES, C_MUT_EXERCISES, C_MUT_FRESH, C_MUT_CLASS), DELTA,
            "`kernel.mutation_tests` names at least one test that resolves, reaches the change, is new or "
            "changed, and at least one class from tools/kernel-covenant/mutation-matrix.toml's registered "
            "mutation classes.",
            _TEST_BOUNDARY,
            rule_mutation_tests,
        ),
        Rule(
            R_CODE_SIZE, ("GOV-4-20",), (C_SIZE_PRESENT, C_SIZE_MATCHES, C_SIZE_NAMES), DELTA,
            "`[kernel.code_size]` states `total` and `budget`, recomputed live over the four kernel crates by "
            "the same method KCOV-01 (tools/check_kernel_covenant.py) uses, and a `note` of minimum length "
            "naming a changed kernel crate.",
            "EXACT: `total` and `budget` are recomputed against `ctx.head` on every run and compared for "
            "equality, not trusted from the record or from the separately-committed kernel-covenant evidence "
            "file, which can be stale relative to the tree under review.",
            rule_code_size,
        ),
        Rule(
            R_OPTIMIZATION, ("GOV-4-21",),
            (C_OPT_DECLARED, C_OPT_VERIFIED, C_OPT_FRESH, C_OPT_EXERCISES, C_OPT_REASON), DELTA,
            "`[kernel.no_unchecked_optimization]` states a boolean `is_optimization`; when true, `verified_by` "
            "names a test that resolves, reaches the change, and is new or changed; when false, `reason` "
            "carries a minimum-length explanation.",
            _TEST_BOUNDARY + " `is_optimization` is self-reported, the same shape `gov4_impact_reduction_pack`'s "
            "claim-impact obligation (GOV-4-07) already carries: the check proves the claim is typed and, when "
            "it claims an optimization, that a fresh test in the changed crate exists to check it — not that "
            "the test actually establishes the checked equivalence, which is a review judgement.",
            rule_no_unchecked_optimization,
        ),
        Rule(
            R_LIST, ALL, (C_LIST, C_VOCAB), TREE,
            "docs/12 §4's Kernel changes list, and the generated registry's GOV-4-19..21 summaries, still name "
            "these ids in order; tools/kernel-covenant/mutation-matrix.toml still carries the mutation classes "
            "GOV-4-19 types against.",
            None,
            rule_list_and_vocabulary,
        ),
    ),
    notes=(
        "GOV-4-17/18 (two reviewers, fuzz corpus) and GOV-4-13..16 (the remaining Pack-changes obligations) "
        "are sibling Bone bn-4x90's; KERNEL_SECTION_ITEMS names the whole five-item Kernel-changes list so the "
        "list check still catches drift in the two items this set does not own.",
        "KERNEL_CORE is `tools/check_kernel_covenant.KERNEL_CRATES`, reused rather than retyped: both files "
        "mean plan §20's same four crates, and `check_kernel_covenant.count_crate` is reused unchanged for "
        "GOV-4-20's live recomputation.",
    ),
)
