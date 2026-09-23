#!/usr/bin/env python3
"""Bind docs/09 T09 ("Agent weakens property") to its live enforcement.

Source of truth:

- `notes/plan/docs/09_THREAT_MODEL.md` T09 — five required controls, verbatim:
  semantic property diff; review gates; immutable baseline property digest in
  CI; mutation score; agent cannot modify claim ledger or certificate.

The convention is `check_t04_evidence.py`'s (bn-1ptb). "Citing" a control
means re-running its enforcer for real: a Python delegate's `--self-test` and
real run, or the exact Rust tests that prove the behavioural fact, narrowly,
by exact name, each required to print its own `... ok` line. A committed
status string is never read back. A control that regresses fails here too.

Control -> enforcement point map
---------------------------------

    semantic property diff                                             PASS
        `continuum-semantic-diff::properties` classifies an agent-authored
        weakening of a real corpus claim (the plan §5.1 complementary-
        disjunct move) as an affirmed `weakened` with a derived witness, and
        `PolicyTable::verdict` closes it to `block` under the fixture's own
        `locked` verb (`pr12_impl02_property_ast_edit_evidence.rs`). The
        disguises fail too: a double-negation dressing dissolves to the same
        `weakened`, a vacuous-conjunct game never reaches `unchanged`, and a
        rename-while-weakening is removed-plus-added, never unchanged. Two
        exploit mutants (a classifier blind to expression content; a blind
        property classifier in the PR-12 exit campaign) are each shown to
        flip the result and to be rejected (`pr12_exit_evidence.rs`).

    review gates                                                       PASS
        Under the fixture's own policy, each of the six INV-001 verbs
        (weaken a property, strengthen an assumption, shrink a bound, hide
        an event, remove a fault, downgrade assurance) forbids `allow` on
        every acceptance path, and no acceptance path reads one as a repair;
        accept/reject/lock are privileged wire operations and proposing is
        not (`continuum-intent` `inv001_protected_intent_evidence.rs`). A
        `review` verb closes a weakening to `review`, never `allow`
        (impl02, above). At the repository grain, a semantic-core change
        must ship a GOV §4 review record: `check_obligations.py` is re-run,
        self-test first.

    immutable baseline property digest in CI                           PASS
        New in bn-20co. `tools/governance/t09-property-baseline.json` pins
        three BLAKE3 digests (whole-contract identity, the `claims` group,
        the `policy` table) of every committed Intent Contract. The digests
        are computed by the real Rust decoder in
        `crates/continuum-intent/tests/t09_property_baseline.rs`, which
        also refuses an unpinned contract, a stale pin, and a doubled pin,
        and carries two hostile fixtures: the complementary-disjunct
        weakening moves the `claims` pin, and unlocking `properties` moves
        the `policy` pin. This file owns the other half, the
        `baseline-revision-delta` rule: against the merge base, a moved,
        added, or retired pin must carry a fresh, named revision (a Bone id
        and a reason). `just check` runs both (`test`, then `governance`),
        and CI runs `just check`.

    mutation score                                                      GAP
        No producer computes a mutation score for a property set: nothing
        seeds program mutants and reports the fraction a property kills.
        What exists is mutant *anti-vacuity* for the classifiers above, the
        kernel mutation-class matrix (KCOV-08, certificates, not
        properties), and docs/19 §4's semantic-engine mutants — none is a
        per-property score, and conflating them would be an unearned claim.
        Forge's mutation challenges (plan §14.5) are pinned as not landed by
        `inv012_nonvacuity_evidence.rs::boundary_mutation_challenges_are_`
        `forges_own_responsibility_and_not_yet_landed`, re-run here.
        `direct_checks`' `mutation-score-gap` scans for a producer and fails
        the moment one appears, so the gap cannot go stale silently.

    agent cannot modify claim ledger or certificate                 PARTIAL
        Bound at the daemon and certificate grain: no mutation request
        admits a caller-supplied status; the `Promotion` witness cannot be
        built outside the verification service; receipt appending is gated
        to service actors and never self-certified; privilege is never
        acquired by default and the privileged set is exactly the five
        governance operations (`inv015_agent_least_authority_evidence.rs`);
        the append-only evidence containers admit no removal verb
        (`inv009_monotonic_task_evidence.rs`); a receipt or a `Published`
        name cannot be forged outside the store
        (`inv017_publication_atomicity_evidence.rs`); the certificate
        checker takes wire-form bytes only and collapses no outcome into a
        bool (`inv004_no_self_certification.rs`), and the no-self-
        certification crate edges hold (`check_crate_boundaries.py`).
        Recorded absence: the repository claims registry (docs/18 and its
        `PLAN_REQUIREMENTS.json` mirror) is writable by any committer. The
        mirror check (`check_claim_governance.py`) makes a one-sided edit
        fail, and review gates an edit, but nothing in the tree prevents an
        agent commit that edits both. `claim-registry-write-protection-gap`
        fails the moment a CODEOWNERS file appears, so the gap is revisited
        rather than left stale.

Failure monotonicity
--------------------

T09's bone asks for proof that assurance can only stay equal or weaken on
failure. Three places carry it:

1. The classifier fails closed. An edit the bounded oracle cannot relate is
   `unknown` and blocks; an exhausted step budget is `unknown`, never a
   guess; `unknown` forbids `allow` under every verb; an abstraction merge
   beyond G0 fails closed to `review`. Each is a re-run citation below.
2. The baseline fails closed. A contract that no longer decodes, a missing
   pin, a doubled pin, and a moved digest are each a test failure; there is
   no skip path (`t09_property_baseline.rs`).
3. This binding fails closed. `binding-fails-closed` in the self-test feeds
   `build_controls` one failed citation, then one tripped absence-scan, and
   requires the control, and the overall status, to read `fail` both times.

Self-test / real run
--------------------

`--self-test` proves every direct check catches a synthetic violation, every
Python delegate's own self-test still passes, and every cited Rust test still
passes when re-run narrowly. The real run re-runs the delegates and the Rust
tests, runs the direct checks against the live tree, runs the delta rule
against the merge base (skipped, with a reason, when no base resolves;
`--require-base` makes that skip fatal), and exits 1 if any `pass` or
`partial` control has a failing citation, or any gap's absence-scan finds a
hit.

Scope and honesty about limits
-------------------------------

- The baseline makes a property change a visible, named diff of a protected
  file. It does not authenticate who approved it. An agent with commit access
  can write a revision entry. The review gate (a human LGTM in seal, and the
  lead's security review) is what authenticates, and it lives outside the
  tree. The value of the pin is that the change cannot be silent.
- The absence-scans are canary-pattern detectors (as in T04). They are real
  evidence of today and go red on a producer under an anticipated name.
- The delta rule compares against the merge base only. History behind it is
  out of scope, as for GOV-1-08/09.

Stdlib only. Subprocesses: the delegate checkers, `cargo test` of tests
`just check` already runs, and `git`. No network.

Usage:

    python3 tools/governance/check_t09_evidence.py --self-test
    python3 tools/governance/check_t09_evidence.py
    python3 tools/governance/check_t09_evidence.py --base origin/main --require-base
    python3 tools/governance/check_t09_evidence.py --evidence tools/governance/evidence/t09.json
"""

