"""GOV-4-01 … GOV-4-06 — what a semantic change must ship with (bn-37b1).

Source: `notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md` §4 "Review
requirements", "Semantic changes / Require:" — reference tests, metamorphic
tests, differential tests, updated schemas, migration note, security review,
claim impact. The first six are this set; `GOV-4-07` (claim impact) is sibling
Bone bn-2tm3's and extends the same `[semantic]` record section from its own
file.

What counts as a semantic change is not decided here. It is GOV-1-08's delta
predicate, `Context.semantic_changes()`: a source file of a
`check_code_policy.SEMANTIC_CORE` crate whose comment-stripped code changed, or
that the delta added. A change that has one ships a review record,
`tools/governance/reviews/<name>.toml`, whose `[semantic]` section names the
evidence each obligation requires, and every name is resolved against the head
tree:

    [change]
    bone = "bn-xxxx"
    author = "continuum-dev"
    summary = "what changed, in one line"

    [semantic]
    covers = ["crates/continuum-engine-reference/src/"]      # paths or dir prefixes
    reference_tests = ["crates/<c>/tests/<f>.rs::<fn>"]
    metamorphic_tests = [{ test = "…::<fn>", relation = "serialization round trip" }]
    differential_tests = [
      { test = "…::<fn>", oracle = "continuum-engine-reference", subject = "continuum-engine-explicit" },
    ]

    [semantic.schemas]
    updated = []                                             # schema documents changed
    reason = "why no schema document changes"               # owed when `updated` is empty
    unaffected = { context-pack = "why this class is untouched" }

    [semantic.migration]
    verdict = "Preserved"                                    # Preserved | Revalidate | Incompatible
    note = "what a consumer of the changed crate must do"

    [semantic.security_review]
    reviewer = "continuum-security"
    review = "<seal review id>"
    verdict = "approved"
"""

from __future__ import annotations

import re

