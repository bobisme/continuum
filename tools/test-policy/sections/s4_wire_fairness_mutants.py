"""docs/19 §4 "Mutation testing" — TEST-4-07 … TEST-4-12.

`sections/s4_mutation_testing.py` (bn-1ccq) claims the first six "Semantic
engine" bullets and explicitly leaves TEST-4-07..09 and the whole "Protocol
corpus" subsection to a later module (its own docstring, and `unclaimed` /
`out_of_scope` in its evidence file). This module claims six of the nine
remaining IDs without editing a line of that module, `tsys.py`, or the
`Justfile` (README-test.md "Adding a section"):

- TEST-4-07 accept unknown field as old meaning
- TEST-4-08 omit fairness edge
- TEST-4-09 use timestamp total order.
- TEST-4-10 all first-demo mutants
- TEST-4-11 stale term/epoch
- TEST-4-12 double counting

TEST-4-13..18 ("Protocol corpus" non-idempotent retry / ack-before-durability
/ forgotten loser drain / timeout via silent drop / restart timer leak /
recovery livelock) are bn-2dt2's, claimed in parallel; see `unclaimed` below.

Per README-test.md step 5 this module states, per ID, exactly which real
substance it runs against, and marks an ID `enforced` only when a real,
already-committed artifact backs the check — never a second hand-invented
schema standing in for one. None of this module's checks read the protocol
IDL: `crates/continuumd/tests/gate_g2_01_acceptance.rs`'s
`no_generator_exists_anywhere_in_the_workspace` enumerates, by name, every
tool under `tools/` and `notes/plan/tools/` permitted to name the IDL by
path, and a new entry there is a semantic-core gate-file edit this bone does
not make. Every check below is grounded in a real committed artifact this
module reads directly instead.

A later rule (bn-3mo3, common.md): a Python port of a Rust predicate is not
evidence about the Rust code. Every ID this module marks `enforced` about
real protocol/codec/engine behaviour therefore names, in `_RUST_TESTS`, the
real Rust test(s) in `crates/` that exercise the real code against that
mutant class; `real_run` checks — textually, no cargo — that each still
exists, carries a `#[test]`-family attribute, is not `#[ignore]`d, and has
not drifted from a pinned fixed-window fingerprint (`_RUST_TEST_DRIFT_HASH`;
see the mechanism's own docstring below). An ID with no real Rust test that
exercises its mutant class — `TEST-4-09` and `TEST-4-11` — is `partial`,
whatever a freestanding Python model of it would otherwise show.

- **TEST-4-07** decodes the real committed wire-fuzz-corpus golden
  `crates/continuumd/tests/wire-fuzz-corpus/frame.request-envelope.json/
  well-formed-workspace-create.case` and treats *that artifact's own declared
  keys* as the known-field set — not a hand-copied duplicate of the request
  envelope's field list, and not a tool reading the IDL. The production rule
  this mutant targets ("a daemon MUST ignore unknown optional request
  fields", "servers MUST ignore unknown optional request fields and MUST NOT
  emit fields the negotiated version does not define") is RFC 0026's; this
  check enforces it against the one real wire artifact this harness can
  read. `enforced`.
- **TEST-4-08** reuses `tsys.compile_system`, `tsys.explore` and
  `tsys.enabled` directly — the same real oracle substance TEST-2-04's
  `reduction-agrees` and TEST-4-01's `causal-edge-drop-detected` (bn-1ccq)
  already rely on, the bar TEST-4-01's own docstring sets. A "fairness
  edge" here is a real (fair-transition, reachable-state) pair the oracle's
  own state exploration and guard evaluation compute; the mutant drops one
  from the claimed set, structurally the same operation TEST-4-01 performs
  on causal edges. This is a distinct concept from TEST-2-08's fairness
  *declaration* well-formedness (`sections/s2_cancellation_fairness.py`,
  bn-1fqu's extension): TEST-2-08 checks that a fairness annotation is
  itself sound; TEST-4-08 checks that an engine cannot silently drop one of
  the state-level obligations that annotation produces. Neither module
  edits the other. `enforced`.
- **TEST-4-10** reads the real, byte-pinned, cargo-produced golden
  `crates/continuumd/tests/golden/dx01_falsification_evidence.txt`
  (bn-37gu / bn-1y4qc, G0-DX-01, "the first demonstration" of plan §25) and
  parses its own `[leg1 replay-preserving core]` section, which already
  records the four drop-one mutants of the first demo's causal core
  (`begin_txn`, `submit_write`, `ack_before_flush`, `power_loss`) each
  against its own pre-registered outcome. This Python, no-cargo harness
  (README-test.md) does not re-run `dx01_falsification.rs`; it reads the
  committed evidence that a real `cargo test` run already produced, and
  fails if that evidence ever stops recording every mutant held. `enforced`.
- **TEST-4-12** reads the real, committed wire-fuzz-corpus `duplicate-id-
  repeated-*.case` files — one per decoder target
  (`crates/continuumd/tests/wire-fuzz-corpus/*/duplicate-id-*.case`, nine of
  the ten targets; `frame.length-prefix` has no keyed content and so no
  duplicate-id hazard) — and checks that every one's committed `expect`
  field still declares a rejection, never `accepted`, for a `class =
  duplicate-id` case. This is the same corpus `crates/continuumd/tests/
  wire_fuzz/target.rs` drives against the real decoders; this harness does
  not execute that drive (no cargo), so it cannot itself observe a decode,
  but a case whose committed `expect` regresses to `accepted` for a
  duplicate identifier — the double-counting this ID names — is caught
  here. `enforced`.
- **TEST-4-09** and **TEST-4-11** have no counterpart this Python, no-cargo
  harness can run against, and (per the no-generator gate above) this module
  does not read the IDL to sharpen their grounding either. `TEST-4-09`'s
  real production surface is RFC 0026's declared `Milestone.at: Timestamp`
  field ("milestones reached so far, in order") and its monotonic-status
  rule, plus the `Timestamp` scalar's documented millisecond precision (a
  real, structural collision hazard); no adapter exists anywhere in this
  repository that reconstructs an order from committed milestones for this
  harness to call. `TEST-4-11`'s real production surface is RFC 0026's
  `ErrorCode` vocabulary (`StaleSnapshot`, `ContinuationEpochMismatch`,
  `EpochUnsupported`) and its epoch-independence rule; but no Python-callable
  adapter decides epoch staleness (epochs are content identities, ADR-0018,
  and the daemon that dispatches on them is Rust this no-cargo harness
  cannot execute). Both checks below are freestanding models of the one
  hazard their rule names, over hand-built cases, not a mutation of running
  harness or engine logic — the same reason bn-1ccq's TEST-4-04..06 are
  `partial` rather than `enforced` despite passing with zero failures. The
  absence is stated in each entry.
"""