from __future__ import annotations

import argparse
import copy
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

TOOL_DIR = Path(__file__).resolve().parent
ROOT = TOOL_DIR.parents[1]
sys.path.insert(0, str(TOOL_DIR))

import check_revision_delta as delta_mod  # noqa: E402

EVIDENCE = "tools/governance/evidence/t09.json"
THREAT_MODEL = "notes/plan/docs/09_THREAT_MODEL.md"
BASELINE = "tools/governance/t09-property-baseline.json"
JUSTFILE = "Justfile"
CLAIMS_MATRIX = "notes/plan/docs/18_CLAIMS_MATRIX.md"

CONTROLS: tuple[str, ...] = (
    "semantic property diff",
    "review gates",
    "immutable baseline property digest in CI",
    "mutation score",
    "agent cannot modify claim ledger or certificate",
)

PY_DELEGATES: dict[str, str] = {
    "check_obligations": "tools/governance/check_obligations.py",
    "check_claim_governance": "tools/governance/check_claim_governance.py",
    "check_crate_boundaries": "tools/check_crate_boundaries.py",
}

RustGroup = tuple[str, list[str], list[str]]

IMPL02 = "pr12_impl02_property_ast_edit_evidence"

RUST_DELEGATES: dict[str, RustGroup] = {
    "semantic_diff_weakening": (
        "continuum-semantic-diff",
        ["--test", IMPL02],
        [
            "positive_gutting_the_real_fixtures_agreement_claim_with_a_complementary_disjunct_classifies_weakened_and_blocks",
            "negative_a_double_negation_dressing_dissolves_and_the_weakening_underneath_classifies_weakened",
            "negative_a_vacuous_conjunct_game_never_classifies_unchanged",
            "negative_renaming_a_real_corpus_claim_while_weakening_it_is_removed_plus_added_never_unchanged",
            "negative_mutant_blind_to_expression_content_would_wrongly_pass_the_weakening_as_unchanged",
        ],
    ),
    "semantic_diff_exit": (
        "continuum-semantic-diff",
        ["--test", "pr12_exit_evidence"],
        [
            "live_weakening_a_property_is_a_privileged_change",
            "mutant_a_blind_property_classifier_flips_the_exit_and_is_rejected",
        ],
    ),
    "review_gates_intent": (
        "continuum-intent",
        ["--test", "inv001_protected_intent_evidence"],
        [
            "policy_decision::positive_each_of_the_six_verbs_forbids_allow_under_the_fixtures_own_policy",
            "policy_decision::positive_no_acceptance_path_reads_any_of_the_six_as_a_repair",
            "policy_decision::negative_a_blind_classifier_reading_an_attack_as_unchanged_is_detected",
            "daemon_boundary::boundary_accept_reject_lock_are_privileged_and_proposing_is_not",
        ],
    ),
    "baseline_digest": (
        "continuum-intent",
        ["--test", "t09_property_baseline"],
        [
            "the_baseline_pins_exactly_the_committed_intent_contracts",
            "every_pinned_contract_still_matches_its_baseline_digests",
            "the_baseline_names_the_production_hasher",
            "hostile_an_agent_authored_property_weakening_moves_the_claims_pin",
            "hostile_unlocking_the_properties_verb_moves_the_policy_pin",
            "boundary_metadata_and_display_source_do_not_move_any_pin",
            "negative_a_flipped_digest_and_a_dropped_entry_are_both_detected",
        ],
    ),
    "mutation_challenge_tripwire": (
        "continuum-intent",
        ["--test", "inv012_nonvacuity_evidence"],
        ["boundary_mutation_challenges_are_forges_own_responsibility_and_not_yet_landed"],
    ),
    "fail_closed_classifier": (
        "continuum-semantic-diff",
        ["--test", IMPL02],
        [
            "boundary_an_edit_the_oracle_cannot_relate_classifies_unknown_and_blocks",
            "boundary_the_step_budget_fails_closed_on_a_derivation_too_large_for_it",
        ],
    ),
    "fail_closed_exit": (
        "continuum-semantic-diff",
        ["--test", "pr12_exit_evidence"],
        ["live_beyond_g0_an_abstraction_merge_fails_closed_to_review"],
    ),
    "fail_closed_policy": (
        "continuum-intent",
        ["--test", "inv001_protected_intent_evidence"],
        ["policy_decision::positive_fail_closed_unknown_forbids_allow_and_correction_13_excludes_assurance_incomparable"],
    ),
    "ledger_authority": (
        "continuumd",
        ["--test", "inv015_agent_least_authority_evidence"],
        [
            "privileged_perimeter::positive_no_mutation_request_admits_a_caller_supplied_status",
            "status_authority::positive_the_promotion_witness_cannot_be_built_outside_the_verification_service",
            "receipt_authority::positive_receipt_appending_is_gated_to_service_actors_and_never_self_certified",
            "privileged_perimeter::positive_the_privileged_set_is_exactly_the_five_governance_operations",
            "no_widening::positive_privilege_is_never_acquired_by_default",
        ],
    ),
    "ledger_append_only": (
        "continuumd",
        ["--test", "inv009_monotonic_task_evidence"],
        ["positive_no_monotone_container_has_a_removal_verb", "negative_the_sweeps_are_not_vacuous"],
    ),
    "receipt_unforgeable": (
        "continuum-workspace",
        ["--test", "inv017_publication_atomicity_evidence"],
        [
            "mutant_a_forged_receipt_constructor_is_caught",
            "mutant_a_published_name_forged_without_a_receipt_is_caught",
        ],
    ),
    "certificate_wire_only": (
        "continuum-certificate",
        ["--test", "inv004_no_self_certification"],
        [
            "checking_has_exactly_one_public_entry_point_and_it_takes_wire_form_bytes",
            "no_public_item_collapses_the_four_outcomes_into_a_bool",
        ],
    ),
}


