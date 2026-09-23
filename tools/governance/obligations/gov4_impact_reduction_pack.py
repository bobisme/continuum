"""GOV-4-07, GOV-4-08…GOV-4-11, GOV-4-12 — claim impact, POR/reduction changes,
and the first Pack-changes obligation (bn-2tm3).

Source: `notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md` §4 "Review
requirements":

- "Semantic changes / Require:" seventh item, claim impact (`GOV-4-07`). It
  extends the `[semantic]` review-record section `gov4_semantic.py` (bn-37b1)
  already owns, adding `claim_impact` — the allowed keys of a shared section
  are the union of every set that declares them.
- "POR/reduction changes / Require:" (`GOV-4-08`…`GOV-4-11`): no-reduction
  differential campaign, mutation of the dependence relation,
  liveness/property-class analysis, performance report. A new change class,
  triggered by a source change to the partial-order reduction engine
  (`continuum-engine-dpor`, docs/01 §7.2), carrying its own record section,
  `[por_reduction]`.
- "Pack changes / Require:" (`GOV-4-12` only; the four siblings — host/Lab
  conformance, fault coverage, independence review, version bump — are a
  later Bone's). A domain pack is a `continuum-effects-*` crate (RFC 0002); a
  source change to one of them is a pack change, carrying its own record
  section, `[pack]`.

A change with a POR/reduction change ships:

    [por_reduction]
    covers = ["crates/continuum-engine-dpor/src/"]            # paths or dir prefixes
    no_reduction_tests = [
      { test = "crates/continuum-engine-dpor/tests/x.rs::vs_unreduced", oracle = "continuum-engine-explicit", subject = "continuum-engine-dpor" },
    ]
    dependence_mutation_tests = ["crates/continuum-engine-dpor/tests/x.rs::declares_independent_conflict"]

    [por_reduction.property_analysis]
    tier = "Tier A"                                            # RFC 0014 "Property preservation tiers"
    note = "what continuum-engine-dpor's reduction preserves for this tier"

    [por_reduction.performance]
    metric = "reduction ratio by observer"                     # RFC 0014 "Evaluation" metrics
    note = "what changed for continuum-engine-dpor and by how much"

A change with a pack change ships:

    [pack]
    covers = ["crates/continuum-effects-network/src/"]

    [pack.fidelity_profile]
    profile = "adversarial-envelope"                           # RFC 0002 "Required profiles"
    note = "why continuum-effects-network claims this profile"

A change with a semantic change that also needs `GOV-4-07` extends the
existing `[semantic]` table:

    [semantic.claim_impact]
    claims = ["C005", "C014"]                                  # docs/18 claim ids, or none
    reason = "why no docs/18 claim is affected"                # owed when `claims` is empty
"""

from __future__ import annotations

import re

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

CONSTITUTION = "notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md"
TEST_STRATEGY = "notes/plan/docs/19_TEST_STRATEGY.md"
DOCS_18 = "notes/plan/docs/18_CLAIMS_MATRIX.md"
RFC_0014 = "notes/plan/rfcs/0014-observer-indexed-dpor.md"
RFC_0002 = "notes/plan/rfcs/0002-controlled-effects-and-domain-packs.md"

POR_HEADING = "### POR/reduction changes"
PACK_HEADING = "### Pack changes"
REQUIRE_LEAD = "Require:"
MUTATION_HEADING = "### Semantic engine"
TIER_HEADING = "## Property preservation tiers"
EVAL_HEADING = "## Evaluation"
EVAL_LEAD = "Metrics:"
PACK_PROFILES_HEADING = "## Required profiles"

# docs/01 §7.2's partial-order reduction engine: the only crate a POR/reduction
# change is about. RFC 0002's four standard packs: declared data plus a
# normalized adapter, deliberately excluded from `check_code_policy.SEMANTIC_CORE`.
POR_REDUCTION_CORE = frozenset({"continuum-engine-dpor"})
PACK_CORE = frozenset({
    "continuum-effects-network",
    "continuum-effects-process",
    "continuum-effects-storage",
    "continuum-effects-time",
})