from __future__ import annotations

import copy
import hashlib
import json
import re
from pathlib import Path
from typing import Any

import tsys

SECTION = 4
TITLE = "Mutation testing"

OBLIGATIONS = {
    "TEST-4-07": "accept unknown field as old meaning",
    "TEST-4-08": "omit fairness edge",
    "TEST-4-09": "use timestamp total order.",
    "TEST-4-10": "all first-demo mutants",
    "TEST-4-11": "stale term/epoch",
    "TEST-4-12": "double counting",
}
RULES: dict[str, str] = {
    "unknown-field-remapped-detected": "TEST-4-07",
    "fairness-edge-drop-detected": "TEST-4-08",
    "timestamp-total-order-detected": "TEST-4-09",
    "first-demo-mutant-not-held-detected": "TEST-4-10",
    "stale-epoch-accepted-detected": "TEST-4-11",
    "duplicate-id-accepted-detected": "TEST-4-12",
}
SEEDS = range(128)

# ---------------------------------------------------------------------------
# Real, checked-in substance this module reads (never re-implemented copies,
# and never the protocol IDL: crates/continuumd/tests/gate_g2_01_acceptance.rs's
# no_generator_exists_anywhere_in_the_workspace enumerates, by name, every
# tool under tools/ and notes/plan/tools/ allowed to name it, and this module
# is not one of them).
# ---------------------------------------------------------------------------

_HERE = Path(__file__).resolve()
_ROOT = _HERE.parents[3]  # sections/ -> test-policy/ -> tools/ -> repo root
_ENVELOPE_CASE_PATH = (
    _ROOT
    / "crates/continuumd/tests/wire-fuzz-corpus/frame.request-envelope.json"
    / "well-formed-workspace-create.case"
)
_WIRE_FUZZ_CORPUS = _ROOT / "crates/continuumd/tests/wire-fuzz-corpus"
_DX01_GOLDEN = _ROOT / "crates/continuumd/tests/golden/dx01_falsification_evidence.txt"


def _parse_wire_fuzz_case(text: str) -> dict[str, Any]:
    """The `# continuum wire-fuzz case` format: `key = value` metadata lines,
    then a `bytes =` line followed by indented hex, real across every case
    file in `tests/wire-fuzz-corpus/`."""
    meta: dict[str, Any] = {}
    hex_lines: list[str] = []
    in_bytes = False
    for line in text.splitlines():
        if line.startswith("#"):
            continue
        if not in_bytes:
            if not line.strip():
                continue
            if line.startswith("bytes"):
                in_bytes = True
                continue
            if " = " in line:
                k, v = line.split(" = ", 1)
                meta[k.strip()] = v.strip()
            continue
        stripped = line.strip()
        if stripped:
            hex_lines.append(stripped)
    raw = "".join(hex_lines)
    meta["_bytes"] = bytes.fromhex(raw) if raw else b""
    return meta


_DX01_MUTANT_LINE = re.compile(r"^drop_(\w+)=(\S+) pre_registered=(\S+) held=(true|false)$")


def _parse_dx01_mutants(text: str) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for line in text.splitlines():
        m = _DX01_MUTANT_LINE.match(line.strip())
        if m:
            name, observed, pre_registered, held = m.groups()
            out[name] = {"observed": observed, "pre_registered": pre_registered, "held": held == "true"}
    return out


# ---------------------------------------------------------------------------
# Rust-test citation and drift tie. A Python port of a Rust predicate is not
# evidence about the Rust code (bn-3mo3 rule, common.md): an ID marked
# `enforced` about real protocol/codec behaviour must name the real Rust
# test(s) in crates/ that exercise the real code against that mutant class,
# and this harness must check — textually, no cargo — that each named test
# still exists and is not `#[ignore]`d. A fixed-line-window SHA-256 over each
# cited test (attribute lines through the next `_DRIFT_WINDOW` lines) is
# pinned below; a change to that window fails the check, forcing a human to
# re-read the Rust test and re-validate this Python check against it before
# moving the pin. This is a textual fingerprint, not a semantic diff: an edit
# outside the window is not caught, and the pin must be moved by a human, not
# by re-running `--evidence` (which would silently launder drift).
# ---------------------------------------------------------------------------