# ============================================================================
# Direct checks this file owns
# ============================================================================


def read(rel: str) -> str | None:
    path = ROOT / rel
    return path.read_text(encoding="utf-8") if path.is_file() else None


def t09_controls(text: str | None) -> list[str] | None:
    if text is None:
        return None
    match = re.search(r"^### T09 — Agent weakens property.*?$(.*?)(?=^### )", text, re.MULTILINE | re.DOTALL)
    if match is None:
        return None
    return [m.group(1).strip() for m in re.finditer(r"^- (.+?);?\.?$", match.group(1), re.MULTILINE)]


def rule_t09_control_list(text: str | None) -> list[str]:
    """docs/09's T09 control list is exactly CONTROLS, in order."""
    found = t09_controls(text)
    if found is None:
        return [f"{THREAT_MODEL} has no T09 section"]
    if found != list(CONTROLS):
        return [f"{THREAT_MODEL} T09 controls are {found!r}, this binding covers {list(CONTROLS)!r}"]
    return []


BONE_ID = re.compile(r"^bn-[0-9a-z]+$")
MIN_REASON = 24
DIGEST_KEYS = ("intent_identity", "claims", "policy")


def revision_findings(where: str, revision: object) -> list[str]:
    if not isinstance(revision, dict):
        return [f"{where}: no revision table"]
    out: list[str] = []
    bone = revision.get("bone")
    reason = revision.get("reason")
    if not isinstance(bone, str) or not BONE_ID.match(bone):
        out.append(f"{where}: revision.bone {bone!r} is not a bn- id")
    if not isinstance(reason, str) or len(reason.strip()) < MIN_REASON:
        out.append(f"{where}: revision.reason must say why in at least {MIN_REASON} characters")
    return out


def parse_baseline(text: str | None) -> tuple[dict | None, list[str]]:
    if text is None:
        return None, [f"{BASELINE} is missing"]
    try:
        doc = json.loads(text)
    except json.JSONDecodeError as exc:
        return None, [f"{BASELINE} does not parse: {exc}"]
    if not isinstance(doc, dict) or not isinstance(doc.get("entries"), list) or not isinstance(doc.get("retired", []), list):
        return None, [f"{BASELINE} needs an entries list and a retired list"]
    return doc, []


def rule_baseline_shape(text: str | None) -> list[str]:
    """Every pin names a path, three hex digests, and a named revision."""
    doc, errors = parse_baseline(text)
    if doc is None:
        return errors
    out: list[str] = []
    if doc.get("schema") != "continuum.governance.t09-property-baseline/1":
        out.append(f"{BASELINE}: unexpected schema {doc.get('schema')!r}")
    if not doc["entries"]:
        out.append(f"{BASELINE}: pins nothing")
    for index, entry in enumerate(doc["entries"]):
        where = f"entries[{index}]"
        if not isinstance(entry, dict) or not isinstance(entry.get("path"), str):
            out.append(f"{where}: no path")
            continue
        for key in DIGEST_KEYS:
            if not re.fullmatch(r"[0-9a-f]{64}", str(entry.get(key, ""))):
                out.append(f"{where} ({entry['path']}): {key} is not a 64-hex digest")
        out.extend(revision_findings(f"{where} ({entry['path']})", entry.get("revision")))
    for index, item in enumerate(doc.get("retired", [])):
        where = f"retired[{index}]"
        if not isinstance(item, dict) or not isinstance(item.get("path"), str):
            out.append(f"{where}: no path")
            continue
        out.extend(revision_findings(f"{where} ({item['path']})", item.get("revision")))
    return out