REQUIREMENTS = {
    "GOV-4-07": "claim impact",
    "GOV-4-08": "no-reduction differential campaign",
    "GOV-4-09": "mutation of dependence relation",
    "GOV-4-10": "liveness/property-class analysis",
    "GOV-4-11": "performance report",
    "GOV-4-12": "fidelity profile",
}
POR_SECTION_ITEMS = (
    "no-reduction differential campaign",
    "mutation of dependence relation",
    "liveness/property-class analysis",
    "performance report",
)
PACK_SECTION_ITEMS = (
    "fidelity profile",
    "host/Lab conformance",
    "fault coverage",
    "independence review",
    "version bump",
)

CLAIM_MIN_REASON = 24
NOTE_MIN = 40
CLAIM_ID_RE = re.compile(r"\bC\d{3}\b")
CLAIM_ROW_RE = re.compile(r"^\|\s*(C\d{3})\s*\|", re.M)
TIER_RE = re.compile(r"^(Tier [A-D])\b")

# ---------------------------------------------------------------------------
# Rule and check names
# ---------------------------------------------------------------------------

R_CLAIM = "semantic-claim-impact"
C_CLAIM_DECLARED = "claim-impact-declared"
C_CLAIM_RESOLVES = "claim-impact-ids-resolve"

R_POR_RECORD = "por-review-record"
C_POR_HAS_RECORD = "por-change-has-review-record"
C_POR_COVERS = "por-review-record-covers-a-por-change"

R_NO_REDUCTION = "no-reduction-differential"
C_NR_NAMED = "no-reduction-tests-named"
C_NR_RESOLVES = "no-reduction-test-resolves"
C_NR_DISTINCT = "no-reduction-pair-distinct-implementations"
C_NR_USES = "no-reduction-test-uses-both-implementations"
C_NR_FRESH = "no-reduction-test-is-fresh"

R_MUTATION = "dependence-mutation"
C_MUT_NAMED = "dependence-mutation-tests-named"
C_MUT_RESOLVES = "dependence-mutation-test-resolves"
C_MUT_EXERCISES = "dependence-mutation-test-exercises-the-change"
C_MUT_FRESH = "dependence-mutation-test-is-fresh"
C_MUT_STATED = "dependence-mutation-named-at-the-test"

R_PROPERTY = "property-class-analysis"
C_PROP_PRESENT = "property-analysis-present"
C_PROP_TIER = "property-tier-registered"
C_PROP_NAMES = "property-analysis-names-the-change"

R_PERF = "performance-report"
C_PERF_PRESENT = "performance-report-present"
C_PERF_METRIC = "performance-metric-registered"
C_PERF_NAMES = "performance-report-names-the-change"

R_PACK_RECORD = "pack-review-record"
C_PACK_HAS_RECORD = "pack-change-has-review-record"
C_PACK_COVERS = "pack-review-record-covers-a-pack-change"

R_PACK_FIDELITY = "pack-fidelity-profile"
C_FID_PRESENT = "pack-fidelity-profile-declared"
C_FID_REGISTERED = "pack-fidelity-profile-registered"
C_FID_NAMES = "pack-fidelity-profile-names-the-change"

R_LIST = "gov4-07-12-review-list"
C_LIST = "gov4-07-12-list-registered"
C_VOCAB = "gov4-07-12-vocabulary-present"

ALL = tuple(REQUIREMENTS)
POR_IDS = ("GOV-4-08", "GOV-4-09", "GOV-4-10", "GOV-4-11")


# ---------------------------------------------------------------------------
# Shared helpers
# ---------------------------------------------------------------------------


def _covers(entry: str, rel: str) -> bool:
    return rel == entry or (entry.endswith("/") and rel.startswith(entry))


def _crates(ctx: Context, paths: list[str]) -> set[str]:
    return {c for c in (ctx.crate_of(p) for p in paths) if c}


def _names_a_crate(text: str, crates: set[str]) -> bool:
    return any(c in text or c.replace("continuum-", "") in text.split() for c in crates)