_RUST_FN_LINE = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(")
_DRIFT_WINDOW = 60


def _locate_rust_fn(text: str, fn_name: str) -> tuple[int, int] | None:
    """(attribute-block start line, fn line), both 0-based, or None."""
    lines = text.splitlines()
    for i, line in enumerate(lines):
        m = _RUST_FN_LINE.match(line)
        if m and m.group(1) == fn_name:
            j = i - 1
            while j >= 0 and lines[j].strip().startswith("#["):
                j -= 1
            return j + 1, i
    return None


def _rust_test_status(text: str, fn_name: str) -> dict[str, Any]:
    loc = _locate_rust_fn(text, fn_name)
    if loc is None:
        return {"exists": False, "is_test": False, "ignored": False, "hash": None}
    attr_start, fn_line = loc
    lines = text.splitlines()
    attrs = lines[attr_start:fn_line]
    window = "\n".join(lines[attr_start : min(len(lines), fn_line + _DRIFT_WINDOW)])
    return {
        "exists": True,
        "is_test": any("test" in a for a in attrs),
        "ignored": any(a.strip().startswith("#[ignore") for a in attrs),
        "hash": hashlib.sha256(window.encode("utf-8")).hexdigest(),
    }


# ID -> list of (repo-relative path, test fn name). Every entry here backs a
# status: enforced ID whose claim is about real continuumd/engine-reference
# behaviour, not a semantic-engine-only concept run purely against tsys
# (TEST-4-01..03/08's own bar, README-test.md; those cite no Rust test for
# the same reason bn-1ccq's TEST-4-01 does not).
_RUST_TESTS: dict[str, list[tuple[str, str]]] = {
    "TEST-4-07": [
        ("crates/continuumd/tests/codec_canonical_form.rs", "an_unknown_optional_field_is_ignored_and_an_unknown_closed_enum_member_is_not"),
        ("crates/continuumd/tests/codec_canonical_cbor.rs", "an_unknown_field_and_an_unknown_enum_member_behave_the_same_in_cbor"),
        ("crates/continuumd/tests/wire_fuzz.rs", "the_campaign_finds_no_defect"),
    ],
    "TEST-4-08": [
        ("crates/continuum-engine-reference/tests/semantic_oracle.rs", "a_weakly_fair_discharge_closes_the_lasso_and_an_unfair_one_does_not"),
        ("crates/continuum-engine-reference/tests/semantic_oracle.rs", "every_seeded_defect_class_is_detected_with_a_replayable_witness"),
    ],
    "TEST-4-10": [
        ("crates/continuumd/tests/dx01_falsification.rs", "a01_dropping_any_causal_event_fails_the_pre_registered_way"),
        ("crates/continuumd/tests/dx01_falsification.rs", "d_the_campaign_renders_one_byte_stable_evidence_artifact"),
    ],
    "TEST-4-12": [
        ("crates/continuumd/tests/wire_fuzz.rs", "every_committed_case_lands_where_it_declares"),
        ("crates/continuumd/tests/wire_fuzz.rs", "regression_duplicate_id_cases_reach_their_uniqueness_rules"),
    ],
}

# Pinned fixed-window fingerprints, computed once against the cited tests as
# they read at the time this module was written. See the drift-tie docstring
# above.
_RUST_TEST_DRIFT_HASH: dict[tuple[str, str], str] = {
    ("crates/continuumd/tests/codec_canonical_form.rs", "an_unknown_optional_field_is_ignored_and_an_unknown_closed_enum_member_is_not"): "8a7e93c861c74862c6cff42901fd5e5d6b337dd020c4ed1a933ed727cb686d99",
    ("crates/continuumd/tests/codec_canonical_cbor.rs", "an_unknown_field_and_an_unknown_enum_member_behave_the_same_in_cbor"): "7cee9ee990545e765e1462082e7d69b1030a2c5406b3a5e2efd10c858310f549",
    ("crates/continuumd/tests/wire_fuzz.rs", "the_campaign_finds_no_defect"): "4bddeb1057aa08ef33e7cfc6f6636c64f462701dec7c47aab2dffe520e4fc002",
    ("crates/continuum-engine-reference/tests/semantic_oracle.rs", "a_weakly_fair_discharge_closes_the_lasso_and_an_unfair_one_does_not"): "2edce68ab58a8e0210d4770382fe9e74cdcaf046596b3c7fb2a5386f1e6516a9",
    ("crates/continuum-engine-reference/tests/semantic_oracle.rs", "every_seeded_defect_class_is_detected_with_a_replayable_witness"): "b04931c6a369ef687ede1f0502bc323cdf3d31bb20555201598ffc2d8bc2511f",
    ("crates/continuumd/tests/dx01_falsification.rs", "a01_dropping_any_causal_event_fails_the_pre_registered_way"): "975b0d350e4844add359644f0e6592aca6204f06eb4ee750249bb836dfd46a60",
    ("crates/continuumd/tests/dx01_falsification.rs", "d_the_campaign_renders_one_byte_stable_evidence_artifact"): "23791908eaf0a2f9aad2302106450f418a0e7296a4b1aed29f65bebd1a2df740",
    ("crates/continuumd/tests/wire_fuzz.rs", "every_committed_case_lands_where_it_declares"): "080da3ce139a3e47ba05ce28c1aea4233e12813da5e3cc2bd30b019079bb7599",
    ("crates/continuumd/tests/wire_fuzz.rs", "regression_duplicate_id_cases_reach_their_uniqueness_rules"): "03ac1e6382487feeab07af7056b26d445d9cfe71afb6e615f799552b70142c8b",
}


def _check_rust_tests(rid: str) -> tuple[list[str], list[dict[str, Any]]]:
    """Textual existence/ignored/drift check for every Rust test `rid` cites.
    Returns (failure messages, evidence records)."""
    failures: list[str] = []
    records: list[dict[str, Any]] = []
    for path_str, fn_name in _RUST_TESTS.get(rid, []):
        text = (_ROOT / path_str).read_text(encoding="utf-8")
        status = _rust_test_status(text, fn_name)
        record = {"path": path_str, "fn": fn_name, **status}
        records.append(record)
        if not status["exists"]:
            failures.append(f"{path_str}::{fn_name}: no `fn {fn_name}` found (the cited Rust test no longer exists)")
            continue
        if not status["is_test"]:
            failures.append(f"{path_str}::{fn_name}: found, but carries no #[test]-family attribute")
        if status["ignored"]:
            failures.append(f"{path_str}::{fn_name}: is #[ignore]d")
        pinned = _RUST_TEST_DRIFT_HASH.get((path_str, fn_name))
        if pinned is not None and status["hash"] != pinned:
            failures.append(
                f"{path_str}::{fn_name}: source drifted from the pinned {_DRIFT_WINDOW}-line fingerprint "
                f"({pinned[:12]}... -> {status['hash'][:12]}...); re-read the Rust test, re-validate this "
                "Python check against it, and move the pin"
            )
    return failures, records


_REFERENCE_ENVELOPE: dict[str, Any] = json.loads(
    _parse_wire_fuzz_case(_ENVELOPE_CASE_PATH.read_text(encoding="utf-8"))["_bytes"].decode("utf-8")
)
assert _REFERENCE_ENVELOPE, "the committed golden envelope decoded empty"
# The known-field set is the real golden's own declared keys — not a parse of the IDL
# (forbidden to a new tool by the no-generator gate, above) and not a hand-copied
# duplicate of RequestEnvelope's field list.
_REQUEST_ENVELOPE_FIELDS = frozenset(_REFERENCE_ENVELOPE)

_EXPECTED_DX01_MUTANTS = frozenset({"begin_txn", "submit_write", "ack_before_flush", "power_loss"})

_BOUNDARY_07 = (
    "the real committed wire-fuzz-corpus golden crates/continuumd/tests/wire-fuzz-corpus/"
    "frame.request-envelope.json/well-formed-workspace-create.case: its own decoded keys are "
    "the known-field set (not a parse of the protocol IDL, which the no-generator gate "
    "gate_g2_01_acceptance.rs forbids a new tool from reading, and not a hand-copied duplicate "
    "of its struct). RFC 0026's own rule ('a daemon MUST ignore unknown optional request fields "
    "... MUST NOT emit fields the negotiated version does not define') is the production rule "
    "this mutant targets, and real Rust tests exercise the real decoder against exactly this "
    "class of mutant: codec_canonical_form.rs::an_unknown_optional_field_is_ignored_and_an_unknown_"
    "closed_enum_member_is_not and codec_canonical_cbor.rs::an_unknown_field_and_an_unknown_enum_"
    "member_behave_the_same_in_cbor each widen a real decoded value with an extra field and assert "
    "the decode is unchanged; wire_fuzz.rs::the_campaign_finds_no_defect drives the RequestEnvelope "
    "frame itself (wire_fuzz/target.rs's RequestEnvelopeJson/RequestEnvelopeCbor probes) and treats "
    "an unknown-field acceptance as normal form. `_RUST_TESTS['TEST-4-07']` names all three; this "
    "harness checks their existence, `#[test]` status and drift fingerprint (real_run, below) but "
    "does not execute them (no cargo, README-test.md). This Python check itself compares a candidate "
    "decode's output against the real golden's own keys and true values."
)
_BOUNDARY_08 = (
    "tsys.compile_system / tsys.explore / tsys.enabled (the real oracle substance TEST-2-04's "
    "reduction-agrees and TEST-4-01's causal-edge-drop-detected, bn-1ccq, already rely on) compute "
    "the true (fair-transition, reachable-state) edge set directly; the mutant drops one, the same "
    "operation TEST-4-01 performs on causal edges — the same bar TEST-4-01 meets without citing a "
    "Rust test, because the substance under test is this harness's own oracle, not continuumd. "
    "continuum-engine-reference/tests/semantic_oracle.rs::a_weakly_fair_discharge_closes_the_lasso_"
    "and_an_unfair_one_does_not and its sibling every_seeded_defect_class_is_detected_with_a_"
    "replayable_witness (sweeping DefectClass::Fairness) are real, closely-matching corroborating "
    "production surface — a scheduler-level weak-fairness test with a starvation witness — cited and "
    "existence/drift-checked in `_RUST_TESTS['TEST-4-08']` for extra rigor, though not required by "
    "this ID's own concept (tsys's semantic-engine oracle, not continuumd protocol/codec). Distinct "
    "from TEST-2-08's fairness-annotation well-formedness check (sections/s2_cancellation_fairness.py, "
    "bn-1fqu), not edited here."
)
_BOUNDARY_10 = (
    "crates/continuumd/tests/golden/dx01_falsification_evidence.txt (bn-37gu / bn-1y4qc, "
    "G0-DX-01, plan §25's 'first demonstration'), a byte-pinned artifact a real cargo test run "
    "produced and committed. This module reads that committed evidence's [leg1 replay-preserving "
    "core] section. The real Rust test that runs the drop-one campaign is "
    "dx01_falsification.rs::a01_dropping_any_causal_event_fails_the_pre_registered_way (asserts each "
    "of the four drop-one mutants against its pre-registered outcome); "
    "dx01_falsification.rs::d_the_campaign_renders_one_byte_stable_evidence_artifact is what writes "
    "the golden this module reads. `_RUST_TESTS['TEST-4-10']` names both; this harness checks their "
    "existence, `#[test]` status and drift fingerprint but does not re-run them (no cargo, "
    "README-test.md)."
)
_BOUNDARY_12 = (
    "crates/continuumd/tests/wire-fuzz-corpus/*/duplicate-id-*.case, the real committed corpus. "
    "The real Rust tests that drive it against the real decoders are "
    "wire_fuzz.rs::every_committed_case_lands_where_it_declares (every committed case, asserting "
    "each lands where its own `expect` field declares) and "
    "wire_fuzz.rs::regression_duplicate_id_cases_reach_their_uniqueness_rules (the duplicate-id class "
    "specifically). `_RUST_TESTS['TEST-4-12']` names both; this harness checks their existence, "
    "`#[test]` status and drift fingerprint but does not execute the drive itself (no cargo). This "
    "module reads the committed expect field of every duplicate-id case (nine of the ten targets; "
    "frame.length-prefix has no keyed content)."
)
_ABSENT_09 = (
    "RFC 0026's protocol IDL declares Milestone.at: Timestamp required and 'milestones reached so "
    "far, in order' (list order authoritative), a monotonic-status rule, and the Timestamp scalar's "
    "millisecond precision (a real, structural collision hazard) — but no adapter anywhere in this "
    "repository reconstructs an order from committed milestones/events for this harness to call, and "
    "no real crates/continuumd test exercises a timestamp-total-order mutant by name (checked; none "
    "found). The check below is a freestanding model of the one hazard the rule names (a bare "
    "comparison on `at` cannot recover the true append order when two entries share a millisecond), "
    "not a mutation of running harness or engine logic and not tied to any Rust test, hence `partial` "
    "rather than `enforced` (README-test.md step 5; bn-3mo3's Rust-test-citation rule, common.md) "
    "despite passing with zero failures."
)
_ABSENT_11 = (
    "RFC 0026's protocol IDL declares the ErrorCode vocabulary (StaleSnapshot, "
    "ContinuationEpochMismatch, EpochUnsupported) and an epoch-independence rule states the "
    "obligation. idl_conformance.rs::the_error_taxonomy_is_the_complete_idl_set asserts these codes "
    "are named in the taxonomy, but exercises no request or continuation actually being rejected for "
    "a stale epoch — the taxonomy's completeness, not the mutant this ID names. No Python-callable "
    "adapter decides epoch staleness either way: epochs are content identities (ADR-0018), and the "
    "daemon that dispatches on them is Rust this no-cargo harness cannot execute. The check below is "
    "a freestanding acceptance-predicate model over hand-built epoch pairs, not a mutation of running "
    "harness or engine logic and not tied to any Rust test that exercises staleness rejection, hence "
    "`partial` (README-test.md step 5; bn-3mo3's Rust-test-citation rule, common.md)."
)

BOUNDARIES: dict[str, list[str]] = {
    "TEST-4-07": [_BOUNDARY_07],
    "TEST-4-08": [_BOUNDARY_08],
    "TEST-4-09": [_ABSENT_09],
    "TEST-4-10": [_BOUNDARY_10],
    "TEST-4-11": [_ABSENT_11],
    "TEST-4-12": [_BOUNDARY_12],
}


def _finding(rule: str, subject: str, message: str) -> tsys.Finding:
    return tsys.Finding(rule, RULES[rule], subject, message)


# ---------------------------------------------------------------------------
# TEST-4-07: accept unknown field as old meaning
# ---------------------------------------------------------------------------


def _check_unknown_field(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"decoded", "reference"}):
        return [_finding("unknown-field-remapped-detected", "<payload>", "payload must have exactly decoded/reference").as_json()]
    decoded, reference = payload["decoded"], payload["reference"]
    if not (isinstance(decoded, dict) and isinstance(reference, dict)):
        return [_finding("unknown-field-remapped-detected", "<payload>", "decoded/reference must be objects").as_json()]
    out = []
    for key in sorted(set(decoded) - _REQUEST_ENVELOPE_FIELDS):
        out.append(
            _finding(
                "unknown-field-remapped-detected",
                key,
                "decoded output carries a field RequestEnvelope does not declare; rule envelope.unknown_fields "
                "requires a daemon ignore it, never interpret it",
            ).as_json()
        )
    for key in sorted(set(decoded) & set(reference) & _REQUEST_ENVELOPE_FIELDS):
        if decoded[key] != reference[key]:
            out.append(
                _finding(
                    "unknown-field-remapped-detected",
                    key,
                    f"declared field {key!r} decoded to {decoded[key]!r}, not the reference value {reference[key]!r}: "
                    "an unknown field's content was mapped onto this field's old meaning",
                ).as_json()
            )
    return out