def rule_baseline_revision_delta(base_text: str | None, head_text: str | None) -> list[str]:
    """A pin that moved, appeared, or disappeared since the base carries a fresh, named revision.

    A pin absent at the base (including a base that predates the baseline) is new
    and needs a named revision. A pin whose digests moved needs a revision that
    differs from the base's: reusing the old revision is a silent move. A pin the
    base had and the head lacks needs a `retired` entry with its own revision.
    """
    head, errors = parse_baseline(head_text)
    if head is None:
        return errors
    base, _ = parse_baseline(base_text)
    base_entries = {e["path"]: e for e in (base or {}).get("entries", []) if isinstance(e, dict) and "path" in e}
    head_entries = {e["path"]: e for e in head["entries"] if isinstance(e, dict) and "path" in e}
    retired = {r["path"]: r for r in head.get("retired", []) if isinstance(r, dict) and "path" in r}
    out: list[str] = []
    for path, entry in sorted(head_entries.items()):
        before = base_entries.get(path)
        if before is None:
            out.extend(revision_findings(f"new pin {path}", entry.get("revision")))
            continue
        moved = [k for k in DIGEST_KEYS if entry.get(k) != before.get(k)]
        if not moved:
            continue
        where = f"moved pin {path} ({', '.join(moved)})"
        found = revision_findings(where, entry.get("revision"))
        if not found and entry.get("revision") == before.get("revision"):
            found = [f"{where}: the revision is the base's; a moved pin needs a new named revision"]
        out.extend(found)
    for path in sorted(set(base_entries) - set(head_entries)):
        item = retired.get(path)
        if item is None:
            out.append(f"pin {path} disappeared without a retired entry")
        else:
            out.extend(revision_findings(f"retired pin {path}", item.get("revision")))
    return out


def rule_justfile_runs_t09(text: str | None) -> list[str]:
    if text is None:
        return [f"{JUSTFILE} is missing"]
    out: list[str] = []
    check = re.search(r"^check:(.*)$", text, re.MULTILINE)
    deps = check.group(1).split() if check else []
    for dep in ("test", "governance"):
        if dep not in deps:
            out.append(f"`just check` no longer depends on {dep!r}")
    for line in (
        "python3 tools/governance/check_t09_evidence.py --self-test",
        "python3 tools/governance/check_t09_evidence.py\n",
    ):
        if line not in text:
            out.append(f"the governance recipe no longer runs {line.strip()!r}")
    return out


MUTATION_SCORE_PATTERNS: tuple[re.Pattern[str], ...] = (
    re.compile(r"\bmutation_score\b"),
    re.compile(r"\bMutationScore\b"),
    re.compile(r"\bkill_rate\b"),
    re.compile(r"\bKillRate\b"),
    re.compile(r"\bmutants_killed\b"),
)
MUTATION_TOOL_CONFIGS: tuple[str, ...] = (".cargo/mutants.toml", "mutants.toml")


def rule_mutation_score_gap(rust_sources: list[tuple[str, str]], config_paths: list[str]) -> list[str]:
    """Returns hits (evidence the gap is NOT real). Empty means the gap holds."""
    hits: list[str] = []
    for rel, text in rust_sources:
        for pattern in MUTATION_SCORE_PATTERNS:
            if pattern.search(text):
                hits.append(f"{rel}: matches {pattern.pattern!r}")
    for rel in config_paths:
        hits.append(f"{rel} exists: a mutation-testing tool is configured")
    return hits


CODEOWNERS_PATHS: tuple[str, ...] = (".github/CODEOWNERS", "CODEOWNERS", "docs/CODEOWNERS")


def rule_claim_registry_write_protection_gap(codeowners: list[str], baseline_text: str | None) -> list[str]:
    """Returns hits (evidence the gap is NOT real). Empty means the gap holds."""
    hits = [f"{rel} exists: path ownership may now protect the claims registry" for rel in codeowners]
    if baseline_text is not None and CLAIMS_MATRIX in baseline_text:
        hits.append(f"{BASELINE} now names {CLAIMS_MATRIX}")
    return hits


def real_rust_sources() -> list[tuple[str, str]]:
    out: list[tuple[str, str]] = []
    for path in sorted((ROOT / "crates").glob("*/src/**/*.rs")):
        try:
            out.append((path.relative_to(ROOT).as_posix(), path.read_text(encoding="utf-8")))
        except OSError:
            continue
    return out


def existing(paths: tuple[str, ...]) -> list[str]:
    return [p for p in paths if (ROOT / p).is_file()]


def direct_checks() -> dict[str, list[str]]:
    baseline_text = read(BASELINE)
    return {
        "t09-control-list": rule_t09_control_list(read(THREAT_MODEL)),
        "baseline-shape": rule_baseline_shape(baseline_text),
        "justfile-runs-t09": rule_justfile_runs_t09(read(JUSTFILE)),
        "mutation-score-gap": rule_mutation_score_gap(real_rust_sources(), existing(MUTATION_TOOL_CONFIGS)),
        "claim-registry-write-protection-gap": rule_claim_registry_write_protection_gap(
            existing(CODEOWNERS_PATHS), baseline_text
        ),
    }


# ============================================================================
# The delta rule, against the merge base
# ============================================================================


