"""GOV-4-13…GOV-4-18 — the rest of Pack changes, and the first Kernel-changes
obligations (bn-4x90).

Source: `notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md` §4 "Review
requirements":

- "Pack changes / Require:" second through fifth items (`GOV-4-13`…`GOV-4-16`):
  host/Lab conformance, fault coverage, independence review, version bump. The
  first item, fidelity profile (`GOV-4-12`), is sibling Bone bn-2tm3's; this
  set extends the same `[pack]` review-record section bn-2tm3 defined, and
  reuses its change-class trigger (`pack_changes`, `pack_records`,
  `gov4_impact_reduction_pack._record_coverage`) unchanged by importing it —
  a pack change is still "a `continuum-effects-*` source file whose
  comment-stripped code changed, or that the delta added", and this file does
  not re-derive that predicate.
- "Kernel changes / Require:" first two items (`GOV-4-17`, `GOV-4-18`): two
  reviewers, fuzz corpus. The remaining three (mutation tests, code-size
  report, no unchecked optimization) are a later Bone's (bn-2b4e,
  GOV-4-19..21 — `KERNEL_SECTION_ITEMS` below names all five so the list
  check still catches drift in the whole section). A kernel change is a
  source change to one of the four `continuum-kernel-*` crates (RFC 0005
  "Kernel layering"; `check_code_policy.KERNEL`), carrying its own record
  section, `[kernel]`. No sibling Bone owns that trigger yet, so it is built
  here from `gov4_impact_reduction_pack`'s generic tier-change helpers
  (`_tier_source_changes`, `_section_records`), imported rather than copied.

A change with the remaining pack obligations extends the `[pack]` table
GOV-4-12 already ships:

    [pack]
    covers = ["crates/continuum-effects-network/src/"]

    [pack.fidelity_profile]                                    # bn-2tm3, GOV-4-12
    profile = "adversarial-envelope"
    note = "..."

    host_lab_conformance_tests = [
      "crates/continuum-effects-network/tests/x.rs::host_matches_lab",
    ]
    fault_coverage_tests = [
      { test = "crates/continuum-effects-network/tests/x.rs::crash_mid_write", fault = "crash" },
      { test = "crates/continuum-effects-network/tests/x.rs::partitions_both_ways", fault = "partition" },
    ]

    [pack.independence_review]
    reviewer = "continuum-security"
    review = "<seal review id>"
    verdict = "approved"

    [pack.version_bump]
    from = "0.3.0"
    to = "0.4.0"
    note = "continuum-effects-network's contract widened; consumers must re-pin"

A change with a kernel change ships:

    [kernel]
    covers = ["crates/continuum-kernel-core/src/"]

    [kernel.reviewers]
    entries = [
      { reviewer = "continuum-security", review = "<seal review id>", verdict = "approved" },
      { reviewer = "continuum-dev-2", review = "<seal review id>", verdict = "approved" },
    ]

    [kernel.fuzz_corpus]
    files = ["crates/continuum-kernel-core/tests/fuzz-corpus/gov4-fixture.case"]
    test = "crates/continuum-kernel-core/tests/x.rs::replays_committed_corpus"

`crates/continuumd/tests/wire-fuzz-corpus/` is the repository's existing
precedent for a committed, replayed fuzz corpus (bn-3tz3); `kernel.fuzz_corpus`
asks the kernel crates for the same shape, not for `cargo fuzz` (ruled out
there for the whole workspace: `unsafe_code = "forbid"`, the pinned stable
toolchain, and INV-005's seeded-input requirement).
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

from check_obligations import (
    DELTA,
    TREE,
    Context,
    Finding,
    ObligationSet,
    Rule,
    markdown_list,
    normalize,
    requirement_summaries,
    resolve_test,
    test_is_fresh,
)
import check_code_policy as policy
import rust_lexer

# `tools/governance` is already on `sys.path` (check_obligations.py inserts it),
# but `tools/governance/obligations` is not; add it so the sibling set's
# triggers can be imported by their plain module name.
sys.path.insert(0, str(Path(__file__).resolve().parent))

from gov4_impact_reduction_pack import (  # noqa: E402  (path insert must run first)
    NOTE_MIN,
    _crates,
    _names_a_crate,
    _record_coverage,
    _section_records,
    _tier_source_changes,
    pack_changes,
    pack_records,
)

CONSTITUTION = "notes/plan/docs/12_GOVERNANCE_AND_ENGINEERING.md"
RFC_0002 = "notes/plan/rfcs/0002-controlled-effects-and-domain-packs.md"

PACK_HEADING = "### Pack changes"
KERNEL_HEADING = "### Kernel changes"
REQUIRE_LEAD = "Require:"
FAULT_HEADING = "## Fault algebra"

# docs/01 §7.2 has no kernel crate list of its own; RFC 0005 "Kernel layering"
# is the source, and `check_code_policy.KERNEL` is already the same four
# crates read live from `Cargo.toml` membership rather than restated here.
KERNEL_CORE = frozenset(policy.KERNEL)

REQUIREMENTS = {
    "GOV-4-13": "host/Lab conformance",
    "GOV-4-14": "fault coverage",
    "GOV-4-15": "independence review",
    "GOV-4-16": "version bump",
    "GOV-4-17": "two reviewers",
    "GOV-4-18": "fuzz corpus",
}
ALL = tuple(REQUIREMENTS)

# The full "Pack changes" list, GOV-4-12's item included, so a drift in the
# item this set does not own is still caught (the ids are positional).
PACK_SECTION_ITEMS = (
    "fidelity profile",
    "host/Lab conformance",
    "fault coverage",
    "independence review",
    "version bump",
)
# The full "Kernel changes" list; only the first two are this set's.
KERNEL_SECTION_ITEMS = (
    "two reviewers",
    "fuzz corpus",
    "mutation tests",
    "code-size report",
    "no unchecked optimization",
)

NOTE_MIN_LOCAL = NOTE_MIN
SEMVER_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")
REVIEW_ID_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{2,63}")
HOST_RE = re.compile(r"\bhost\b", re.I)
LAB_RE = re.compile(r"\blab\b", re.I)
HOST_LAB_PHRASE = "production/lab paths"

# ---------------------------------------------------------------------------
# Rule and check names
# ---------------------------------------------------------------------------

R_HOST_LAB = "host-lab-conformance"
C_HL_NAMED = "host-lab-tests-named"
C_HL_RESOLVES = "host-lab-test-resolves"
C_HL_EXERCISES = "host-lab-test-exercises-the-change"
C_HL_FRESH = "host-lab-test-is-fresh"
C_HL_BOTH = "host-lab-test-names-both-paths"
C_HL_NA_FORM = "host-lab-not-applicable-well-formed"
C_HL_NA_REASON = "host-lab-not-applicable-reason"
C_HL_NA_NOSTD = "host-lab-not-applicable-crate-is-no-std"
C_HL_NA_TEST = "host-lab-not-applicable-tests-resolve"

# The lead's not-applicable form for GOV-4-13 (bn-3ohe, cr-1dl1d7): a pack that can
# perform no host effect and whose profile claims no host semantics (docs/09 T06) has
# no host path to conform. No lexical scan carries that claim. The absence of host
# effects is the compiler's: the crate is `#![no_std]` unconditionally (no
# `cfg_attr`, no `extern crate std`, no `[features]`), so it links only `core` and
# `alloc`, and `unsafe_code` is forbidden workspace-wide. The pack's own compiler lane
# test shows that proof is live. The value `host == HostQualification::None` is a
# named Rust test's to assert. This check verifies the structure and that both
# tests exist, live in the pack crate, and are not ignored.
HOST_NA_STATUS = "not-applicable"
HOST_NA_KEYS = {"status", "reason", "profile_test", "no_std_lane_test"}
# cr-35ujnx: the source rules (`#![no_std]` first, no block comment, raw token or
# banned identifier, and `extern` only as `extern crate alloc;` in `lib.rs`) are
# `rust_lexer.structure_problem`, token by token over a complete Rust lexer that fails
# closed on source it cannot lex. `#![no_std]` does not forbid `extern crate std`.
# The profile test must assert the declared profile's host qualification exactly.
PROFILE_ASSERT_RE = re.compile(
    r"assert_eq!\(\s*[A-Z][A-Z0-9_]*\s*\.\s*host\s*,\s*HostQualification\s*::\s*None\s*\)"
)
HOST_DECL_RE = re.compile(r"\bhost\s*:\s*HostQualification\s*::\s*(\w+)")
# What identifies a compiler lane: it plants a `std` use, demands the unresolved-`std`
# error, and checks the release profile too.
# The lane also checks rustc's dep-info (`ambient_inputs`) and Cargo's resolved view of
# the pack (`cargo_problem`, cr-35ujnx round 5): the gating evidence for "no build
# script, no links, no dependency" is `cargo metadata`, not the manifest text.
LANE_MARKERS = (
    "std::net::UdpSocket", "E0433", "release", "with_aliased_std", "ambient_inputs(&control",
    "cargo_problem(manifest_dir()",
)
# Attributes that make a named test evidence of nothing.
DEAD_TEST_ATTR_RE = re.compile(r"\b(should_panic|cfg|cfg_attr|ignore)\b")

R_FAULT = "fault-coverage"
C_FC_NAMED = "fault-coverage-tests-named"
C_FC_RESOLVES = "fault-coverage-test-resolves"
C_FC_REGISTERED = "fault-coverage-fault-registered"
C_FC_EXERCISES = "fault-coverage-test-exercises-the-change"
C_FC_FRESH = "fault-coverage-test-is-fresh"
C_FC_BREADTH = "fault-coverage-breadth"

R_INDEP = "independence-review"
C_IND_PRESENT = "independence-review-present"
C_IND_INDEPENDENT = "independence-review-independent"
C_IND_APPROVED = "independence-review-approved"

R_VERSION = "version-bump"
C_VB_PRESENT = "version-bump-present"
C_VB_FORMAT = "version-bump-well-formed"
C_VB_INCREASES = "version-bump-increases"
C_VB_NAMES = "version-bump-names-the-change"

R_KERNEL_RECORD = "kernel-review-record"
C_KR_HAS = "kernel-change-has-review-record"
C_KR_COVERS = "kernel-review-record-covers-a-kernel-change"

R_REVIEWERS = "kernel-two-reviewers"
C_RV_PRESENT = "kernel-reviewers-present"
C_RV_DISTINCT = "kernel-reviewers-distinct"
C_RV_APPROVED = "kernel-reviewers-approved"

R_FUZZ = "kernel-fuzz-corpus"
C_FZ_PRESENT = "fuzz-corpus-present"
C_FZ_FILES = "fuzz-corpus-files-exist"
C_FZ_RESOLVES = "fuzz-corpus-test-resolves"
C_FZ_EXERCISES = "fuzz-corpus-test-exercises-the-change"
C_FZ_FRESH = "fuzz-corpus-test-is-fresh"

R_LIST = "gov4-13-18-review-list"
C_LIST = "gov4-13-18-list-registered"
C_VOCAB = "gov4-13-18-vocabulary-present"


# ---------------------------------------------------------------------------
# Kernel change-class trigger, built from the shared tier-change helpers
# ---------------------------------------------------------------------------


def kernel_changes(ctx: Context) -> list[str]:
    return _tier_source_changes(ctx, KERNEL_CORE)


def kernel_records(ctx: Context) -> list[tuple]:
    return _section_records(ctx, "kernel", kernel_changes(ctx))


# ---------------------------------------------------------------------------
# Vocabularies read live
# ---------------------------------------------------------------------------


def fault_vocabulary(ctx: Context) -> list[str]:
    text = ctx.head.read_text(RFC_0002) or ""
    idx = text.find(FAULT_HEADING)
    if idx == -1:
        return []
    m = re.search(r"```text\n(.*?)\n```", text[idx:], re.S)
    if not m:
        return []
    return sorted(set(re.findall(r"\b([a-z_]+)\s*\(", m.group(1))))


# ---------------------------------------------------------------------------
# GOV-4-13 — host/Lab conformance
# ---------------------------------------------------------------------------


def no_std_problem(ctx: Context, crate: str) -> str | None:
    """Why some build of `crate` could link `std`, or `None` when none can.

    The same structure rules the pack's compiler lane and the INV-015 audit enforce,
    read token by token by `rust_lexer` (source it cannot lex fails):
    `#![no_std]` is the crate root's first item; `src/` has no block comment, no `cfg`,
    `cfg_attr`, macro definition, `include!`, `#[path]`, `asm!` or raw token, and
    exactly one `extern crate`, which is `alloc` in `lib.rs`; the manifest has only
    `[package]` (no `build`, no `links`) and `[lints] workspace = true`, and there is
    no `build.rs`. They make the lane's dev and release host builds the only builds;
    the lane runs the compiler on them."""
    lib = ctx.head.read_text(f"crates/{crate}/src/lib.rs")
    if lib is None:
        return f"crates/{crate}/src/lib.rs does not exist"
    files = [(rel, ctx.head.read_text(rel) or "") for rel in ctx.head.glob(f"crates/{crate}/src/**/*.rs")]
    problem = rust_lexer.structure_problem(files)
    if problem:
        return problem
    if ctx.head.exists(f"crates/{crate}/build.rs"):
        return f"crates/{crate}/build.rs exists"
    manifest = ctx.head.read_toml(f"crates/{crate}/Cargo.toml")
    if manifest is None:
        return f"crates/{crate}/Cargo.toml does not parse"
    extra = sorted(set(manifest) - {"package", "lints"})
    if extra:
        return f"crates/{crate}/Cargo.toml has `{extra[0]}`; only `[package]` and `[lints]` are allowed"
    package = manifest.get("package") if isinstance(manifest.get("package"), dict) else {}
    if "build" in package or "links" in package:
        return f"crates/{crate}/Cargo.toml sets `build` or `links`"
    if manifest.get("lints") != {"workspace": True}:
        return f"crates/{crate}/Cargo.toml does not take exactly `[lints] workspace = true`"
    return None


def _live_test(ctx: Context, ref: object, crates: set[str]) -> tuple[object | None, str | None]:
    """A named evidence test: resolves, lives in a changed pack crate, and carries no
    attribute that makes it evidence of nothing (`should_panic`, `cfg`, `cfg_attr`)."""
    test, problem = resolve_test(ctx, ref)
    if test is None:
        return None, problem
    if ctx.crate_of(test.path) not in crates:
        return None, f"{test.path} is not in a changed pack crate {sorted(crates)}"
    attrs = policy.strip_rust_comments(test.attrs)
    if DEAD_TEST_ATTR_RE.search(attrs):
        return None, f"`fn {test.name}` carries `should_panic`, `cfg` or `ignore`, so it proves nothing"
    return test, None


def _host_not_applicable(ctx: Context, rec, table: dict, crates: set[str]) -> list[Finding]:
    """Validate `[pack.host_conformance]`. Fail closed: the form holds only when every
    changed pack crate is unconditionally `no_std`, and the record names a live profile
    test asserting `HostQualification::None` and a live compiler lane test, both in a
    changed pack crate."""
    out: list[Finding] = []
    form = table.get("host_conformance")
    if (
        not isinstance(form, dict)
        or form.get("status") != HOST_NA_STATUS
        or set(form) - HOST_NA_KEYS
        or "host_lab_conformance_tests" in table
    ):
        return [Finding(
            R_HOST_LAB, C_HL_NA_FORM,
            f"{rec.path}: `[pack.host_conformance]` takes exactly `status = \"{HOST_NA_STATUS}\"`, `reason`, "
            "`profile_test` and `no_std_lane_test`, and must not be combined with `host_lab_conformance_tests`",
        )]
    reason = form.get("reason")
    text = normalize(reason) if isinstance(reason, str) else ""
    if len(text) < NOTE_MIN_LOCAL:
        out.append(Finding(
            R_HOST_LAB, C_HL_NA_REASON,
            f"{rec.path}: `pack.host_conformance.reason` must be at least {NOTE_MIN_LOCAL} characters; "
            "not-applicable is a claim, and a claim carries its reason",
        ))
    for crate in sorted(crates):
        problem = no_std_problem(ctx, crate)
        if problem is not None:
            out.append(Finding(
                R_HOST_LAB, C_HL_NA_NOSTD,
                f"{rec.path}: host/Lab conformance is not-applicable only for a pack the compiler proves "
                f"free of host effects: {problem}; the GOV-4-13 obligation applies in full",
            ))
    declared: list[str] = []
    for crate in sorted(crates):
        for rel in ctx.head.glob(f"crates/{crate}/src/**/*.rs"):
            declared.extend(HOST_DECL_RE.findall(policy.strip_rust_comments(ctx.head.read_text(rel) or "")))
    if not declared or any(q != "None" for q in declared):
        out.append(Finding(
            R_HOST_LAB, C_HL_NA_TEST,
            f"{rec.path}: the pack's profiles declare host qualifications {declared or 'none at all'}; "
            "not-applicable needs every declared profile to say `HostQualification::None`",
        ))
    profile, problem = _live_test(ctx, form.get("profile_test"), crates)
    if profile is None:
        out.append(Finding(R_HOST_LAB, C_HL_NA_TEST, f"{rec.path}: `pack.host_conformance.profile_test`: {problem}"))
    elif not PROFILE_ASSERT_RE.search(policy.strip_rust_comments(profile.body)):
        out.append(Finding(
            R_HOST_LAB, C_HL_NA_TEST,
            f"{rec.path}: `pack.host_conformance.profile_test` {profile.name} does not assert "
            "`assert_eq!(<PROFILE>.host, HostQualification::None)`",
        ))
    lane, problem = _live_test(ctx, form.get("no_std_lane_test"), crates)
    if lane is None:
        out.append(Finding(R_HOST_LAB, C_HL_NA_TEST, f"{rec.path}: `pack.host_conformance.no_std_lane_test`: {problem}"))
    elif not all(marker in lane.body for marker in LANE_MARKERS):
        out.append(Finding(
            R_HOST_LAB, C_HL_NA_TEST,
            f"{rec.path}: `pack.host_conformance.no_std_lane_test` {lane.name} is not a compiler lane: it must "
            f"plant a `std` use and demand the unresolved-`std` error in both profiles {list(LANE_MARKERS)}",
        ))
    return out


def rule_host_lab(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    for rec, table, covered in pack_records(ctx):
        if not covered:
            continue
        crates = _crates(ctx, covered)
        if "host_conformance" in table:
            out.extend(_host_not_applicable(ctx, rec, table, crates))
            continue
        entries = table.get("host_lab_conformance_tests")
        if not isinstance(entries, list) or not entries or not all(isinstance(e, str) for e in entries):
            out.append(Finding(
                R_HOST_LAB, C_HL_NAMED,
                f"{rec.path}: `pack.host_lab_conformance_tests` is missing, empty, or malformed; a pack change "
                f"over {sorted(crates)} requires host/Lab conformance testing ({CONSTITUTION} §4)",
            ))
            continue
        fresh_any = exercised_any = both_any = False
        bad = False
        for ref in entries:
            test, problem = resolve_test(ctx, ref)
            if test is None:
                out.append(Finding(R_HOST_LAB, C_HL_RESOLVES, f"{rec.path}: {problem}"))
                bad = True
                continue
            fresh_any |= test_is_fresh(ctx, test)
            test_crate = ctx.crate_of(test.path)
            exercised_any |= bool(test_crate) and ctx.reaches(test_crate, crates)
            text = test.attrs + "\n" + test.body
            both_any |= bool(HOST_RE.search(text)) and bool(LAB_RE.search(text))
        if bad:
            continue
        if not exercised_any:
            out.append(Finding(
                R_HOST_LAB, C_HL_EXERCISES,
                f"{rec.path}: no test in `pack.host_lab_conformance_tests` lives in, or reaches through its "
                f"dependencies, a changed pack crate {sorted(crates)}",
            ))
        if not fresh_any:
            out.append(Finding(
                R_HOST_LAB, C_HL_FRESH,
                f"{rec.path}: no test in `pack.host_lab_conformance_tests` is new or has a changed body in this "
                "delta",
            ))
        if not both_any:
            out.append(Finding(
                R_HOST_LAB, C_HL_BOTH,
                f"{rec.path}: no test in `pack.host_lab_conformance_tests` names both a host and a Lab path "
                f"({RFC_0002} \"Pack qualification\": differential testing across production/Lab paths)",
            ))
    return out


# ---------------------------------------------------------------------------
# GOV-4-14 — fault coverage
# ---------------------------------------------------------------------------


def rule_fault_coverage(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    faults = [normalize(f) for f in fault_vocabulary(ctx)]
    for rec, table, covered in pack_records(ctx):
        if not covered:
            continue
        crates = _crates(ctx, covered)
        entries = table.get("fault_coverage_tests")
        if not isinstance(entries, list) or not entries or not all(isinstance(e, dict) for e in entries):
            out.append(Finding(
                R_FAULT, C_FC_NAMED,
                f"{rec.path}: `pack.fault_coverage_tests` is missing, empty, or malformed; a pack change over "
                f"{sorted(crates)} requires fault coverage ({CONSTITUTION} §4)",
            ))
            continue
        fresh_any = exercised_any = False
        bad = False
        seen: set[str] = set()
        for entry in entries:
            test, problem = resolve_test(ctx, entry.get("test"))
            if test is None:
                out.append(Finding(R_FAULT, C_FC_RESOLVES, f"{rec.path}: {problem}"))
                bad = True
                continue
            fault = entry.get("fault")
            if not isinstance(fault, str) or normalize(fault) not in faults:
                out.append(Finding(
                    R_FAULT, C_FC_REGISTERED,
                    f"{rec.path}: {entry.get('test')}: fault {fault!r} is not one of {RFC_0002}'s "
                    f"`{FAULT_HEADING}` operators {fault_vocabulary(ctx)}",
                ))
                bad = True
                continue
            seen.add(normalize(fault))
            fresh_any |= test_is_fresh(ctx, test)
            test_crate = ctx.crate_of(test.path)
            exercised_any |= bool(test_crate) and ctx.reaches(test_crate, crates)
        if bad:
            continue
        if not exercised_any:
            out.append(Finding(
                R_FAULT, C_FC_EXERCISES,
                f"{rec.path}: no test in `pack.fault_coverage_tests` lives in, or reaches through its "
                f"dependencies, a changed pack crate {sorted(crates)}",
            ))
        if not fresh_any:
            out.append(Finding(
                R_FAULT, C_FC_FRESH,
                f"{rec.path}: no test in `pack.fault_coverage_tests` is new or has a changed body in this delta",
            ))
        if len(seen) < 2:
            out.append(Finding(
                R_FAULT, C_FC_BREADTH,
                f"{rec.path}: `pack.fault_coverage_tests` names only {sorted(seen)}; fault coverage names at "
                f"least two distinct {RFC_0002} `{FAULT_HEADING}` operators, not one fault dressed up twice",
            ))
    return out


# ---------------------------------------------------------------------------
# GOV-4-15 — independence review
# ---------------------------------------------------------------------------


def rule_independence_review(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    for rec, table, covered in pack_records(ctx):
        if not covered:
            continue
        review_tbl = table.get("independence_review")
        reviewer = review_tbl.get("reviewer") if isinstance(review_tbl, dict) else None
        review = review_tbl.get("review") if isinstance(review_tbl, dict) else None
        if not isinstance(reviewer, str) or not reviewer.strip() or not isinstance(review, str) \
                or not REVIEW_ID_RE.fullmatch(review):
            out.append(Finding(
                R_INDEP, C_IND_PRESENT,
                f"{rec.path}: `[pack.independence_review]` must name a `reviewer` and a `review` id; a pack "
                f"change requires an independence review ({CONSTITUTION} §4)",
            ))
            continue
        author = (rec.data or {}).get("change", {}).get("author")
        if isinstance(author, str) and normalize(author) == normalize(reviewer):
            out.append(Finding(
                R_INDEP, C_IND_INDEPENDENT,
                f"{rec.path}: the independence reviewer {reviewer!r} is the change author; no self-certification "
                "(INV-004)",
            ))
        verdict = review_tbl.get("verdict")
        if verdict != "approved":
            out.append(Finding(
                R_INDEP, C_IND_APPROVED,
                f"{rec.path}: independence review verdict is {verdict!r}; only `approved` lets a pack change "
                "land",
            ))
    return out


# ---------------------------------------------------------------------------
# GOV-4-16 — version bump
# ---------------------------------------------------------------------------


def _semver(value: object) -> tuple[int, int, int] | None:
    if not isinstance(value, str):
        return None
    m = SEMVER_RE.fullmatch(value)
    return None if m is None else (int(m.group(1)), int(m.group(2)), int(m.group(3)))


def rule_version_bump(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    for rec, table, covered in pack_records(ctx):
        if not covered:
            continue
        crates = _crates(ctx, covered)
        vb = table.get("version_bump")
        if not isinstance(vb, dict) or "from" not in vb or "to" not in vb or "note" not in vb:
            out.append(Finding(
                R_VERSION, C_VB_PRESENT,
                f"{rec.path}: `[pack.version_bump]` with `from`, `to`, and `note` is missing; a pack change "
                f"requires a version bump ({CONSTITUTION} §4)",
            ))
            continue
        before, after = _semver(vb.get("from")), _semver(vb.get("to"))
        if before is None or after is None:
            out.append(Finding(
                R_VERSION, C_VB_FORMAT,
                f"{rec.path}: `pack.version_bump.from`/`to` must both be `major.minor.patch` versions, got "
                f"{vb.get('from')!r} -> {vb.get('to')!r}",
            ))
            continue
        if after <= before:
            out.append(Finding(
                R_VERSION, C_VB_INCREASES,
                f"{rec.path}: `pack.version_bump` {vb['from']!r} -> {vb['to']!r} does not increase; a pack "
                "change bumps its pack version",
            ))
        note = vb.get("note")
        text = normalize(note) if isinstance(note, str) else ""
        if len(text) < NOTE_MIN_LOCAL or not _names_a_crate(text, crates):
            out.append(Finding(
                R_VERSION, C_VB_NAMES,
                f"{rec.path}: `pack.version_bump.note` must be at least {NOTE_MIN_LOCAL} characters and name a "
                f"changed pack crate {sorted(crates)}",
            ))
    return out


# ---------------------------------------------------------------------------
# GOV-4-17/18 — kernel changes
# ---------------------------------------------------------------------------


def rule_kernel_record(ctx: Context) -> list[Finding]:
    return _record_coverage(
        ctx, kernel_records(ctx), kernel_changes(ctx), R_KERNEL_RECORD, C_KR_HAS, C_KR_COVERS,
        "kernel", "kernel change", "two reviewers and a fuzz corpus",
    )


def rule_two_reviewers(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    for rec, table, covered in kernel_records(ctx):
        if not covered:
            continue
        reviewers_tbl = table.get("reviewers")
        entries = reviewers_tbl.get("entries") if isinstance(reviewers_tbl, dict) else None
        valid_shape = (
            isinstance(entries, list) and len(entries) >= 2
            and all(isinstance(e, dict) and isinstance(e.get("reviewer"), str) and e["reviewer"].strip()
                    for e in entries)
        )
        if not valid_shape:
            out.append(Finding(
                R_REVIEWERS, C_RV_PRESENT,
                f"{rec.path}: `[kernel.reviewers]` needs an `entries` list of at least two reviewers, each "
                f"naming a `reviewer`; a kernel change requires two reviewers ({CONSTITUTION} §4)",
            ))
            continue
        author = (rec.data or {}).get("change", {}).get("author")
        author_n = normalize(author) if isinstance(author, str) else None
        names = [normalize(e["reviewer"]) for e in entries]
        if len(set(names)) != len(names) or (author_n is not None and author_n in names):
            out.append(Finding(
                R_REVIEWERS, C_RV_DISTINCT,
                f"{rec.path}: `kernel.reviewers.entries` names {[e['reviewer'] for e in entries]}, which repeats "
                f"a reviewer or names the change author {author!r}; two reviewers means two distinct people, "
                "neither of them the author (INV-004)",
            ))
            continue
        for entry in entries:
            review, verdict = entry.get("review"), entry.get("verdict")
            if not isinstance(review, str) or not REVIEW_ID_RE.fullmatch(review) or verdict != "approved":
                out.append(Finding(
                    R_REVIEWERS, C_RV_APPROVED,
                    f"{rec.path}: {entry['reviewer']!r}: `review` must be a review id and `verdict` must be "
                    f"`approved` (got {verdict!r})",
                ))
    return out


def rule_fuzz_corpus(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    for rec, table, covered in kernel_records(ctx):
        if not covered:
            continue
        crates = _crates(ctx, covered)
        fz = table.get("fuzz_corpus")
        files = fz.get("files") if isinstance(fz, dict) else None
        if not isinstance(fz, dict) or not isinstance(files, list) or not files \
                or not all(isinstance(f, str) for f in files) or not isinstance(fz.get("test"), str):
            out.append(Finding(
                R_FUZZ, C_FZ_PRESENT,
                f"{rec.path}: `[kernel.fuzz_corpus]` needs a non-empty `files` list and a `test`; a kernel "
                f"change requires a fuzz corpus ({CONSTITUTION} §4)",
            ))
            continue
        missing = [f for f in files if ctx.head.read_text(f) is None]
        if missing:
            out.append(Finding(
                R_FUZZ, C_FZ_FILES,
                f"{rec.path}: `kernel.fuzz_corpus.files` names {missing}, which do not exist at head; a "
                "committed corpus is evidence, not a claim",
            ))
            continue
        test, problem = resolve_test(ctx, fz["test"])
        if test is None:
            out.append(Finding(R_FUZZ, C_FZ_RESOLVES, f"{rec.path}: {problem}"))
            continue
        test_crate = ctx.crate_of(test.path)
        if not (bool(test_crate) and ctx.reaches(test_crate, crates)):
            out.append(Finding(
                R_FUZZ, C_FZ_EXERCISES,
                f"{rec.path}: `kernel.fuzz_corpus.test` {fz['test']} does not live in, or reach through its "
                f"dependencies, a changed kernel crate {sorted(crates)}",
            ))
        if not test_is_fresh(ctx, test):
            out.append(Finding(
                R_FUZZ, C_FZ_FRESH,
                f"{rec.path}: `kernel.fuzz_corpus.test` {fz['test']} is not new and has no changed body in this "
                "delta",
            ))
    return out


# ---------------------------------------------------------------------------
# Tree rule: the source lists and the vocabularies still say what the ids bind
# ---------------------------------------------------------------------------


def rule_list(ctx: Context) -> list[Finding]:
    out: list[Finding] = []
    text = ctx.head.read_text(CONSTITUTION) or ""
    pack_items = markdown_list(text, PACK_HEADING, REQUIRE_LEAD)
    if pack_items is None or tuple(pack_items) != PACK_SECTION_ITEMS:
        out.append(Finding(
            R_LIST, C_LIST,
            f"{CONSTITUTION} §4 `{PACK_HEADING}` / `{REQUIRE_LEAD}` lists {pack_items}, not "
            f"{list(PACK_SECTION_ITEMS)}; the GOV-4-13..16 ids this set binds are positional",
        ))
    kernel_items = markdown_list(text, KERNEL_HEADING, REQUIRE_LEAD)
    if kernel_items is None or tuple(kernel_items) != KERNEL_SECTION_ITEMS:
        out.append(Finding(
            R_LIST, C_LIST,
            f"{CONSTITUTION} §4 `{KERNEL_HEADING}` / `{REQUIRE_LEAD}` lists {kernel_items}, not "
            f"{list(KERNEL_SECTION_ITEMS)}; the GOV-4-17/18 ids this set binds are the first two, positional",
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
    if len(fault_vocabulary(ctx)) < 2:
        out.append(Finding(
            R_LIST, C_VOCAB,
            f"{RFC_0002} `{FAULT_HEADING}` no longer lists fault operators GOV-4-14 names its tests by",
        ))
    text = normalize(ctx.head.read_text(RFC_0002) or "")
    if HOST_LAB_PHRASE not in text:
        out.append(Finding(
            R_LIST, C_VOCAB,
            f"{RFC_0002} no longer names {HOST_LAB_PHRASE!r} (\"Pack qualification\"); GOV-4-13 host/Lab "
            "conformance is grounded there",
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
    "Proves a typed field exists and a note of minimum length names a changed crate. Whether the note is "
    "accurate is a review judgement."
)
_REVIEW_POINTER_BOUNDARY = (
    "The record is a pointer, not the review: Seal keeps review state outside the repository, so no committed "
    "artifact lets this check confirm that the named review exists, covers this change, or carries the claimed "
    "verdict — the same limit GOV-4-06 documents. The rule still holds the pointer to a typed shape "
    "(independence from the author, or two distinct non-author reviewers) and to a syntactically valid review "
    "id."
)

SET = ObligationSet(
    name="gov-4-pack-kernel",
    title=(
        "GOV §4 host/Lab conformance, fault coverage, independence review, version bump, and the first "
        "Kernel-changes obligations — GOV-4-13..18"
    ),
    source=f"{CONSTITUTION} §4 Review requirements",
    requirements=REQUIREMENTS,
    evidence="tools/governance/evidence/gov-4-pack-kernel.json",
    record_keys={
        "pack": (
            "host_lab_conformance_tests",
            "host_conformance",
            "fault_coverage_tests",
            "independence_review",
            "version_bump",
        ),
        "kernel": ("covers", "reviewers", "fuzz_corpus"),
    },
    rules=(
        Rule(
            R_HOST_LAB, ("GOV-4-13",),
            (C_HL_NAMED, C_HL_RESOLVES, C_HL_EXERCISES, C_HL_FRESH, C_HL_BOTH, C_HL_NA_FORM, C_HL_NA_REASON,
             C_HL_NA_NOSTD, C_HL_NA_TEST), DELTA,
            "`pack.host_lab_conformance_tests` names at least one test that resolves, reaches the change, is "
            "new or changed, and names both `host` and `lab` in its doc comment or body (RFC 0002 \"Pack "
            "qualification\": differential testing across production/Lab paths). Or, only for a pack that is "
            "unconditionally `#![no_std]` (no `cfg_attr`, no `extern crate std`, no `[features]`), "
            "`[pack.host_conformance]` states `status = \"not-applicable\"` with a reason, a live "
            "`profile_test` in the pack that asserts `HostQualification::None`, and a live "
            "`no_std_lane_test` in the pack (docs/09 T06); anything less gets the full obligation.",
            _TEST_BOUNDARY + " `host`/`lab` are checked as literal words, not as a cross-implementation "
            "comparison like GOV-4-03/GOV-4-08's oracle/subject pairs — no host crate distinct from the pack "
            "exists in the workspace to name.",
            rule_host_lab,
        ),
        Rule(
            R_FAULT, ("GOV-4-14",),
            (C_FC_NAMED, C_FC_RESOLVES, C_FC_REGISTERED, C_FC_EXERCISES, C_FC_FRESH, C_FC_BREADTH), DELTA,
            "`pack.fault_coverage_tests` names at least two entries, each pairing a test with one of RFC 0002's "
            "`Fault algebra` operators; every test resolves, reaches the change, and at least one is new or "
            "changed; the named faults span at least two distinct operators.",
            _TEST_BOUNDARY + " Coverage is measured by how many distinct fault operators are named, not by "
            "whether the test actually injects the claimed fault.",
            rule_fault_coverage,
        ),
        Rule(
            R_INDEP, ("GOV-4-15",), (C_IND_PRESENT, C_IND_INDEPENDENT, C_IND_APPROVED), DELTA,
            "`[pack.independence_review]` names a reviewer other than the change author, a review id, and the "
            "verdict `approved`.",
            _REVIEW_POINTER_BOUNDARY,
            rule_independence_review,
        ),
        Rule(
            R_VERSION, ("GOV-4-16",), (C_VB_PRESENT, C_VB_FORMAT, C_VB_INCREASES, C_VB_NAMES), DELTA,
            "`[pack.version_bump]` carries `from`/`to` `major.minor.patch` versions with `to` strictly greater "
            "than `from`, and a `note` of minimum length naming a changed pack crate.",
            _NOTE_BOUNDARY + " No pack crate carries its own `Cargo.toml` version today (`version.workspace = "
            "true`), so the claimed bump is not cross-checked against a manifest; only its own internal "
            "consistency (typed, increasing) is proven.",
            rule_version_bump,
        ),
        Rule(
            R_KERNEL_RECORD, ("GOV-4-17", "GOV-4-18"), (C_KR_HAS, C_KR_COVERS), DELTA,
            "Every kernel change (a `continuum-kernel-*` source file whose comment-stripped code changed or "
            "that the delta added) is covered by a review record the delta adds or modifies; every "
            "`kernel.covers` entry matches such a change.",
            None,
            rule_kernel_record,
        ),
        Rule(
            R_REVIEWERS, ("GOV-4-17",), (C_RV_PRESENT, C_RV_DISTINCT, C_RV_APPROVED), DELTA,
            "`[kernel.reviewers]` names at least two reviewer entries, all distinct from each other and from "
            "the change author, each with a review id and the verdict `approved`.",
            _REVIEW_POINTER_BOUNDARY,
            rule_two_reviewers,
        ),
        Rule(
            R_FUZZ, ("GOV-4-18",), (C_FZ_PRESENT, C_FZ_FILES, C_FZ_RESOLVES, C_FZ_EXERCISES, C_FZ_FRESH), DELTA,
            "`[kernel.fuzz_corpus]` names a non-empty list of committed corpus files (present at head) and a "
            "test that resolves, reaches the change, and is new or changed.",
            _TEST_BOUNDARY + " A committed file's *presence* is proven, not that the named test actually "
            "replays it, and not that the corpus exercises the kernel crate's decoders with any particular "
            "coverage.",
            rule_fuzz_corpus,
        ),
        Rule(
            R_LIST, ALL, (C_LIST, C_VOCAB), TREE,
            "docs/12 §4's Pack changes and Kernel changes lists, and the generated registry's GOV-4-13..18 "
            "summaries, still name these ids in order; RFC 0002 still carries the fault-algebra vocabulary and "
            "the production/Lab-paths wording these rules type against.",
            None,
            rule_list_and_vocabulary,
        ),
    ),
    undelivered={
        "GOV-4-15": (
            "The record names an independent reviewer, a review id, and a verdict, and the check holds them to "
            "independence and approval, but Seal keeps review state outside the repository: no committed "
            "artifact lets a checker confirm the review exists, covers this change, and approved it. Same limit "
            "as GOV-4-06."
        ),
        "GOV-4-17": (
            "The record names two distinct, non-author reviewers with review ids and verdicts, and the check "
            "holds them to distinctness and approval, but Seal keeps review state outside the repository: no "
            "committed artifact lets a checker confirm either review exists, covers this change, or approved "
            "it. Same limit as GOV-4-06."
        ),
    },
    notes=(
        "GOV-4-12 (fidelity profile), the first Pack-changes item, is sibling Bone bn-2tm3's; this set imports "
        "its `pack_changes`/`pack_records`/`_record_coverage` rather than re-deriving what a pack change is.",
        "GOV-4-19..GOV-4-21 (the remaining Kernel-changes obligations: mutation tests, code-size report, no "
        "unchecked optimization) are a later Bone's (bn-2b4e, running in parallel); `KERNEL_SECTION_ITEMS` "
        "names them so the list check still catches drift in the whole section.",
        "`KERNEL_CORE` is `check_code_policy.KERNEL` (the four `continuum-kernel-*` crates), read live rather "
        "than restated; kernel crates are also `SEMANTIC_CORE` members, so a kernel change is a semantic change "
        "too and separately ships `gov4_semantic.py`'s obligations.",
    ),
)