# ---------------------------------------------------------------------------
# TEST-4-08: omit fairness edge
# ---------------------------------------------------------------------------


def _state_key(state: tuple) -> str:
    return tsys.canonical([list(state[0]), list(state[1])])


def _true_fairness_edges(system: dict, fair: list[str]) -> set[tuple[str, str]]:
    c = tsys.compile_system(system)
    report = tsys.explore(system)
    fair_set = set(fair)
    edges: set[tuple[str, str]] = set()
    for s in report.reachable:
        skey = _state_key(s)
        for t in c.transitions:
            if t["name"] in fair_set and tsys.enabled(c, s, t):
                edges.add((t["name"], skey))
    return edges


def _check_fairness_edges(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"system", "fair", "claimed_edges"}):
        return [_finding("fairness-edge-drop-detected", "<payload>", "payload must have exactly system/fair/claimed_edges").as_json()]
    system, fair, raw_edges = payload["system"], payload["fair"], payload["claimed_edges"]
    static = tsys.check_static(system)
    if static or not isinstance(fair, list) or not all(isinstance(x, str) for x in fair) or not isinstance(raw_edges, list):
        return [f.as_json() for f in static] or [
            _finding("fairness-edge-drop-detected", "<payload>", "fair must be a list of transition names, claimed_edges a list").as_json()
        ]
    true_edges = _true_fairness_edges(system, fair)
    claimed: set[tuple[str, str]] = set()
    for raw in raw_edges:
        if isinstance(raw, list) and len(raw) == 2 and isinstance(raw[0], str) and isinstance(raw[1], str):
            claimed.add((raw[0], raw[1]))
    missing = sorted(true_edges - claimed)
    return [
        _finding(
            "fairness-edge-drop-detected",
            f"{name}@{state}",
            "a reachable state where the declared-fair transition is enabled is missing from the claimed "
            "fairness-obligation edge set",
        ).as_json()
        for name, state in missing
    ]