from check_obligations import (
    COMPATIBILITY_VERDICTS,
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

CONSTITUTION = "notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md"
TEST_STRATEGY = "notes/plan/docs/19_TEST_STRATEGY.md"
SECTION_HEADING = "### Semantic changes"
SECTION_LEAD = "Require:"
RELATIONS_HEADING = "## 3. Metamorphic relations"
RELATIONS_LEAD = "Expected preservation:"

REQUIREMENTS = {
    "GOV-4-01": "reference tests",
    "GOV-4-02": "metamorphic tests",
    "GOV-4-03": "differential tests",
    "GOV-4-04": "updated schemas",
    "GOV-4-05": "migration note",
    "GOV-4-06": "security review",
}
ALL = tuple(REQUIREMENTS)
# §4 "Semantic changes" lists seven items; the seventh is GOV-4-07's.
SECTION_ITEMS = (*REQUIREMENTS.values(), "claim impact")

MIN_REASON = 24
MIN_NOTE = 40
REVIEW_VERDICTS = ("approved", "blocked", "pending")
REVIEW_ID_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{2,63}")
SCHEMA_CLASS_REF = re.compile(r"continuum\.dev/schema/(?:v\d+/)?([a-z0-9-]+)\.json")

# ---------------------------------------------------------------------------
# Rule and check names
# ---------------------------------------------------------------------------

R_RECORD = "semantic-review-record"
C_HAS_RECORD = "semantic-change-has-review-record"
C_COVERS = "review-record-covers-a-semantic-change"

R_LIST = "semantic-review-list"
C_LIST = "semantic-review-list-registered"
C_RELATIONS = "metamorphic-relation-vocabulary-present"

R_REF = "reference-tests"
R_META = "metamorphic-tests"
R_DIFF = "differential-tests"
R_SCHEMA = "updated-schemas"
C_SCHEMA_DECLARED = "schema-impact-declared"
C_SCHEMA_IN_DELTA = "declared-schema-update-in-delta"
C_SCHEMA_UNDECLARED = "schema-change-declared"
C_SCHEMA_BEARING = "schema-bearing-source-addressed"
R_MIGRATION = "migration-note"
C_MIG_PRESENT = "migration-note-present"
C_MIG_VERDICT = "migration-verdict-closed"
C_MIG_NAMES = "migration-note-names-the-change"
C_MIG_EPOCH = "incompatible-verdict-advances-an-epoch"
R_SECURITY = "security-review"
C_SEC_PRESENT = "security-review-present"
C_SEC_INDEPENDENT = "security-review-independent"
C_SEC_APPROVED = "security-review-approved"


def _test_checks(prefix: str) -> dict[str, str]:
    return {
        "named": f"{prefix}-tests-named",
        "resolves": f"{prefix}-test-resolves",
        "exercises": f"{prefix}-test-exercises-the-change",
        "fresh": f"{prefix}-test-is-fresh",
    }


REF = _test_checks("reference")
META = {**_test_checks("metamorphic"), "registered": "metamorphic-relation-registered",
        "stated": "metamorphic-relation-stated-at-the-test"}
DIFF = {**_test_checks("differential"), "distinct": "differential-pair-distinct-implementations",
        "uses": "differential-test-uses-both-implementations"}


# ---------------------------------------------------------------------------
# Shared: which records apply, and what they cover
# ---------------------------------------------------------------------------


def _covers(entry: str, rel: str) -> bool:
    return rel == entry or (entry.endswith("/") and rel.startswith(entry))


def semantic_records(ctx: Context) -> list[tuple[Record, dict, list[str]]]:
    """(record, its [semantic] table, the semantic changes it covers)."""
    changes = ctx.semantic_changes()
    out = []
    for rec in ctx.records_in_delta():
        if rec.data is None or not isinstance(rec.data.get("semantic"), dict):
            continue
        sem = rec.data["semantic"]
        entries = [e for e in sem.get("covers", []) if isinstance(e, str)] if isinstance(sem.get("covers"), list) else []
        covered = [c for c in changes if any(_covers(e, c) for e in entries)]
        out.append((rec, sem, covered))
    return out


def _crates(ctx: Context, paths: list[str]) -> set[str]:
    return {c for c in (ctx.crate_of(p) for p in paths) if c}


def rule_record(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    records = semantic_records(ctx)
    covered = {c for _, _, cs in records for c in cs}
    for rel in ctx.semantic_changes():
        if rel not in covered:
            out.append(Finding(
                R_RECORD, C_HAS_RECORD,
                f"{rel} is a semantic change ({ctx.base_label} -> {ctx.head_label}) and no review record the "
                f"delta adds or modifies under tools/governance/reviews/ covers it in `[semantic].covers`; "
                f"{CONSTITUTION} §4 requires reference, metamorphic, and differential tests, updated schemas, "
                "a migration note, and a security review for it",
            ))
    for rec, sem, _ in records:
        entries = sem.get("covers")
        if not isinstance(entries, list) or not entries or not all(isinstance(e, str) for e in entries):
            out.append(Finding(R_RECORD, C_COVERS, f"{rec.path}: `semantic.covers` must be a non-empty list of paths"))
            continue
        for entry in entries:
            if not any(_covers(entry, c) for c in ctx.semantic_changes()):
                out.append(Finding(
                    R_RECORD, C_COVERS,
                    f"{rec.path}: `semantic.covers` entry {entry!r} matches no semantic change in the delta; a "
                    "record claims only what the change actually touched",
                ))
    return out


# ---------------------------------------------------------------------------
# Tree rule: the source list and the relation vocabulary still say what the ids bind
# ---------------------------------------------------------------------------


def relation_vocabulary(ctx: Context) -> list[str]:
    text = ctx.head.read_text(TEST_STRATEGY) or ""
    return markdown_list(text, RELATIONS_HEADING, RELATIONS_LEAD) or []


def rule_list(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    text = ctx.head.read_text(CONSTITUTION)
    items = markdown_list(text or "", SECTION_HEADING, SECTION_LEAD)
    if items is None:
        out.append(Finding(R_LIST, C_LIST, f"{CONSTITUTION} has no `{SECTION_HEADING}` / `{SECTION_LEAD}` list"))
    elif tuple(items) != SECTION_ITEMS:
        out.append(Finding(
            R_LIST, C_LIST,
            f"{CONSTITUTION} §4 `{SECTION_HEADING}` lists {items}, not {list(SECTION_ITEMS)}; the GOV-4-0N ids "
            "this set binds are positional, so the list and this set must move together",
        ))
    summaries = requirement_summaries(ctx)
    for rid, bullet in REQUIREMENTS.items():
        got = summaries.get(rid)
        if got is None or got.rstrip(";.").strip() != bullet:
            out.append(Finding(
                R_LIST, C_LIST,
                f"notes/plan/notes/PLAN_REQUIREMENTS.json {rid} summary is {got!r}, not {bullet!r}",
            ))
    if len(relation_vocabulary(ctx)) < 2:
        out.append(Finding(
            R_LIST, C_RELATIONS,
            f"{TEST_STRATEGY} `{RELATIONS_HEADING}` / `{RELATIONS_LEAD}` no longer lists the metamorphic "
            "relations GOV-4-02 names its tests by",
        ))
    return out


# ---------------------------------------------------------------------------
# GOV-4-01/02/03 — tests
# ---------------------------------------------------------------------------


def _entries(sem: dict, key: str, table: bool) -> list | None:
    value = sem.get(key)
    if not isinstance(value, list) or not value:
        return None
    if table:
        return value if all(isinstance(v, dict) for v in value) else None
    return value if all(isinstance(v, str) for v in value) else None


def _test_rule(ctx: Context, rule: str, key: str, names: dict[str, str], table: bool) -> list[Finding]:
    out: list[Finding] = []
    relations = [normalize(r) for r in relation_vocabulary(ctx)]
    for rec, sem, covered in semantic_records(ctx):
        if not covered:
            continue
        crates = _crates(ctx, covered)
        entries = _entries(sem, key, table)
        if entries is None:
            out.append(Finding(
                rule, names["named"],
                f"{rec.path}: `semantic.{key}` is missing, empty, or malformed; a semantic change over "
                f"{sorted(crates)} requires {key.replace('_', ' ')} ({CONSTITUTION} §4)",
            ))
            continue
        fresh_any = False
        exercised_any = False
        for entry in entries:
            ref = entry.get("test") if table else entry
            test, problem = resolve_test(ctx, ref)
            if test is None:
                out.append(Finding(rule, names["resolves"], f"{rec.path}: {problem}"))
                continue
            fresh_any |= test_is_fresh(ctx, test)
            test_crate = ctx.crate_of(test.path)
            if rule == R_DIFF:
                oracle, subject = entry.get("oracle"), entry.get("subject")
                members = ctx.workspace().members
                if not (isinstance(oracle, str) and isinstance(subject, str)) or oracle == subject \
                        or oracle not in members or subject not in members:
                    out.append(Finding(
                        rule, names["distinct"],
                        f"{rec.path}: {ref}: oracle {oracle!r} and subject {subject!r} must be two distinct "
                        "workspace crates — a differential test compares two implementations",
                    ))
                    continue
                file_text = ctx.head.read_text(test.path) or ""
                missing = [c for c in (oracle, subject) if not re.search(rf"\b{c.replace('-', '_')}\b", file_text)]
                if missing:
                    out.append(Finding(
                        rule, names["uses"],
                        f"{rec.path}: {ref}: {test.path} never names {missing} (as a Rust path), so it cannot "
                        "compare both implementations",
                    ))
                    continue
                exercised_any |= ctx.reaches(oracle, crates) or ctx.reaches(subject, crates)
            else:
                if rule == R_META:
                    relation = entry.get("relation")
                    if not isinstance(relation, str) or normalize(relation) not in relations:
                        out.append(Finding(
                            rule, names["registered"],
                            f"{rec.path}: {ref}: relation {relation!r} is not one of {TEST_STRATEGY} §3's "
                            f"metamorphic relations {relation_vocabulary(ctx)}",
                        ))
                        continue
                    if normalize(relation) not in normalize(test.attrs + "\n" + test.body):
                        out.append(Finding(
                            rule, names["stated"],
                            f"{rec.path}: {ref}: the test neither documents nor mentions the relation "
                            f"{relation!r}; the relation a test claims is stated at the test",
                        ))
                        continue
                exercised_any |= bool(test_crate) and ctx.reaches(test_crate, crates)
        if any(f.rule == rule and rec.path in f.message for f in out):
            continue
        if not exercised_any:
            out.append(Finding(
                rule, names["exercises"],
                f"{rec.path}: no test in `semantic.{key}` lives in, or reaches through its dependencies, a "
                f"changed crate {sorted(crates)}",
            ))
        if not fresh_any:
            out.append(Finding(
                rule, names["fresh"],
                f"{rec.path}: no test in `semantic.{key}` is new or has a changed body in this delta; a "
                "semantic change changes behaviour, and a test that did not move does not pin the new one",
            ))
    return out


def rule_reference(ctx: Context) -> list[Finding]:
    return _test_rule(ctx, R_REF, "reference_tests", REF, table=False)


def rule_metamorphic(ctx: Context) -> list[Finding]:
    return _test_rule(ctx, R_META, "metamorphic_tests", META, table=True)


def rule_differential(ctx: Context) -> list[Finding]:
    return _test_rule(ctx, R_DIFF, "differential_tests", DIFF, table=True)


# ---------------------------------------------------------------------------
# GOV-4-04 — updated schemas
# ---------------------------------------------------------------------------


def _schema_path(klass: str) -> str:
    return f"{delta_mod.SCHEMA_DIR}/{klass}.schema.json"


def rule_schemas(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    changed_schemas = set(ctx.changed_matching(delta_mod.is_schema_document))
    declared_all: set[str] = set()
    any_record = False
    for rec, sem, covered in semantic_records(ctx):
        if not covered:
            continue
        any_record = True
        table = sem.get("schemas")
        updated = table.get("updated") if isinstance(table, dict) else None
        if not isinstance(table, dict) or not isinstance(updated, list) or not all(isinstance(u, str) for u in updated):
            out.append(Finding(
                R_SCHEMA, C_SCHEMA_DECLARED,
                f"{rec.path}: `[semantic.schemas]` with an `updated` list is missing; a semantic change states "
                "which schema documents it updated, even when the answer is none",
            ))
            continue
        reason = table.get("reason")
        if not updated and (not isinstance(reason, str) or len(normalize(reason)) < MIN_REASON):
            out.append(Finding(
                R_SCHEMA, C_SCHEMA_DECLARED,
                f"{rec.path}: `semantic.schemas.updated` is empty and `reason` is missing or shorter than "
                f"{MIN_REASON} characters; `no schema changed` is a claim that carries its reason",
            ))
        declared_all.update(updated)
        for path in updated:
            if path not in changed_schemas or ctx.head.read_text(path) is None:
                out.append(Finding(
                    R_SCHEMA, C_SCHEMA_IN_DELTA,
                    f"{rec.path}: `semantic.schemas.updated` names {path}, which is not a schema document this "
                    "delta adds or changes",
                ))
        unaffected = table.get("unaffected") or {}
        if not isinstance(unaffected, dict):
            unaffected = {}
        classes_updated = {delta_mod.schema_class(p) for p in updated}
        for rel in covered:
            for klass in sorted(set(SCHEMA_CLASS_REF.findall(ctx.head.read_text(rel) or ""))):
                if klass in classes_updated:
                    continue
                why = unaffected.get(klass)
                if isinstance(why, str) and len(normalize(why)) >= MIN_REASON:
                    continue
                out.append(Finding(
                    R_SCHEMA, C_SCHEMA_BEARING,
                    f"{rec.path}: {rel} names schema class `{klass}`, which is neither in "
                    f"`semantic.schemas.updated` ({_schema_path(klass)}) nor in `semantic.schemas.unaffected` "
                    f"with a reason of at least {MIN_REASON} characters",
                ))
    if any_record:
        for path in sorted(changed_schemas - declared_all):
            out.append(Finding(
                R_SCHEMA, C_SCHEMA_UNDECLARED,
                f"{path} changed in a delta that carries a semantic change, and no review record lists it in "
                "`semantic.schemas.updated`",
            ))
    return out


# ---------------------------------------------------------------------------
# GOV-4-05 — migration note
# ---------------------------------------------------------------------------


def _epoch_advanced(ctx: Context) -> bool:
    for rel in ctx.changed_matching(delta_mod.is_schema_document):
        before = delta_mod.schema_epoch(ctx.base.read_text(rel) if ctx.base else None)
        after = delta_mod.schema_epoch(ctx.head.read_text(rel))
        if before is not None and after is not None and after > before:
            return True
    return False


def rule_migration(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    for rec, sem, covered in semantic_records(ctx):
        if not covered:
            continue
        table = sem.get("migration")
        if not isinstance(table, dict) or "verdict" not in table or "note" not in table:
            out.append(Finding(
                R_MIGRATION, C_MIG_PRESENT,
                f"{rec.path}: `[semantic.migration]` with `verdict` and `note` is missing",
            ))
            continue
        verdict = table["verdict"]
        if verdict not in COMPATIBILITY_VERDICTS:
            out.append(Finding(
                R_MIGRATION, C_MIG_VERDICT,
                f"{rec.path}: `semantic.migration.verdict` {verdict!r} is not one of {list(COMPATIBILITY_VERDICTS)} "
                "(plan §4.6)",
            ))
            continue
        note = table["note"]
        crates = _crates(ctx, covered)
        text = normalize(note) if isinstance(note, str) else ""
        if len(text) < MIN_NOTE or not any(c in text or c.replace("continuum-", "") in text.split() for c in crates):
            out.append(Finding(
                R_MIGRATION, C_MIG_NAMES,
                f"{rec.path}: `semantic.migration.note` must be at least {MIN_NOTE} characters and name a changed "
                f"crate {sorted(crates)}",
            ))
        if verdict == "Incompatible" and not _epoch_advanced(ctx):
            out.append(Finding(
                R_MIGRATION, C_MIG_EPOCH,
                f"{rec.path}: verdict `Incompatible` with no schema epoch advance in the delta; every breaking "
                f"change increments the epoch ({CONSTITUTION} §1, GOV-1-09)",
            ))
    return out


# ---------------------------------------------------------------------------
# GOV-4-06 — security review
# ---------------------------------------------------------------------------


def rule_security(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    for rec, sem, covered in semantic_records(ctx):
        if not covered:
            continue
        table = sem.get("security_review")
        reviewer = table.get("reviewer") if isinstance(table, dict) else None
        review = table.get("review") if isinstance(table, dict) else None
        if not isinstance(reviewer, str) or not reviewer.strip() or not isinstance(review, str) \
                or not REVIEW_ID_RE.fullmatch(review):
            out.append(Finding(
                R_SECURITY, C_SEC_PRESENT,
                f"{rec.path}: `[semantic.security_review]` must name a `reviewer` and a `review` id",
            ))
            continue
        author = (rec.data or {}).get("change", {}).get("author")
        if isinstance(author, str) and normalize(author) == normalize(reviewer):
            out.append(Finding(
                R_SECURITY, C_SEC_INDEPENDENT,
                f"{rec.path}: the security reviewer {reviewer!r} is the change author; no self-certification "
                "(INV-004)",
            ))
        verdict = table.get("verdict")
        if verdict != "approved":
            out.append(Finding(
                R_SECURITY, C_SEC_APPROVED,
                f"{rec.path}: security review verdict is {verdict!r}; only `approved` (of {list(REVIEW_VERDICTS)}) "
                "lets a semantic change land",
            ))
    return out


# ---------------------------------------------------------------------------
# The set
# ---------------------------------------------------------------------------

_TEST_BOUNDARY = (
    "Proves the named test exists at head, is a non-ignored `#[test]` that `just check` runs, lives in or "
    "reaches a changed crate, and that at least one named test is new or changed in the delta. It does not "
    "judge whether the assertions pin the new behaviour."
)

SET = ObligationSet(
    name="gov-4-semantic",
    title="GOV §4 semantic changes — reference, metamorphic, differential tests, schemas, migration, security",
    source=f"{CONSTITUTION} §4 Review requirements / Semantic changes",
    requirements=REQUIREMENTS,
    evidence="tools/governance/evidence/gov-4-semantic.json",
    record_keys={
        "semantic": (
            "covers", "reference_tests", "metamorphic_tests", "differential_tests",
            "schemas", "migration", "security_review",
        )
    },
    rules=(
        Rule(
            R_RECORD, ALL, (C_HAS_RECORD, C_COVERS), DELTA,
            "Every semantic change in the delta (GOV-1-08's predicate: a semantic-tier source whose code "
            "changed or that the delta added) is covered by a review record the delta adds or modifies; every "
            "`covers` entry matches a semantic change.",
            "`Semantic change` inherits GOV-1-08's approximation: comment-stripped code of a semantic-tier "
            "`src/` file. A test-only or boundary-crate change is not a semantic change.",
            rule_record,
        ),
        Rule(
            R_LIST, ALL, (C_LIST, C_RELATIONS), TREE,
            "docs/12 §4 `Semantic changes / Require:` still lists the seven items in order, the generated "
            "registry's GOV-4-01..06 summaries still name items 1..6, and docs/19 §3 still lists the "
            "metamorphic relations GOV-4-02 cites.",
            None,
            rule_list,
        ),
        Rule(
            R_REF, ("GOV-4-01",), tuple(REF.values()), DELTA,
            "`semantic.reference_tests` names at least one `crates/<c>/(tests|src)/<f>.rs::<fn>` for each "
            "covered change; each resolves to a non-ignored `#[test]`; one lives in or depends on a changed "
            "crate; one is new or changed in the delta.",
            _TEST_BOUNDARY,
            rule_reference,
        ),
        Rule(
            R_META, ("GOV-4-02",), tuple(META.values()), DELTA,
            "`semantic.metamorphic_tests` names tests with a `relation` from docs/19 §3's list, read live; the "
            "relation is stated in the test's doc comment or body; plus the reference-test conditions.",
            _TEST_BOUNDARY + " The relation text proves the claimed relation is named at the test, not that "
            "the test checks it.",
            rule_metamorphic,
        ),
        Rule(
            R_DIFF, ("GOV-4-03",), tuple(DIFF.values()), DELTA,
            "`semantic.differential_tests` names tests with an `oracle` and a `subject`, two distinct workspace "
            "crates both named as Rust paths in the test file, one of which is or reaches a changed crate; plus "
            "the resolve and freshness conditions.",
            _TEST_BOUNDARY + " Only in-workspace pairs are recognized; docs/19 §5's external oracles (TLC, "
            "Kani, solvers) are out of scope until a harness for them exists.",
            rule_differential,
        ),
        Rule(
            R_SCHEMA, ("GOV-4-04",),
            (C_SCHEMA_DECLARED, C_SCHEMA_IN_DELTA, C_SCHEMA_UNDECLARED, C_SCHEMA_BEARING), DELTA,
            "`[semantic.schemas]` states `updated` (with a reason when empty); every listed document changed in "
            "the delta; every schema document the delta changes is listed; every schema class a changed source "
            "names is updated or declared unaffected with a reason.",
            "A schema class is recognized only when the source names its URI (`continuum.dev/schema/…json`); a "
            "change that alters an artifact's shape through an unnamed type is caught only by review. Epoch "
            "discipline for the update is GOV-1-09's delta rule.",
            rule_schemas,
        ),
        Rule(
            R_MIGRATION, ("GOV-4-05",), (C_MIG_PRESENT, C_MIG_VERDICT, C_MIG_NAMES, C_MIG_EPOCH), DELTA,
            "`[semantic.migration]` carries a typed verdict from plan §4.6 (`Preserved | Revalidate | "
            "Incompatible`) and a note naming a changed crate; `Incompatible` requires a schema epoch advance "
            "in the same delta.",
            "Proves a typed note exists and is consistent with the epoch; whether the verdict is the correct one "
            "is a review judgement.",
            rule_migration,
        ),
        Rule(
            R_SECURITY, ("GOV-4-06",), (C_SEC_PRESENT, C_SEC_INDEPENDENT, C_SEC_APPROVED), DELTA,
            "`[semantic.security_review]` names a reviewer other than the change author, a review id, and the "
            "verdict `approved`.",
            "The record is a pointer, not the review: Seal keeps review state outside the repository, so no "
            "committed artifact lets this check confirm that the named review exists, covers this change, or "
            "carries that verdict. GOV-4-06 is therefore NOT delivered by this rule.",
            rule_security,
        ),
    ),
    undelivered={
        "GOV-4-06": (
            "The record names a reviewer, a review id, and a verdict, and the check holds them to "
            "independence and approval, but Seal keeps review state outside the repository: no committed "
            "artifact lets a checker confirm the review exists, covers this change, and approved it. Until "
            "review verdicts are committed (or attested) where a checker can read them, the record is a "
            "claim, not evidence."
        ),
    },
    notes=(
        "GOV-4-07 (claim impact) is the seventh §4 `Semantic changes` item and sibling Bone bn-2tm3's; it "
        "extends `[semantic]` from its own obligation file.",
        "GOV-4-06 is enforced at the record level only; see its rule's boundary.",
    ),
)