def delta_check(explicit_base: str | None, require_base: bool) -> dict[str, object]:
    git = delta_mod.Git(ROOT)
    resolution = delta_mod.resolve_base(git, explicit=explicit_base)
    if not resolution.resolved:
        fatal = resolution.fatal or require_base
        return {
            "status": "fail" if fatal else "skipped",
            "reason": resolution.reason,
            "findings": [],
        }
    rc, base_text = git("show", f"{resolution.rev}:{BASELINE}")
    findings = rule_baseline_revision_delta(base_text if rc == 0 else None, read(BASELINE))
    return {
        "status": "fail" if findings else "pass",
        "base": str(resolution.rev)[:12],
        "base_resolution": resolution.how,
        "findings": findings,
    }


# ============================================================================
# Self-test of the direct checks
# ============================================================================


def _pin(path: str, digit: str, bone: str = "bn-base", reason: str = "the pin this fixture starts from") -> dict:
    return {
        "path": path,
        "intent_identity": digit * 64,
        "claims": digit * 64,
        "policy": digit * 64,
        "revision": {"bone": bone, "reason": reason},
    }


def _doc(entries: list[dict], retired: list[dict] | None = None) -> str:
    return json.dumps(
        {"schema": "continuum.governance.t09-property-baseline/1", "entries": entries, "retired": retired or []}
    )


def delta_fixture_failures() -> list[str]:
    """The delta rule's own fixtures. Empty means every one behaves.

    The silent move, the unnamed new pin, and the unretired removal must trip; a
    move with a fresh named revision, a named retirement, a first pin, and no
    change must not. Pure functions over synthetic documents, so the binding
    re-runs them on every invocation, not only under `--self-test`.
    """
    base = _doc([_pin("a.json", "a"), _pin("b.json", "b")])
    silent = _doc([_pin("a.json", "c"), _pin("b.json", "b")])
    named = _doc(
        [_pin("a.json", "c", bone="bn-next", reason="Agreement narrowed by a reviewed revision"), _pin("b.json", "b")]
    )
    unnamed = json.loads(_doc([_pin("a.json", "a"), _pin("b.json", "b"), _pin("n.json", "d")]))
    del unnamed["entries"][2]["revision"]
    dropped = _doc([_pin("a.json", "a")])
    retired = _doc(
        [_pin("a.json", "a")],
        [{"path": "b.json", "revision": {"bone": "bn-next", "reason": "contract deleted with its reviewed revision"}}],
    )
    cases = [
        ("silent move", base, silent, True),
        ("named move", base, named, False),
        ("unnamed new pin", base, json.dumps(unnamed), True),
        ("unretired removal", base, dropped, True),
        ("named retirement", base, retired, False),
        ("first pin with names", None, base, False),
        ("no change", base, base, False),
    ]
    out: list[str] = []
    for label, before, after, want_hit in cases:
        found = rule_baseline_revision_delta(before, after)
        if bool(found) != want_hit:
            out.append(f"fixture {label}: baseline-revision-delta returned {found!r}")
    return out


def direct_self_test() -> dict[str, object]:
    failures: list[str] = []
    caught: list[str] = []
    baseline = direct_checks()
    dirty = {k: v for k, v in baseline.items() if v}
    if dirty:
        failures.append(f"the real repository already violates direct checks {sorted(dirty)}")

    def expect(name: str, findings: list[str], want_hit: bool, label: str) -> bool:
        if bool(findings) != want_hit:
            failures.append(f"fixture {label}: {name} returned {findings!r}")
            return False
        return True

    # t09-control-list: dropping one control from the section must trip it.
    threat = read(THREAT_MODEL) or ""
    anchor = "- mutation score;\n"
    if threat.count(anchor) != 1:
        failures.append(f"t09-control-list fixture anchor matches {threat.count(anchor)} times, expected 1")
    elif expect("t09-control-list", rule_t09_control_list(threat.replace(anchor, "", 1)), True, "dropped control"):
        caught.append("t09-control-list")

    # baseline-shape: a short reason and a non-hex digest must each trip it.
    bad_reason = _doc([_pin("a.json", "a", reason="short")])
    bad_digest = json.loads(_doc([_pin("a.json", "a")]))
    bad_digest["entries"][0]["claims"] = "not-hex"
    ok_shape = expect("baseline-shape", rule_baseline_shape(_doc([_pin("a.json", "a")])), False, "well-formed pin")
    if (
        ok_shape
        and expect("baseline-shape", rule_baseline_shape(bad_reason), True, "short reason")
        and expect("baseline-shape", rule_baseline_shape(json.dumps(bad_digest)), True, "non-hex digest")
        and expect("baseline-shape", rule_baseline_shape(None), True, "missing baseline")
    ):
        caught.append("baseline-shape")

    # baseline-revision-delta: see delta_fixture_failures.
    delta_failures = delta_fixture_failures()
    if delta_failures:
        failures.extend(delta_failures)
    else:
        caught.append("baseline-revision-delta")

    # justfile-runs-t09: dropping the real-run line must trip it.
    just = read(JUSTFILE) or ""
    line = "    python3 tools/governance/check_t09_evidence.py\n"
    if just.count(line) != 1:
        failures.append(f"justfile-runs-t09 fixture anchor matches {just.count(line)} times, expected 1")
    elif expect("justfile-runs-t09", rule_justfile_runs_t09(just.replace(line, "", 1)), True, "dropped real run"):
        caught.append("justfile-runs-t09")

    # mutation-score-gap: a producer identifier and a tool config must each trip it.
    if expect(
        "mutation-score-gap",
        rule_mutation_score_gap([("crates/x/src/lib.rs", "pub fn mutation_score() -> u32 { 0 }")], []),
        True,
        "producer identifier",
    ) and expect("mutation-score-gap", rule_mutation_score_gap([], [".cargo/mutants.toml"]), True, "tool config"):
        caught.append("mutation-score-gap")

    # claim-registry-write-protection-gap: a CODEOWNERS file must trip it.
    if expect(
        "claim-registry-write-protection-gap",
        rule_claim_registry_write_protection_gap([".github/CODEOWNERS"], None),
        True,
        "CODEOWNERS present",
    ):
        caught.append("claim-registry-write-protection-gap")

    # binding-fails-closed: one failed citation, then one tripped absence-scan,
    # must each turn a control and the overall status to `fail`.
    good_direct = {k: [] for k in baseline}
    good_py = {name: {"status": "pass"} for name in PY_DELEGATES}
    good_rust = {name: {"status": "pass"} for name in RUST_DELEGATES}
    healthy = build_controls(good_direct, good_py, good_rust)
    if any(c["status"] != "pass" for c in healthy.values()):
        failures.append(f"binding fixture: an all-green input did not bind green: {healthy}")
    else:
        bad_rust = copy.deepcopy(good_rust)
        bad_rust["semantic_diff_weakening"]["status"] = "fail"
        one = build_controls(good_direct, good_py, bad_rust)
        bad_direct = dict(good_direct, **{"mutation-score-gap": ["crates/x/src/lib.rs: mutation_score"]})
        two = build_controls(bad_direct, good_py, good_rust)
        if (
            one["semantic property diff"]["status"] == "fail"
            and overall(one) == "fail"
            and two["mutation score"]["status"] == "fail"
            and overall(two) == "fail"
        ):
            caught.append("binding-fails-closed")
        else:
            failures.append("binding fixture: a failed citation or a tripped gap did not fail the binding")

    return {"status": "fail" if failures else "pass", "checks_caught": sorted(caught), "failures": failures}