def _tier_source_changes(ctx: Context, tier: frozenset[str]) -> list[str]:
    """Source files of a crate in `tier` whose comment-stripped code changed or was added.

    The same shape as `Context.semantic_changes()`, restricted to a narrower
    crate set than `check_code_policy.SEMANTIC_CORE` — POR/reduction and pack
    changes are not semantic-tier changes (packs are `BOUNDARY` tier by
    design), so they need their own predicate, not `ctx.semantic_changes()`.
    """
    out: list[str] = []
    for rel in ctx.changed_matching(lambda p: bool(
        (m := delta_mod.SEMANTIC_SOURCE_RE.fullmatch(p)) and m.group(1) in tier
    )):
        head = ctx.head.read_text(rel)
        if head is None:
            continue
        base = ctx.base.read_text(rel) if ctx.base else None
        if base is None or delta_mod.semantic_text(base) != delta_mod.semantic_text(head):
            out.append(rel)
    return out


def por_changes(ctx: Context) -> list[str]:
    return _tier_source_changes(ctx, POR_REDUCTION_CORE)


def pack_changes(ctx: Context) -> list[str]:
    return _tier_source_changes(ctx, PACK_CORE)


def _section_records(ctx: Context, section: str, changes: list[str]) -> list[tuple[Record, dict, list[str]]]:
    """(record, its `[section]` table, the `changes` it covers) for every record naming the section."""
    out = []
    for rec in ctx.records_in_delta():
        if rec.data is None or not isinstance(rec.data.get(section), dict):
            continue
        table = rec.data[section]
        entries = (
            [e for e in table.get("covers", []) if isinstance(e, str)]
            if isinstance(table.get("covers"), list)
            else []
        )
        covered = [c for c in changes if any(_covers(e, c) for e in entries)]
        out.append((rec, table, covered))
    return out


def _semantic_records(ctx: Context) -> list[tuple[Record, dict, list[str]]]:
    """Records covering a semantic change (GOV-1-08's predicate), `[semantic]` only.

    A local copy of `gov4_semantic.semantic_records`'s covered-only view: sets
    are independent files and do not import one another's internals.
    """
    return [
        (rec, sem, covered)
        for rec, sem, covered in _section_records(ctx, "semantic", ctx.semantic_changes())
        if covered
    ]


def por_records(ctx: Context) -> list[tuple[Record, dict, list[str]]]:
    return _section_records(ctx, "por_reduction", por_changes(ctx))


def pack_records(ctx: Context) -> list[tuple[Record, dict, list[str]]]:
    return _section_records(ctx, "pack", pack_changes(ctx))


def _record_coverage(
    ctx: Context,
    records: list[tuple[Record, dict, list[str]]],
    changes: list[str],
    rule: str,
    c_has_record: str,
    c_covers: str,
    key: str,
    class_name: str,
    requires: str,
) -> list[Finding]:
    out: list[Finding] = []
    covered_all = {c for _, _, cs in records for c in cs}
    for rel in changes:
        if rel not in covered_all:
            out.append(Finding(
                rule, c_has_record,
                f"{rel} is a {class_name} ({ctx.base_label} -> {ctx.head_label}) and no review record the "
                f"delta adds or modifies under tools/governance/reviews/ covers it in `[{key}].covers`; "
                f"{CONSTITUTION} §4 requires {requires} for it",
            ))
    for rec, table, _ in records:
        entries = table.get("covers")
        if not isinstance(entries, list) or not entries or not all(isinstance(e, str) for e in entries):
            out.append(Finding(rule, c_covers, f"{rec.path}: `{key}.covers` must be a non-empty list of paths"))
            continue
        for entry in entries:
            if not any(_covers(entry, c) for c in changes):
                out.append(Finding(
                    rule, c_covers,
                    f"{rec.path}: `{key}.covers` entry {entry!r} matches no {class_name} in the delta; a record "
                    "claims only what the change actually touched",
                ))
    return out


# ---------------------------------------------------------------------------
# Vocabularies read live
# ---------------------------------------------------------------------------


def claim_registry_ids(ctx: Context) -> set[str]:
    return set(CLAIM_ROW_RE.findall(ctx.head.read_text(DOCS_18) or ""))


def mutation_vocabulary(ctx: Context) -> list[str]:
    text = ctx.head.read_text(TEST_STRATEGY) or ""
    return markdown_list(text, MUTATION_HEADING) or []