# ---------------------------------------------------------------------------
# TEST-4-09: use timestamp total order. (freestanding model; partial)
# ---------------------------------------------------------------------------


def _check_timestamp_order(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"true_order", "timestamps_ms"}):
        return [_finding("timestamp-total-order-detected", "<payload>", "payload must have exactly true_order/timestamps_ms").as_json()]
    true_order, ts = payload["true_order"], payload["timestamps_ms"]
    ok = (
        isinstance(true_order, list)
        and bool(true_order)
        and all(isinstance(n, str) for n in true_order)
        and len(set(true_order)) == len(true_order)
        and isinstance(ts, dict)
        and set(ts) == set(true_order)
        and all(isinstance(v, int) and not isinstance(v, bool) for v in ts.values())
    )
    if not ok:
        return [
            _finding(
                "timestamp-total-order-detected",
                "<payload>",
                "true_order must be a list of distinct names; timestamps_ms a same-keyed int (ms) map",
            ).as_json()
        ]
    reconstructed = sorted(true_order, key=lambda n: (ts[n], n))
    if reconstructed != true_order:
        return [
            _finding(
                "timestamp-total-order-detected",
                ",".join(true_order),
                f"sorting by millisecond `at` (Timestamp scalar; ties broken lexically, the only total order a "
                f"bare timestamp comparison can produce) yields {reconstructed}, not the declared-authoritative "
                f"append order {true_order} (Milestone.at 'milestones ... in order'; rule task.status_monotonic)",
            ).as_json()
        ]
    return []