# ============================================================================
# Delegates
# ============================================================================


def run_py_delegate(rel: str, *args: str) -> tuple[int, dict | None]:
    proc = subprocess.run([sys.executable, str(ROOT / rel), *args], cwd=ROOT, capture_output=True, text=True, check=False)
    try:
        data = json.loads(proc.stdout) if proc.stdout.strip() else None
    except json.JSONDecodeError:
        data = None
    return proc.returncode, data


def py_delegate_self_test() -> dict[str, object]:
    results: dict[str, object] = {}
    failed: list[str] = []
    for name, rel in sorted(PY_DELEGATES.items()):
        code, data = run_py_delegate(rel, "--self-test")
        report = data if isinstance(data, dict) else {}
        status = report.get("status", report.get("self_test"))
        results[name] = {"exit_code": code, "status": status}
        if code != 0 or status != "pass":
            failed.append(f"{name} --self-test did not pass cleanly (exit {code}, status {status!r})")
    return {"status": "fail" if failed else "pass", "delegates": results, "failures": failed}


def py_delegate_real_run() -> dict[str, dict]:
    reports: dict[str, dict] = {}
    for name, rel in sorted(PY_DELEGATES.items()):
        code, data = run_py_delegate(rel)
        report = data if isinstance(data, dict) else {"status": "fail", "note": "no parseable JSON report"}
        if code != 0:
            report = dict(report, status="fail")
        reports[name] = report
    return reports


def run_rust_group(group: RustGroup) -> tuple[bool, str]:
    package, target_args, tests = group
    cmd = ["cargo", "test", "-p", package, *target_args, "--locked", "--", *tests, "--exact"]
    proc = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, check=False)
    output = proc.stdout + proc.stderr
    ok = proc.returncode == 0
    for test in tests:
        # A typo'd name matches zero tests and still exits 0, so every name must
        # print its own passing line.
        if f"test {test} ... ok" not in output:
            ok = False
    return ok, output


def result_line(output: str) -> str:
    """The last `test result:` line, without its wall-clock suffix (deterministic)."""
    lines = [line for line in output.splitlines() if line.startswith("test result:")]
    return re.sub(r"; finished in .*$", "", lines[-1]) if lines else "no test result line"


def rust_delegate_run() -> dict[str, dict]:
    results: dict[str, dict] = {}
    for name, group in sorted(RUST_DELEGATES.items()):
        ok, output = run_rust_group(group)
        results[name] = {
            "package": group[0],
            "tests": group[2],
            "status": "pass" if ok else "fail",
            "result": result_line(output),
        }
    return results


# ============================================================================
# The five-control binding
# ============================================================================


@dataclass(frozen=True)
class Citation:
    artifact: str
    enforces: str
    status: str


def overall(controls: dict[str, dict]) -> str:
    return "pass" if all(c["status"] == "pass" for c in controls.values()) else "fail"