def property_tier_vocabulary(ctx: Context) -> list[str]:
    text = ctx.head.read_text(RFC_0014) or ""
    out = []
    for item in markdown_list(text, TIER_HEADING) or []:
        m = TIER_RE.match(item)
        if m:
            out.append(m.group(1))
    return out


def performance_metric_vocabulary(ctx: Context) -> list[str]:
    text = ctx.head.read_text(RFC_0014) or ""
    return markdown_list(text, EVAL_HEADING, EVAL_LEAD) or []


def pack_profile_vocabulary(ctx: Context) -> list[str]:
    text = ctx.head.read_text(RFC_0002) or ""
    idx = text.find(PACK_PROFILES_HEADING)
    if idx == -1:
        return []
    m = re.search(r"```text\n(.*?)\n```", text[idx:], re.S)
    if not m:
        return []
    return [line.strip() for line in m.group(1).splitlines() if line.strip()]


# ---------------------------------------------------------------------------
# GOV-4-07 — claim impact (extends `[semantic]`)
# ---------------------------------------------------------------------------


def rule_claim_impact(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    ids = claim_registry_ids(ctx)
    for rec, sem, _covered in _semantic_records(ctx):
        table = sem.get("claim_impact")
        claims = table.get("claims") if isinstance(table, dict) else None
        if not isinstance(table, dict) or not isinstance(claims, list) or not all(isinstance(c, str) for c in claims):
            out.append(Finding(
                R_CLAIM, C_CLAIM_DECLARED,
                f"{rec.path}: `[semantic.claim_impact]` with a `claims` list is missing, empty of type, or "
                f"malformed; a semantic change states which {DOCS_18} claims it bears on, even when the answer "
                f"is none ({CONSTITUTION} §4)",
            ))
            continue
        reason = table.get("reason")
        if not claims and (not isinstance(reason, str) or len(normalize(reason)) < CLAIM_MIN_REASON):
            out.append(Finding(
                R_CLAIM, C_CLAIM_DECLARED,
                f"{rec.path}: `semantic.claim_impact.claims` is empty and `reason` is missing or shorter than "
                f"{CLAIM_MIN_REASON} characters; `no claim is affected` is a claim that carries its reason",
            ))
        for cid in claims:
            if cid not in ids:
                out.append(Finding(
                    R_CLAIM, C_CLAIM_RESOLVES,
                    f"{rec.path}: `semantic.claim_impact.claims` names {cid!r}, which resolves to no row in "
                    f"{DOCS_18}",
                ))
    return out


# ---------------------------------------------------------------------------
# GOV-4-08…GOV-4-11 — POR/reduction changes
# ---------------------------------------------------------------------------


def rule_por_record(ctx: Context) -> list[Finding]:
    return _record_coverage(
        ctx, por_records(ctx), por_changes(ctx), R_POR_RECORD, C_POR_HAS_RECORD, C_POR_COVERS,
        "por_reduction", "POR/reduction change",
        "a no-reduction differential campaign, mutation of the dependence relation, a liveness/property-class "
        "analysis, and a performance report",
    )


def rule_no_reduction(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    for rec, table, covered in por_records(ctx):
        if not covered:
            continue
        crates = _crates(ctx, covered)
        entries = table.get("no_reduction_tests")
        if not isinstance(entries, list) or not entries or not all(isinstance(e, dict) for e in entries):
            out.append(Finding(
                R_NO_REDUCTION, C_NR_NAMED,
                f"{rec.path}: `por_reduction.no_reduction_tests` is missing, empty, or malformed; a "
                f"POR/reduction change over {sorted(crates)} requires a no-reduction differential campaign "
                f"({CONSTITUTION} §4)",
            ))
            continue
        fresh_any = False
        bad = False
        for entry in entries:
            ref = entry.get("test")
            test, problem = resolve_test(ctx, ref)
            if test is None:
                out.append(Finding(R_NO_REDUCTION, C_NR_RESOLVES, f"{rec.path}: {problem}"))
                bad = True
                continue
            oracle, subject = entry.get("oracle"), entry.get("subject")
            members = ctx.workspace().members
            if (
                not (isinstance(oracle, str) and isinstance(subject, str))
                or oracle == subject
                or oracle not in members
                or subject not in members
                or subject not in POR_REDUCTION_CORE
                or oracle in POR_REDUCTION_CORE
            ):
                out.append(Finding(
                    R_NO_REDUCTION, C_NR_DISTINCT,
                    f"{rec.path}: {ref}: oracle {oracle!r} and subject {subject!r} must be two distinct "
                    f"workspace crates, with `subject` the reduction engine {sorted(POR_REDUCTION_CORE)} and "
                    "`oracle` an unreduced implementation — a no-reduction campaign holds the reduction engine "
                    "fixed as the subject under test",
                ))
                bad = True
                continue
            file_text = ctx.head.read_text(test.path) or ""
            missing = [c for c in (oracle, subject) if not re.search(rf"\b{c.replace('-', '_')}\b", file_text)]
            if missing:
                out.append(Finding(
                    R_NO_REDUCTION, C_NR_USES,
                    f"{rec.path}: {ref}: {test.path} never names {missing} (as a Rust path), so it cannot "
                    "compare both implementations",
                ))
                bad = True
                continue
            fresh_any |= test_is_fresh(ctx, test)
        if bad:
            continue
        if not fresh_any:
            out.append(Finding(
                R_NO_REDUCTION, C_NR_FRESH,
                f"{rec.path}: no test in `por_reduction.no_reduction_tests` is new or has a changed body in "
                "this delta",
            ))
    return out


def rule_dependence_mutation(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    vocab = [normalize(v) for v in mutation_vocabulary(ctx)]
    for rec, table, covered in por_records(ctx):
        if not covered:
            continue
        crates = _crates(ctx, covered)
        entries = table.get("dependence_mutation_tests")
        if not isinstance(entries, list) or not entries or not all(isinstance(e, str) for e in entries):
            out.append(Finding(
                R_MUTATION, C_MUT_NAMED,
                f"{rec.path}: `por_reduction.dependence_mutation_tests` is missing, empty, or malformed; a "
                f"POR/reduction change over {sorted(crates)} requires mutation of the dependence relation "
                f"({CONSTITUTION} §4)",
            ))
            continue
        fresh_any = exercised_any = named_any = False
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
            text = normalize(test.attrs + "\n" + test.body)
            named_any |= any(v in text for v in vocab)
        if bad:
            continue
        if not exercised_any:
            out.append(Finding(
                R_MUTATION, C_MUT_EXERCISES,
                f"{rec.path}: no test in `por_reduction.dependence_mutation_tests` lives in, or reaches through "
                f"its dependencies, a changed crate {sorted(crates)}",
            ))
        if not fresh_any:
            out.append(Finding(
                R_MUTATION, C_MUT_FRESH,
                f"{rec.path}: no test in `por_reduction.dependence_mutation_tests` is new or has a changed body "
                "in this delta",
            ))
        if not named_any:
            out.append(Finding(
                R_MUTATION, C_MUT_STATED,
                f"{rec.path}: no test in `por_reduction.dependence_mutation_tests` names one of {TEST_STRATEGY} "
                f"§4 `{MUTATION_HEADING}`'s mutation kinds {mutation_vocabulary(ctx)}; a mutation test names the "
                "mutation it applies",
            ))
    return out


def rule_property_class(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    tiers = [normalize(t) for t in property_tier_vocabulary(ctx)]
    for rec, table, covered in por_records(ctx):
        if not covered:
            continue
        crates = _crates(ctx, covered)
        analysis = table.get("property_analysis")
        if not isinstance(analysis, dict) or "tier" not in analysis or "note" not in analysis:
            out.append(Finding(
                R_PROPERTY, C_PROP_PRESENT,
                f"{rec.path}: `[por_reduction.property_analysis]` with `tier` and `note` is missing; a "
                f"POR/reduction change requires a liveness/property-class analysis ({CONSTITUTION} §4)",
            ))
            continue
        tier = analysis["tier"]
        if not isinstance(tier, str) or normalize(tier) not in tiers:
            out.append(Finding(
                R_PROPERTY, C_PROP_TIER,
                f"{rec.path}: `por_reduction.property_analysis.tier` {tier!r} is not one of {RFC_0014}'s "
                f"property preservation tiers {property_tier_vocabulary(ctx)}",
            ))
            continue
        note = analysis["note"]
        text = normalize(note) if isinstance(note, str) else ""
        if len(text) < NOTE_MIN or not _names_a_crate(text, crates):
            out.append(Finding(
                R_PROPERTY, C_PROP_NAMES,
                f"{rec.path}: `por_reduction.property_analysis.note` must be at least {NOTE_MIN} characters and "
                f"name a changed crate {sorted(crates)}",
            ))
    return out


def rule_performance(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    metrics = [normalize(m) for m in performance_metric_vocabulary(ctx)]
    for rec, table, covered in por_records(ctx):
        if not covered:
            continue
        crates = _crates(ctx, covered)
        perf = table.get("performance")
        if not isinstance(perf, dict) or "metric" not in perf or "note" not in perf:
            out.append(Finding(
                R_PERF, C_PERF_PRESENT,
                f"{rec.path}: `[por_reduction.performance]` with `metric` and `note` is missing; a POR/reduction "
                f"change requires a performance report ({CONSTITUTION} §4)",
            ))
            continue
        metric = perf["metric"]
        if not isinstance(metric, str) or normalize(metric) not in metrics:
            out.append(Finding(
                R_PERF, C_PERF_METRIC,
                f"{rec.path}: `por_reduction.performance.metric` {metric!r} is not one of {RFC_0014}'s "
                f"`{EVAL_HEADING}` metrics {performance_metric_vocabulary(ctx)}",
            ))
            continue
        note = perf["note"]
        text = normalize(note) if isinstance(note, str) else ""
        if len(text) < NOTE_MIN or not _names_a_crate(text, crates):
            out.append(Finding(
                R_PERF, C_PERF_NAMES,
                f"{rec.path}: `por_reduction.performance.note` must be at least {NOTE_MIN} characters and name a "
                f"changed crate {sorted(crates)}",
            ))
    return out


# ---------------------------------------------------------------------------
# GOV-4-12 — pack changes (fidelity profile)
# ---------------------------------------------------------------------------


def rule_pack_record(ctx: Context) -> list[Finding]:
    return _record_coverage(
        ctx, pack_records(ctx), pack_changes(ctx), R_PACK_RECORD, C_PACK_HAS_RECORD, C_PACK_COVERS,
        "pack", "pack change", "a fidelity profile",
    )


def rule_pack_fidelity(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    profiles = [normalize(p) for p in pack_profile_vocabulary(ctx)]
    for rec, table, covered in pack_records(ctx):
        if not covered:
            continue
        crates = _crates(ctx, covered)
        fidelity = table.get("fidelity_profile")
        if not isinstance(fidelity, dict) or "profile" not in fidelity or "note" not in fidelity:
            out.append(Finding(
                R_PACK_FIDELITY, C_FID_PRESENT,
                f"{rec.path}: `[pack.fidelity_profile]` with `profile` and `note` is missing; a pack change "
                f"requires a fidelity profile ({CONSTITUTION} §4)",
            ))
            continue
        profile = fidelity["profile"]
        if not isinstance(profile, str) or normalize(profile) not in profiles:
            out.append(Finding(
                R_PACK_FIDELITY, C_FID_REGISTERED,
                f"{rec.path}: `pack.fidelity_profile.profile` {profile!r} is not one of {RFC_0002}'s "
                f"`{PACK_PROFILES_HEADING}` {pack_profile_vocabulary(ctx)}",
            ))
            continue
        note = fidelity["note"]
        text = normalize(note) if isinstance(note, str) else ""
        if len(text) < NOTE_MIN or not _names_a_crate(text, crates):
            out.append(Finding(
                R_PACK_FIDELITY, C_FID_NAMES,
                f"{rec.path}: `pack.fidelity_profile.note` must be at least {NOTE_MIN} characters and name a "
                f"changed pack crate {sorted(crates)}",
            ))
    return out


# ---------------------------------------------------------------------------
# Tree rule: the source lists and the vocabularies still say what the ids bind
# ---------------------------------------------------------------------------


def rule_list(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    text = ctx.head.read_text(CONSTITUTION) or ""
    por_items = markdown_list(text, POR_HEADING, REQUIRE_LEAD)
    if por_items is None or tuple(por_items) != POR_SECTION_ITEMS:
        out.append(Finding(
            R_LIST, C_LIST,
            f"{CONSTITUTION} §4 `{POR_HEADING}` / `{REQUIRE_LEAD}` lists {por_items}, not "
            f"{list(POR_SECTION_ITEMS)}; the GOV-4-08..11 ids this set binds are positional",
        ))
    pack_items = markdown_list(text, PACK_HEADING, REQUIRE_LEAD)
    if pack_items is None or tuple(pack_items) != PACK_SECTION_ITEMS:
        out.append(Finding(
            R_LIST, C_LIST,
            f"{CONSTITUTION} §4 `{PACK_HEADING}` / `{REQUIRE_LEAD}` lists {pack_items}, not "
            f"{list(PACK_SECTION_ITEMS)}; GOV-4-12 (fidelity profile) is the first, positional item",
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
    if not claim_registry_ids(ctx):
        out.append(Finding(
            R_LIST, C_VOCAB,
            f"{DOCS_18} has no `C\\d{{3}}` claim rows; GOV-4-07's `semantic.claim_impact.claims` ids resolve "
            "against it",
        ))
    if len(property_tier_vocabulary(ctx)) < 2:
        out.append(Finding(
            R_LIST, C_VOCAB,
            f"{RFC_0014} `{TIER_HEADING}` no longer lists property preservation tiers GOV-4-10 types against",
        ))
    if len(performance_metric_vocabulary(ctx)) < 2:
        out.append(Finding(
            R_LIST, C_VOCAB,
            f"{RFC_0014} `{EVAL_HEADING}` / `{EVAL_LEAD}` no longer lists the metrics GOV-4-11 names its report "
            "by",
        ))
    if len(mutation_vocabulary(ctx)) < 2:
        out.append(Finding(
            R_LIST, C_VOCAB,
            f"{TEST_STRATEGY} §4 `{MUTATION_HEADING}` no longer lists the mutation kinds GOV-4-09 names its "
            "tests by",
        ))
    if len(pack_profile_vocabulary(ctx)) < 2:
        out.append(Finding(
            R_LIST, C_VOCAB,
            f"{RFC_0002} `{PACK_PROFILES_HEADING}` no longer lists the fidelity profiles GOV-4-12 types "
            "against",
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
    "reaches a changed crate, and that at least one named test is new or changed in the delta. It does not "
    "judge whether the assertions pin the new behaviour."
)
_NOTE_BOUNDARY = (
    "Proves a typed field exists, is one of the registered vocabulary, and a note of minimum length names a "
    "changed crate. Whether the tier, metric, or profile is the correct one, or the note is accurate, is a "
    "review judgement."
)

SET = ObligationSet(
    name="gov-4-impact-reduction-pack",
    title=(
        "GOV §4 claim impact, POR/reduction changes, and the first pack-changes obligation — GOV-4-07..12"
    ),
    source=f"{CONSTITUTION} §4 Review requirements",
    requirements=REQUIREMENTS,
    evidence="tools/governance/evidence/gov-4-impact-reduction-pack.json",
    record_keys={
        "semantic": ("claim_impact",),
        "por_reduction": ("covers", "no_reduction_tests", "dependence_mutation_tests", "property_analysis", "performance"),
        "pack": ("covers", "fidelity_profile"),
    },
    rules=(
        Rule(
            R_CLAIM, ("GOV-4-07",), (C_CLAIM_DECLARED, C_CLAIM_RESOLVES), DELTA,
            "A semantic change's review record (GOV-1-08's predicate, reused from `gov4_semantic`) states, in "
            "`[semantic.claim_impact]`, which docs/18 claim ids it bears on, with a reason when there are none; "
            "every named claim id resolves to a docs/18 row.",
            "Coverage of the semantic change itself is `gov4_semantic`'s `semantic-review-record` rule; this "
            "rule only proves the extension field on a record that already covers one.",
            rule_claim_impact,
        ),
        Rule(
            R_POR_RECORD, POR_IDS, (C_POR_HAS_RECORD, C_POR_COVERS), DELTA,
            "Every POR/reduction change (a `continuum-engine-dpor` source file whose comment-stripped code "
            "changed or that the delta added) is covered by a review record the delta adds or modifies; every "
            "`por_reduction.covers` entry matches such a change.",
            None,
            rule_por_record,
        ),
        Rule(
            R_NO_REDUCTION, ("GOV-4-08",),
            (C_NR_NAMED, C_NR_RESOLVES, C_NR_DISTINCT, C_NR_USES, C_NR_FRESH), DELTA,
            "`por_reduction.no_reduction_tests` names at least one differential test pairing the reduction "
            "engine (`subject`) against a distinct, unreduced workspace crate (`oracle`), both named as Rust "
            "paths in the test file; the test resolves and is new or changed.",
            _TEST_BOUNDARY + " Only in-workspace pairs are recognized. `subject` is pinned to "
            "`POR_REDUCTION_CORE`, which today has one member, so a passing pair always reaches the change; "
            "the rule does not carry a separate `exercises` check for that reason.",
            rule_no_reduction,
        ),
        Rule(
            R_MUTATION, ("GOV-4-09",),
            (C_MUT_NAMED, C_MUT_RESOLVES, C_MUT_EXERCISES, C_MUT_FRESH, C_MUT_STATED), DELTA,
            "`por_reduction.dependence_mutation_tests` names at least one test that resolves, reaches the "
            "change, is new or changed, and names one of docs/19 §4's semantic-engine mutation kinds in its "
            "doc comment or body.",
            _TEST_BOUNDARY + " The mutation text proves the claimed mutation is named at the test, not that "
            "the test applies it.",
            rule_dependence_mutation,
        ),
        Rule(
            R_PROPERTY, ("GOV-4-10",), (C_PROP_PRESENT, C_PROP_TIER, C_PROP_NAMES), DELTA,
            "`[por_reduction.property_analysis]` carries a `tier` from RFC 0014's property preservation tiers "
            "and a `note` of minimum length naming a changed crate.",
            _NOTE_BOUNDARY,
            rule_property_class,
        ),
        Rule(
            R_PERF, ("GOV-4-11",), (C_PERF_PRESENT, C_PERF_METRIC, C_PERF_NAMES), DELTA,
            "`[por_reduction.performance]` carries a `metric` from RFC 0014's `Evaluation` metrics and a `note` "
            "of minimum length naming a changed crate.",
            _NOTE_BOUNDARY,
            rule_performance,
        ),
        Rule(
            R_PACK_RECORD, ("GOV-4-12",), (C_PACK_HAS_RECORD, C_PACK_COVERS), DELTA,
            "Every pack change (a `continuum-effects-*` source file whose comment-stripped code changed or "
            "that the delta added) is covered by a review record the delta adds or modifies; every "
            "`pack.covers` entry matches such a change.",
            None,
            rule_pack_record,
        ),
        Rule(
            R_PACK_FIDELITY, ("GOV-4-12",), (C_FID_PRESENT, C_FID_REGISTERED, C_FID_NAMES), DELTA,
            "`[pack.fidelity_profile]` carries a `profile` from RFC 0002's required profiles and a `note` of "
            "minimum length naming a changed pack crate.",
            _NOTE_BOUNDARY,
            rule_pack_fidelity,
        ),
        Rule(
            R_LIST, ALL, (C_LIST, C_VOCAB), TREE,
            "docs/12 §4's POR/reduction and Pack changes lists, and the generated registry's GOV-4-07..12 "
            "summaries, still name these ids in order; docs/18, RFC 0014, docs/19 §4, and RFC 0002 still carry "
            "the vocabularies these rules type against.",
            None,
            rule_list_and_vocabulary,
        ),
    ),
    notes=(
        "GOV-4-13..GOV-4-16 (the remaining Pack-changes obligations: host/Lab conformance, fault coverage, "
        "independence review, version bump) are a later Bone's; PACK_SECTION_ITEMS names them so the list "
        "check still catches drift in the whole section.",
        "POR_REDUCTION_CORE and PACK_CORE are this set's own crate classification, not "
        "`check_code_policy.SEMANTIC_CORE`: POR/reduction and pack changes are a different change class from "
        "the semantic changes `gov4_semantic.py` enforces.",
    ),
)