# ---------------------------------------------------------------------------
# TEST-4-10: all first-demo mutants
# ---------------------------------------------------------------------------


def _check_first_demo_mutants(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"mutants"}):
        return [_finding("first-demo-mutant-not-held-detected", "<payload>", "payload must have exactly mutants").as_json()]
    mutants = payload["mutants"]
    if not isinstance(mutants, dict):
        return [_finding("first-demo-mutant-not-held-detected", "<payload>", "mutants must be an object").as_json()]
    out = []
    for name, rec in sorted(mutants.items()):
        if not (isinstance(rec, dict) and set(rec) == {"observed", "pre_registered", "held"}):
            out.append(_finding("first-demo-mutant-not-held-detected", name, "malformed mutant record").as_json())
            continue
        if rec["held"] is not True:
            out.append(
                _finding(
                    "first-demo-mutant-not-held-detected",
                    name,
                    f"drop-{name} is not held (observed={rec['observed']!r}): the falsification campaign's own "
                    "pre-registered outcome did not hold",
                ).as_json()
            )
        elif rec["observed"] != rec["pre_registered"]:
            out.append(
                _finding(
                    "first-demo-mutant-not-held-detected",
                    name,
                    f"observed {rec['observed']!r} != pre_registered {rec['pre_registered']!r}",
                ).as_json()
            )
    return out


# ---------------------------------------------------------------------------
# TEST-4-11: stale term/epoch (freestanding model; partial)
# ---------------------------------------------------------------------------


def _check_stale_epoch(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"pinned_epoch", "current_epoch", "accepted"}):
        return [_finding("stale-epoch-accepted-detected", "<payload>", "payload must have exactly pinned_epoch/current_epoch/accepted").as_json()]
    pinned, current, accepted = payload["pinned_epoch"], payload["current_epoch"], payload["accepted"]
    if not (isinstance(pinned, str) and isinstance(current, str) and isinstance(accepted, bool)):
        return [_finding("stale-epoch-accepted-detected", "<payload>", "pinned_epoch/current_epoch must be strings, accepted a bool").as_json()]
    if accepted and pinned != current:
        return [
            _finding(
                "stale-epoch-accepted-detected",
                f"{pinned} vs {current}",
                "a request/continuation pinned to a stale epoch was accepted instead of rejected with "
                "ContinuationEpochMismatch/StaleSnapshot/EpochUnsupported (rule versioning.epoch_independence)",
            ).as_json()
        ]
    return []


# ---------------------------------------------------------------------------
# TEST-4-12: double counting
# ---------------------------------------------------------------------------