def build_controls(direct: dict[str, list[str]], py: dict[str, dict], rust: dict[str, dict]) -> dict[str, dict]:
    def d(check: str) -> str:
        return "fail" if direct.get(check) else "pass"

    def r(name: str) -> str:
        return rust.get(name, {}).get("status", "unknown")

    def p(name: str) -> str:
        return py.get(name, {}).get("status", "unknown")

    impl02 = f"crates/continuum-semantic-diff/tests/{IMPL02}.rs"
    exit_ev = "crates/continuum-semantic-diff/tests/pr12_exit_evidence.rs"
    inv001 = "crates/continuum-intent/tests/inv001_protected_intent_evidence.rs"
    bound: dict[str, list[Citation]] = {
        "semantic property diff": [
            Citation(THREAT_MODEL, "T09's control list is exactly the five this binding covers", d("t09-control-list")),
            Citation(
                f"{impl02} (five tests)",
                "an agent-authored complementary-disjunct weakening of a real claim is an affirmed `weakened` and blocks under `locked`; double-negation, vacuous-conjunct, and rename disguises fail; a content-blind classifier mutant is caught",
                r("semantic_diff_weakening"),
            ),
            Citation(
                f"{exit_ev} (two tests)",
                "weakening a property is a privileged change at production grain; a blind property classifier flips the exit and is rejected",
                r("semantic_diff_exit"),
            ),
        ],
        "review gates": [
            Citation(
                f"{inv001} (four tests)",
                "each of the six INV-001 verbs forbids `allow` on every acceptance path; none reads as a repair; accept/reject/lock are privileged and proposing is not; a blind classifier is detected",
                r("review_gates_intent"),
            ),
            Citation(
                f"{impl02} positive_gutting_…_classifies_weakened_and_blocks",
                "under a `review` verb the same weakening closes to `review` with named reviewers, never `allow`",
                r("semantic_diff_weakening"),
            ),
            Citation(
                "tools/governance/check_obligations.py",
                "a semantic-core change ships a GOV §4 review record (reference, metamorphic, differential tests; schemas; migration; security review)",
                p("check_obligations"),
            ),
        ],
        "immutable baseline property digest in CI": [
            Citation(
                "crates/continuum-intent/tests/t09_property_baseline.rs (seven tests)",
                "every committed Intent Contract is pinned exactly once by identity, claims, and policy digests the real decoder computes; the complementary-disjunct weakening moves the claims pin; unlocking `properties` moves the policy pin; metadata does not",
                r("baseline_digest"),
            ),
            Citation(BASELINE, "every pin carries three hex digests and a named revision", d("baseline-shape")),
            Citation(
                "tools/governance/check_t09_evidence.py baseline-revision-delta",
                "against the merge base, a moved, new, or retired pin carries a fresh named revision; its seven fixtures are re-run here, and its live verdict fails the run (stdout, exit code)",
                "fail" if delta_fixture_failures() else "pass",
            ),
            Citation(JUSTFILE, "`just check` runs `test` and this checker; CI runs `just check`", d("justfile-runs-t09")),
        ],
        "agent cannot modify claim ledger or certificate": [
            Citation(
                "crates/continuumd/tests/inv015_agent_least_authority_evidence.rs (five tests)",
                "no request admits a caller-supplied status; the Promotion witness is unreachable outside the verification service; receipt appending is service-only and never self-certified; privilege is never default",
                r("ledger_authority"),
            ),
            Citation(
                "crates/continuumd/tests/inv009_monotonic_task_evidence.rs (two tests)",
                "the append-only evidence containers admit no removal verb, with mutants",
                r("ledger_append_only"),
            ),
            Citation(
                "crates/continuum-workspace/tests/inv017_publication_atomicity_evidence.rs (two mutants)",
                "a forged receipt constructor and a Published name forged without a receipt are both caught",
                r("receipt_unforgeable"),
            ),
            Citation(
                "crates/continuum-certificate/tests/inv004_no_self_certification.rs (two tests)",
                "the certificate checker has one entry point over wire-form bytes and collapses no outcome into a bool",
                r("certificate_wire_only"),
            ),
            Citation("tools/check_crate_boundaries.py", "no self-certification crate edges (INV-004)", p("check_crate_boundaries")),
            Citation(
                "tools/governance/check_claim_governance.py",
                "the docs/18 claims registry and its PLAN_REQUIREMENTS mirror agree row for row, so a one-sided edit fails",
                p("check_claim_governance"),
            ),
        ],
    }

    fail_closed = [
        Citation(f"{impl02} (two boundary tests)", "an unrelatable edit and an exhausted step budget are `unknown` and block", r("fail_closed_classifier")),
        Citation(f"{exit_ev} live_beyond_g0_…_fails_closed_to_review", "an abstraction merge beyond G0 fails closed to review", r("fail_closed_exit")),
        Citation(f"{inv001} positive_fail_closed_unknown_forbids_allow_…", "`unknown` forbids `allow` under every verb", r("fail_closed_policy")),
    ]

    controls: dict[str, dict] = {}
    for name in ("semantic property diff", "review gates", "immutable baseline property digest in CI"):
        cites = bound[name]
        controls[name] = {
            "type": "pass",
            "citations": [c.__dict__ for c in cites],
            "status": "pass" if all(c.status == "pass" for c in cites) else "fail",
        }

    mutation_checks = [
        {
            "artifact": "tools/governance/check_t09_evidence.py mutation-score-gap",
            "checks": "no crates/*/src identifier names a mutation score or kill rate; no cargo-mutants configuration exists",
            "status": d("mutation-score-gap"),
        },
        {
            "artifact": "crates/continuum-intent/tests/inv012_nonvacuity_evidence.rs boundary_mutation_challenges_are_forges_own_responsibility_and_not_yet_landed",
            "checks": "plan §14.5's property-mutation challenge is still Forge's and still not landed",
            "status": r("mutation_challenge_tripwire"),
        },
    ]
    controls["mutation score"] = {
        "type": "gap",
        "reason": (
            "No producer computes a per-property mutation score. The mutants that exist are "
            "anti-vacuity mutants of the classifiers, the kernel mutation-class matrix "
            "(certificates, KCOV-08), and docs/19 §4's semantic-engine mutants; none seeds "
            "program mutants and reports the fraction a property kills. Forge's plan §14.5 "
            "mutation challenges are not landed."
        ),
        "absence_checks": mutation_checks,
        "status": "pass" if all(c["status"] == "pass" for c in mutation_checks) else "fail",
    }

    ledger = bound["agent cannot modify claim ledger or certificate"]
    ledger_gap = [
        {
            "artifact": "tools/governance/check_t09_evidence.py claim-registry-write-protection-gap",
            "checks": "no CODEOWNERS file exists and the T09 baseline does not pin docs/18: the repository claims registry has no in-tree write protection",
            "status": d("claim-registry-write-protection-gap"),
        },
    ]
    controls["agent cannot modify claim ledger or certificate"] = {
        "type": "partial",
        "citations": [c.__dict__ for c in ledger],
        "reason": (
            "Bound at the daemon and certificate grain. The repository claims registry "
            "(docs/18 and its PLAN_REQUIREMENTS mirror) is writable by any committer: the "
            "mirror check fails a one-sided edit and review gates a two-sided one, but "
            "nothing in the tree refuses an agent commit that edits both."
        ),
        "absence_checks": ledger_gap,
        "status": "pass"
        if all(c.status == "pass" for c in ledger) and all(c["status"] == "pass" for c in ledger_gap)
        else "fail",
    }

    controls["(failure monotonicity)"] = {
        "type": "pass",
        "citations": [c.__dict__ for c in fail_closed],
        "status": "pass" if all(c.status == "pass" for c in fail_closed) else "fail",
    }
    return controls


# ============================================================================
# Evidence and entry point
# ============================================================================


def build_evidence(controls: dict[str, dict], direct_st: dict, py_st: dict, rust: dict[str, dict]) -> dict:
    rust_ok = all(v.get("status") == "pass" for v in rust.values())
    return {
        "artifact": "continuum.governance.evidence/t09",
        "produced_by": "tools/governance/check_t09_evidence.py",
        "reproduce": f"python3 tools/governance/check_t09_evidence.py --evidence {EVIDENCE}",
        "authority": f"{THREAT_MODEL} T09 (Agent weakens property)",
        "determinism": (
            "No timestamp, commit id, or absolute path is recorded, and the merge-base delta "
            "verdict goes to stdout only: the file is a function of the tree and the delegates' "
            "outputs. Each Rust group records only its `test result:` line, without wall-clock time."
        ),
        "py_delegates": sorted(PY_DELEGATES.values()),
        "rust_delegates": {n: {"package": g[0], "args": g[1], "tests": g[2]} for n, g in sorted(RUST_DELEGATES.items())},
        "controls": controls,
        "self_test": {
            "direct": direct_st,
            "py_delegates": py_st,
            "rust_delegates_rerun": rust,
            "status": "pass" if direct_st.get("status") == "pass" and py_st.get("status") == "pass" and rust_ok else "fail",
        },
        "boundary": (
            "Three of five controls are bound to re-run enforcement. 'mutation score' is a typed "
            "gap with a mechanical absence-scan. 'agent cannot modify claim ledger or "
            "certificate' is partial: bound at the daemon and certificate grain, with the "
            "repository claims registry's missing write protection recorded as a checked "
            "absence. The baseline pin makes a property change visible and named; it does not "
            "authenticate the approver, which is the review gate's job outside the tree."
        ),
        "status": overall(controls),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--self-test", action="store_true", help="prove the direct checks, delegate self-tests, and cited Rust tests")
    parser.add_argument("--evidence", nargs="?", const=str(ROOT / EVIDENCE), default=None, metavar="PATH")
    parser.add_argument("--base", default=None, help="explicit merge base for the baseline-revision-delta rule")
    parser.add_argument("--require-base", action="store_true", help="fail, not skip, when no base resolves")
    parser.add_argument("--quiet", action="store_true")
    args = parser.parse_args(argv)

    if args.self_test and not args.evidence:
        direct_st = direct_self_test()
        py_st = py_delegate_self_test()
        rust = rust_delegate_run()
        rust_ok = all(v.get("status") == "pass" for v in rust.values())
        status = "pass" if direct_st["status"] == "pass" and py_st["status"] == "pass" and rust_ok else "fail"
        print(json.dumps({"status": status, "direct": direct_st, "py_delegates": py_st, "rust_delegates": rust}, indent=2, sort_keys=True))
        return 0 if status == "pass" else 1

    direct_st: dict[str, object] = {"status": "not-run"}
    py_st: dict[str, object] = {"status": "not-run"}
    st_ok = True
    if args.evidence:
        direct_st = direct_self_test()
        py_st = py_delegate_self_test()
        st_ok = direct_st["status"] == "pass" and py_st["status"] == "pass"

    direct = direct_checks()
    py = py_delegate_real_run()
    rust = rust_delegate_run()
    controls = build_controls(direct, py, rust)
    delta = delta_check(args.base, args.require_base)

    if args.evidence:
        path = Path(args.evidence)
        if not path.is_absolute():
            path = ROOT / path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(build_evidence(controls, direct_st, py_st, rust), indent=2, sort_keys=True) + "\n", encoding="utf-8")
        st_ok = st_ok and all(v.get("status") == "pass" for v in rust.values())

    failing = sorted(k for k, v in controls.items() if v["status"] != "pass")
    bad = bool(failing) or not st_ok or delta["status"] == "fail"
    report = {
        "controls": {k: v["status"] for k, v in sorted(controls.items())},
        "failing_controls": failing,
        "direct_findings": {k: v for k, v in direct.items() if v},
        "baseline_revision_delta": delta,
        "self_test": {"direct": direct_st.get("status"), "py_delegates": py_st.get("status")},
        "evidence": args.evidence,
        "status": "fail" if bad else "pass",
    }
    print(report["status"] if args.quiet else json.dumps(report, indent=2, sort_keys=True))
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