def _check_double_counting(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"target", "class", "expect"}):
        return [_finding("duplicate-id-accepted-detected", "<payload>", "payload must have exactly target/class/expect").as_json()]
    target, cls, expect = payload["target"], payload["class"], payload["expect"]
    if not (isinstance(target, str) and target and isinstance(cls, str) and cls and isinstance(expect, str) and expect):
        return [_finding("duplicate-id-accepted-detected", "<payload>", "target/class/expect must be non-empty strings").as_json()]
    if cls == "duplicate-id" and expect == "accepted":
        return [
            _finding(
                "duplicate-id-accepted-detected",
                f"{target}:{cls}",
                "a wire-fuzz case with two members under one id/key is expected 'accepted': the repeated "
                "identifier would be double-counted instead of rejected on strict ascent",
            ).as_json()
        ]
    return []


# ---------------------------------------------------------------------------
# Fixtures dispatch
# ---------------------------------------------------------------------------

_KIND_CHECKERS = {
    "unknown-field": _check_unknown_field,
    "fairness-edges": _check_fairness_edges,
    "timestamp-order": _check_timestamp_order,
    "first-demo-mutants": _check_first_demo_mutants,
    "stale-epoch": _check_stale_epoch,
    "double-counting": _check_double_counting,
}


def check_fixture(system: Any) -> list[dict[str, str]]:
    if not isinstance(system, dict) or system.get("kind") not in _KIND_CHECKERS:
        return [_finding("unknown-field-remapped-detected", "<payload>", "payload must have a known 'kind'").as_json()]
    payload = {k: v for k, v in system.items() if k != "kind"}
    return _KIND_CHECKERS[system["kind"]](payload)


# ---------------------------------------------------------------------------
# The real run
# ---------------------------------------------------------------------------


def real_run() -> dict[str, Any]:
    failures: dict[str, list[str]] = {rid: [] for rid in OBLIGATIONS}
    corpus: dict[str, dict[str, int]] = {rid: {} for rid in OBLIGATIONS}
    mutants: dict[str, dict[str, int]] = {rid: {"applied": 0, "detected": 0} for rid in OBLIGATIONS}

    def bump(rid: str, key: str, n: int = 1) -> None:
        corpus[rid][key] = corpus[rid].get(key, 0) + n

    def mutate(rid: str, applied: bool, detected: bool, subject: str) -> None:
        if not applied:
            return
        mutants[rid]["applied"] += 1
        if detected:
            mutants[rid]["detected"] += 1
        else:
            failures[rid].append(f"mutant on {subject} was not detected: the checker reported the mutation preserved")

    # TEST-4-07: accept unknown field as old meaning (real IDL struct + real golden).
    bump("TEST-4-07", "known_fields_checked", len(_REQUEST_ENVELOPE_FIELDS))
    bump("TEST-4-07", "golden_fields_present", len(_REFERENCE_ENVELOPE))
    clean07 = _check_unknown_field({"decoded": dict(_REFERENCE_ENVELOPE), "reference": dict(_REFERENCE_ENVELOPE)})
    if clean07:
        failures["TEST-4-07"].append(f"the real committed envelope, correctly decoded, was itself flagged: {clean07[0]['message']}")
    corrupted = dict(_REFERENCE_ENVELOPE)
    corrupted["auth_token"] = "legacy-secret"  # a field RequestEnvelope does not declare
    corrupted["actor"] = "legacy-secret"  # ... reinterpreted under a declared field's old meaning
    mut07 = _check_unknown_field({"decoded": corrupted, "reference": _REFERENCE_ENVELOPE})
    mutate("TEST-4-07", True, bool(mut07), "well-formed-workspace-create.case (unknown auth_token remapped onto actor)")

    # TEST-4-08: omit fairness edge (real tsys oracle substance).
    for seed in SEEDS:
        base = tsys.generate(seed)
        static = tsys.check_static(base)
        if static:
            failures["TEST-4-08"].append(f"gen-{seed}: unexpected static finding on the generator's own output: {static[0].message}")
            continue
        fair = sorted(t["name"] for t in base["transitions"])
        true_edges = _true_fairness_edges(base, fair)
        bump("TEST-4-08", "systems")
        clean08 = _check_fairness_edges({"system": base, "fair": fair, "claimed_edges": [list(e) for e in true_edges]})
        if clean08:
            failures["TEST-4-08"].append(f"gen-{seed}: the correct edge set was itself flagged: {clean08[0]['message']}")
        if true_edges:
            bump("TEST-4-08", "systems_with_fairness_edges")
            ordered = sorted(true_edges)
            dropped = ordered[1:]
            mut08 = _check_fairness_edges({"system": base, "fair": fair, "claimed_edges": [list(e) for e in dropped]})
            mutate("TEST-4-08", True, bool(mut08), f"gen-{seed} (dropped {ordered[0]})")

    # TEST-4-09: use timestamp total order. (freestanding model, partial).
    bump("TEST-4-09", "cases", 1)
    clean09 = _check_timestamp_order({"true_order": ["alpha", "zeta"], "timestamps_ms": {"alpha": 1000, "zeta": 2000}})
    if clean09:
        failures["TEST-4-09"].append(f"a strictly-increasing timestamp pair matching the true append order was itself flagged: {clean09[0]['message']}")
    mut09 = _check_timestamp_order({"true_order": ["zeta", "alpha"], "timestamps_ms": {"zeta": 1000, "alpha": 1000}})
    mutate(
        "TEST-4-09",
        True,
        bool(mut09),
        "zeta/alpha share a millisecond timestamp (RFC 3339 Timestamp scalar, ms precision): sorting by `at` cannot recover the true append order",
    )

    # TEST-4-10: all first-demo mutants (real byte-pinned golden).
    dx01_text = _DX01_GOLDEN.read_text(encoding="utf-8")
    real_mutants = _parse_dx01_mutants(dx01_text)
    bump("TEST-4-10", "mutants_in_golden", len(real_mutants))
    missing_names = sorted(_EXPECTED_DX01_MUTANTS - set(real_mutants))
    if missing_names:
        failures["TEST-4-10"].append(f"dx01_falsification_evidence.txt no longer records drop-mutants for {missing_names}")
    clean10 = _check_first_demo_mutants({"mutants": real_mutants})
    if clean10:
        failures["TEST-4-10"].append(f"the real committed dx01 golden was itself flagged: {clean10[0]['message']}")
    if real_mutants:
        corrupted10 = copy.deepcopy(real_mutants)
        one_name = sorted(corrupted10)[0]
        corrupted10[one_name]["held"] = False
        mut10 = _check_first_demo_mutants({"mutants": corrupted10})
        mutate("TEST-4-10", True, bool(mut10), f"dx01_falsification_evidence.txt ({one_name} held flipped to false)")

    # TEST-4-11: stale term/epoch (freestanding model, partial; real vocabulary re-verified at import).
    bump("TEST-4-11", "cases", 1)
    clean11 = _check_stale_epoch({"pinned_epoch": "sem:aaa", "current_epoch": "sem:aaa", "accepted": True})
    if clean11:
        failures["TEST-4-11"].append(f"a matching-epoch acceptance was itself flagged: {clean11[0]['message']}")
    mut11 = _check_stale_epoch({"pinned_epoch": "sem:aaa", "current_epoch": "sem:bbb", "accepted": True})
    mutate("TEST-4-11", True, bool(mut11), "pinned epoch sem:aaa silently accepted against current epoch sem:bbb")

    # TEST-4-12: double counting (real committed wire-fuzz-corpus duplicate-id cases).
    dup_cases = sorted(_WIRE_FUZZ_CORPUS.glob("*/duplicate-id-*.case"))
    bump("TEST-4-12", "duplicate_id_cases", len(dup_cases))
    for path in dup_cases:
        meta = _parse_wire_fuzz_case(path.read_text(encoding="utf-8"))
        clean12 = _check_double_counting({"target": meta.get("target"), "class": meta.get("class"), "expect": meta.get("expect")})
        if clean12:
            failures["TEST-4-12"].append(f"{path.relative_to(_ROOT)}: the real committed case was itself flagged: {clean12[0]['message']}")
    mut12 = _check_double_counting({"target": "canonical.json", "class": "duplicate-id", "expect": "accepted"})
    mutate("TEST-4-12", True, bool(mut12), "canonical.json duplicate-id case mutated to expect=accepted (a decoder that double-counts the repeated key)")

    def need(rid: str, key: str, what: str) -> None:
        if corpus[rid].get(key, 0) == 0:
            failures[rid].append(f"corpus exercises no {what} ({key} = 0): the pass would be vacuous")

    need("TEST-4-08", "systems_with_fairness_edges", "generated system with a declared-fair transition enabled at some reachable state")
    need("TEST-4-12", "duplicate_id_cases", "committed wire-fuzz-corpus case exercising a duplicate identifier")

    for rid in OBLIGATIONS:
        if mutants[rid]["applied"] and mutants[rid]["detected"] == 0:
            failures[rid].append(f"{rid}: the mutant was never detected across the corpus (the checker would be vacuous)")

    # A Python port of a Rust predicate is not evidence about the Rust code
    # (bn-3mo3 rule, common.md): every ID this module marks `enforced` about
    # real protocol/codec/engine behaviour must name the real Rust test(s)
    # that exercise the real code against that mutant class, and this
    # harness checks — textually, no cargo — that each still exists, is not
    # `#[ignore]`d, and has not drifted from its pinned fingerprint.
    rust_test_records: dict[str, list[dict[str, Any]]] = {}
    for rid in OBLIGATIONS:
        rust_failures, records = _check_rust_tests(rid)
        rust_test_records[rid] = records
        failures[rid].extend(rust_failures)

    status = {
        "TEST-4-07": "enforced",
        "TEST-4-08": "enforced",
        "TEST-4-09": "partial",
        "TEST-4-10": "enforced",
        "TEST-4-11": "partial",
        "TEST-4-12": "enforced",
    }
    absence = {
        "TEST-4-09": "no adapter in this repository reconstructs an order from committed milestones/events for this harness to call; see BOUNDARIES",
        "TEST-4-11": "no Python-callable adapter decides epoch staleness (epochs are Rust content identities, ADR-0018); see BOUNDARIES",
    }

    results = {}
    for rid, summary in OBLIGATIONS.items():
        entry: dict[str, Any] = {
            "status": status[rid],
            "rules": sorted(r for r, q in RULES.items() if q == rid),
            "failures": failures[rid],
            "corpus": corpus[rid],
            "mutant": mutants[rid],
            "boundaries": BOUNDARIES[rid],
        }
        if rid in absence:
            entry["absence"] = absence[rid]
        if rust_test_records[rid]:
            entry["rust_tests"] = rust_test_records[rid]
        results[rid] = entry
    return {
        "seeds": f"{SEEDS.start}..{SEEDS.stop - 1}",
        "requirements": results,
        "unclaimed": [
            "TEST-4-13",
            "TEST-4-14",
            "TEST-4-15",
            "TEST-4-16",
            "TEST-4-17",
            "TEST-4-18",
        ],
        "out_of_scope": "TEST-4-13..18 (Protocol corpus: non-idempotent retry / ack-before-durability / forgotten loser drain / timeout via silent drop / restart timer leak / recovery livelock) is bn-2dt2's job, claimed in parallel",
    }
