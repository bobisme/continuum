"""docs/19 §5 "Differential oracles" — TEST-5-07 (bn-3iwa): "external
temporal/probabilistic tools for extension lanes", the seventh of §5's seven
bullets, left unclaimed by `s5_differential_oracles.py` (bn-7usz; see its own
module doc and the `"unclaimed": ["TEST-5-07"]` key of its evidence file).
This module claims TEST-5-07 alone, without editing a line of that module,
`tsys.py`, or the `Justfile`.

# What "extension lane" names, and what tool

docs/19's own list orders §5's bullets by increasing distance from what
Continuum has landed: reference-vs-optimized evaluator (TEST-5-01), exhaustive
enumerator vs DPOR (TEST-5-02), TLC/Quint/Apalache (TEST-5-03), asupersync Lab
reports (TEST-5-04), Kani/Verus (TEST-5-05), SAT/SMT solvers and proof
checkers (TEST-5-06) — then, last, "external temporal/probabilistic tools for
extension lanes". The extension lanes are ADR-0016's ("Add timed and
probabilistic semantics as typed extensions", Status: "Accepted in principle;
deferred") and research/06's ("Timed, Probabilistic, and Hyperproperty
Extensions", Claim class: "future extension"). Both name the same four tools
this bullet means: ADR-0016's Validation section reads "Compare against
UPPAAL/IMITATOR/Storm/PRISM"; research/06's Boundaries section reads "The G6
research gate requires comparison with UPPAAL, IMITATOR, Storm, and PRISM."
None of the four is TLC/Quint/Apalache (TEST-5-03's untimed/unprobabilistic
model checkers) or a SAT/SMT solver (TEST-5-06): UPPAAL and IMITATOR are
timed-automata model checkers, Storm and PRISM are probabilistic model
checkers. TEST-5-07 is its own bullet for exactly that reason.

# Checked directly in this environment before writing a line of the checks below

- No adapter crate for any of the four tools exists: `crates/` has 37
  members, none named or shaped as an adapter for UPPAAL, IMITATOR, Storm, or
  PRISM, and a repository-wide, case-insensitive search of `crates/`,
  `tools/`, and `notes/plan/schemas/` for the four tool names finds zero
  hits. `real_run()` re-runs that same scan for real
  (`_scan_for_undeclared_tools`), rather than asserting the absence in prose.
- `continuum-kernel-temporal` (128 lines, PR 9) is real, landed code, but it
  is not this bullet's substance. Its own doc comment: "The liveness engine
  searches; this crate only checks what the search claims" — it independently
  re-checks ranking and fair-SCC-exclusion certificates a search procedure
  *inside this repository* already computed. It never invokes UPPAAL,
  IMITATOR, Storm, or PRISM, and is not a differential comparison against an
  externally implemented system, which is docs/19 §5's own text for every
  bullet in this section.
- ADR-0016 defers the whole lane by its own decision, not merely by omission:
  "No implementation before G4 unless a concrete project demands it."
- No registry, schema, or release-asset scanner exists anywhere in this
  workspace for a "declared extension-lane oracle" record of any kind.
  ADR-0029's own Validation section names one ("a release-artifact check
  enumerates every shipped file and fails on JVM, tlaplus, Apalache, TLAPS,
  or solver binaries") but it is unbuilt, and its normative "foreign oracle
  tooling" definition is scoped to TLC/SANY/the tlaplus toolset, Apalache,
  TLAPS, and SAT/SMT/CHC solvers — UPPAAL/IMITATOR/Storm/PRISM sit outside
  even that list's letter, though plainly inside its purpose: ADR-0029 rule 6
  applies "one adapter crate or process boundary (ADR-0019)" to "such
  invocation" (foreign oracles generally, §5's permission for "Foreign
  oracles MAY execute ... in user workflows where the user has independently
  installed the tool"), and ADR-0019's own decision is general: "Each foreign
  system has one adapter crate/process boundary."

There is therefore no real differential lane to bind to, named or otherwise
(this bone's own task: "It is almost certainly not landed yet"). Per
README-test.md's freestanding-demonstration discipline — the same bar
`s4_mutation_testing.py` and the sibling `s5_differential_oracles.py` both set
for their own absent-substance IDs — this module enforces what IS real and
checkable today: that if and when an extension-lane oracle IS declared, it
can only be declared in a form the ADR-0029/ADR-0019 separate-process
boundary already requires. Two things are real here, and both run for real,
against the actual repository and a freestanding contract, not a mock:

1. A repository-wide drift guard (`extension-lane-tool-undeclared-mention`):
   today's `crates/`, `tools/`, and `notes/plan/schemas/` tree must not
   already contain an undeclared mention of "storm", "prism", "uppaal", or
   "imitator" — so a future lane cannot silently land outside this policy
   between one run of this checker and the next.
2. A freestanding registration-record contract checker
   (`extension-lane-oracle-*`) over the shape ADR-0029 rules 1/3/4/6 and
   ADR-0019 require of any declared extension-lane oracle: it must not ship
   in release artifacts, it must not be invoked silently, its absence must
   produce a typed `Unsupported`/inconclusive result (never a silent
   downgrade, INV-008), and it must cross exactly one adapter crate running
   as a separate process — never one of the trusted-checking-base, engine, or
   search crates INV-004's no-self-certification boundary protects. No real
   registry exists to check (none is claimed to exist), so this is a
   hand-authored, self-consistent record, mutation-tested against itself —
   not a comparison against a real UPPAAL/IMITATOR/Storm/PRISM run.

`partial`, not `enforced`: see the `absence` entry in `real_run()`'s output.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

import tsys

SECTION = 5
TITLE = "Differential oracles"

OBLIGATIONS = {
    "TEST-5-07": "external temporal/probabilistic tools for extension lanes.",
}

RULES: dict[str, str] = {
    "extension-lane-oracle-ships-in-release": "TEST-5-07",
    "extension-lane-oracle-silent-invocation": "TEST-5-07",
    "extension-lane-oracle-absence-not-typed": "TEST-5-07",
    "extension-lane-oracle-no-process-boundary": "TEST-5-07",
    "extension-lane-tool-undeclared-mention": "TEST-5-07",
}

ROOT = Path(__file__).resolve().parents[3]
SCAN_DIRS = (ROOT / "crates", ROOT / "tools", ROOT / "notes/plan/schemas")
SCAN_SUFFIXES = (".rs", ".toml", ".json")
EXTENSION_LANE_TOOLS = ("storm", "prism", "uppaal", "imitator")

# This module's own fixtures and evidence declare the four tool names on purpose,
# to demonstrate `extension-lane-tool-undeclared-mention` itself (self-test, and
# `build_evidence`'s embedded fixture payloads). Those are declared mentions, in the
# one file whose job is exactly this rule — excluded from the drift guard's scan of
# the rest of the tree, the same way a lint tool excludes its own test fixtures.
_SELF_FIXTURES_DIR = ROOT / "tools/test-policy/fixtures/s5_extension_lane_oracles"
_SELF_EVIDENCE_FILE = ROOT / "tools/test-policy/evidence/s5_extension_lane_oracles.json"


def _is_self_reference(path: Path) -> bool:
    return path == _SELF_EVIDENCE_FILE or _SELF_FIXTURES_DIR in path.parents

# Crates that must never themselves be an adapter's process boundary (INV-004: no
# self-certification; ADR-0019: "the kernel does not depend on them"). Not
# exhaustive of the workspace — enough to demonstrate the contract catches a
# forbidden crate, not just a missing one.
FORBIDDEN_ADAPTER_HOMES = (
    "continuum-kernel-core",
    "continuum-kernel-sat",
    "continuum-kernel-smt",
    "continuum-kernel-temporal",
    "continuum-certificate",
    "continuum-forge",
    "continuum-engine-reference",
    "continuum-engine-explicit",
    "continuum-engine-dpor",
    "continuum-engine-liveness",
    "continuum-engine-symbolic",
    "continuum-asupersync",
)

_REG_FIELDS = {
    "name",
    "adapter_crate",
    "boundary",
    "ships_in_release",
    "invocation",
    "recorded_evidence",
    "absence_behavior",
}


def _finding(rule: str, subject: str, message: str) -> tsys.Finding:
    return tsys.Finding(rule, RULES[rule], subject, message)


def _malformed(rule: str, what: str) -> list[dict]:
    return [_finding(rule, "<payload>", f"payload must have exactly {what}").as_json()]


# ---------------------------------------------------------------------------
# The registration-record contract: ADR-0029 rules 1/3/4/6 and ADR-0019, over a
# hand-authored "declared extension-lane oracle" shape. No real registry exists
# (module doc); this is what one would have to satisfy if it did.
# ---------------------------------------------------------------------------


def _check_registration(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == _REG_FIELDS):
        return _malformed("extension-lane-oracle-ships-in-release", "/".join(sorted(_REG_FIELDS)))
    name = payload.get("name", "<unnamed>")
    findings: list[dict] = []

    # ADR-0029 rule 1: release artifacts MUST NOT contain, bundle, vendor, or embed
    # foreign oracle tooling.
    if payload.get("ships_in_release") is not False:
        findings.append(
            _finding(
                "extension-lane-oracle-ships-in-release",
                name,
                f"{name!r} declares ships_in_release={payload.get('ships_in_release')!r}: ADR-0029 rule 1 forbids "
                "release binaries, installers, packaged distributions, container images, and the docs/12 §5 "
                "release assets from containing, bundling, vendoring, or embedding foreign oracle tooling",
            ).as_json()
        )

    # ADR-0029 rule 3: no silent invocation; every invocation is explicitly requested
    # and recorded in the evidence graph with the tool's identity and version.
    ev = payload.get("recorded_evidence")
    has_identity = (
        isinstance(ev, dict)
        and isinstance(ev.get("tool_identity"), str)
        and bool(ev.get("tool_identity"))
        and isinstance(ev.get("tool_version"), str)
        and bool(ev.get("tool_version"))
    )
    if payload.get("invocation") != "explicit" or not has_identity:
        findings.append(
            _finding(
                "extension-lane-oracle-silent-invocation",
                name,
                f"{name!r} declares invocation={payload.get('invocation')!r} recorded_evidence={ev!r}: ADR-0029 "
                "rule 3 requires release binaries to never silently invoke foreign tooling — any invocation MUST "
                "be explicitly requested and MUST be recorded in the evidence graph with the tool's identity and "
                "version",
            ).as_json()
        )

    # ADR-0029 rule 4 / INV-008: absence of the oracle is a typed Unsupported or
    # inconclusive result, never a silent downgrade that still reads as success.
    if payload.get("absence_behavior") != "typed-unsupported":
        findings.append(
            _finding(
                "extension-lane-oracle-absence-not-typed",
                name,
                f"{name!r} declares absence_behavior={payload.get('absence_behavior')!r}: ADR-0029 rule 4 requires "
                "a typed Unsupported or inconclusive value in the assurance envelope when the oracle is absent — "
                "'silent downgrade to a weaker claim that still reads as success is prohibited' — and INV-008 "
                "(typed inconclusiveness) forbids collapsing this into a bare boolean",
            ).as_json()
        )

    # ADR-0029 rule 6 / ADR-0019: every foreign-oracle invocation crosses exactly one
    # adapter crate running as a separate process, and that crate is never one of the
    # trusted-checking-base, engine, or search crates INV-004's no-self-certification
    # boundary protects ("the kernel does not depend on them", ADR-0019).
    adapter = payload.get("adapter_crate")
    boundary_ok = (
        payload.get("boundary") == "separate-process"
        and isinstance(adapter, str)
        and adapter.strip() != ""
        and adapter not in FORBIDDEN_ADAPTER_HOMES
    )
    if not boundary_ok:
        findings.append(
            _finding(
                "extension-lane-oracle-no-process-boundary",
                name,
                f"{name!r} declares adapter_crate={adapter!r} boundary={payload.get('boundary')!r}: ADR-0029 rule 6 "
                "and ADR-0019 require every foreign-oracle invocation to cross exactly one adapter crate running "
                "as a separate process, and that crate must not be one of the trusted-checking-base/engine/search "
                f"crates {list(FORBIDDEN_ADAPTER_HOMES)} whose isolation from adapters INV-004 protects",
            ).as_json()
        )

    return findings


# ---------------------------------------------------------------------------
# The repository-wide drift guard: a real check against real files, not a fixture
# stand-in. `_check_scan` runs the identical logic against a fixture's synthetic
# {path: text} map so the rule can be demonstrated without touching the tree.
# ---------------------------------------------------------------------------


def _scan_text_for_tools(path: str, text: str) -> list[dict]:
    findings = []
    for tool in EXTENSION_LANE_TOOLS:
        if re.search(rf"(?i)\b{tool}\b", text):
            findings.append(
                _finding(
                    "extension-lane-tool-undeclared-mention",
                    path,
                    f"{path} mentions {tool!r} (one of {EXTENSION_LANE_TOOLS}), but no oracle-registration record "
                    "or adapter crate exists for it in this workspace: an extension-lane oracle may not land "
                    "without first passing the ADR-0029/ADR-0019 boundary contract this module checks",
                ).as_json()
            )
    return findings


def _scan_for_undeclared_tools() -> tuple[list[dict], int]:
    findings: list[dict] = []
    scanned = 0
    for base in SCAN_DIRS:
        if not base.exists():
            continue
        for path in sorted(base.rglob("*")):
            if not path.is_file() or path.suffix not in SCAN_SUFFIXES:
                continue
            if _is_self_reference(path):
                continue
            try:
                text = path.read_text(encoding="utf-8")
            except (OSError, UnicodeDecodeError):
                continue
            scanned += 1
            findings.extend(_scan_text_for_tools(str(path.relative_to(ROOT)), text))
    return findings, scanned


def _check_scan(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"files"}) or not isinstance(payload["files"], dict):
        return _malformed("extension-lane-tool-undeclared-mention", "files (a path->text map)")
    findings = []
    for path, text in payload["files"].items():
        findings.extend(_scan_text_for_tools(path, text))
    return findings


# ---------------------------------------------------------------------------
# Fixtures dispatch
# ---------------------------------------------------------------------------

_KIND_CHECKERS = {
    "registration": _check_registration,
    "scan": _check_scan,
}


def check_fixture(system: Any) -> list[dict]:
    if not isinstance(system, dict) or system.get("kind") not in _KIND_CHECKERS:
        return [_finding("extension-lane-oracle-ships-in-release", "<payload>", "payload must have a known 'kind'").as_json()]
    payload = {k: v for k, v in system.items() if k != "kind"}
    return _KIND_CHECKERS[system["kind"]](payload)


# ---------------------------------------------------------------------------
# The real run
# ---------------------------------------------------------------------------


def _self_consistent_registration() -> dict:
    """A hand-authored record satisfying every ADR-0029/ADR-0019 rule the checker
    above enforces. Not a real registry entry — none exists (module doc) — this is
    what a compliant one would have to look like."""
    return {
        "name": "storm-extension-lane (freestanding demonstration; no real registry exists)",
        "adapter_crate": "continuum-adapter-storm",
        "boundary": "separate-process",
        "ships_in_release": False,
        "invocation": "explicit",
        "recorded_evidence": {"tool_identity": "storm", "tool_version": "1.8.1"},
        "absence_behavior": "typed-unsupported",
    }


_MUTANTS: dict[str, Any] = {
    "extension-lane-oracle-ships-in-release": lambda r: dict(r, ships_in_release=True),
    "extension-lane-oracle-silent-invocation": lambda r: dict(r, invocation="silent"),
    "extension-lane-oracle-absence-not-typed": lambda r: dict(r, absence_behavior="silent-success"),
    "extension-lane-oracle-no-process-boundary": lambda r: dict(r, boundary="in-process", adapter_crate="continuum-kernel-temporal"),
}


def _run_5_07() -> tuple[list[str], dict]:
    failures: list[str] = []

    # (1) the repository-wide drift guard, run for real against the actual tree.
    scan_hits, scanned = _scan_for_undeclared_tools()
    for h in scan_hits:
        failures.append(f"undeclared extension-lane tool mention: {h['message']}")
    if scanned == 0:
        failures.append("the undeclared-tool scan covered zero files: the drift guard would be vacuous")

    # (2) the freestanding registration-record contract: self-consistency, then one
    # mutant per rule, each independently detected.
    clean = _self_consistent_registration()
    clean_hits = _check_registration(clean)
    if clean_hits:
        failures.append(f"a self-consistent ADR-0029-compliant registration record was itself flagged: {clean_hits[0]['message']}")

    applied = 0
    detected = 0
    for rule, mutate in _MUTANTS.items():
        applied += 1
        hits = _check_registration(mutate(clean))
        if any(h["rule"] == rule for h in hits):
            detected += 1
        else:
            failures.append(f"mutating the registration record to violate {rule!r} was not detected")
    if applied == 0 or detected == 0:
        failures.append("no mutant of the registration-record contract was detected: the check would be vacuous")

    return failures, {
        "scan_dirs": [str(d.relative_to(ROOT)) for d in SCAN_DIRS],
        "scanned_files": scanned,
        "undeclared_tool_hits": len(scan_hits),
        "registration_contract_mutants_applied": applied,
        "registration_contract_mutants_detected": detected,
    }


_ABSENCE = {
    "TEST-5-07": (
        "No adapter crate for UPPAAL, IMITATOR, Storm, or PRISM (ADR-0016's own named comparison targets, "
        "repeated verbatim by research/06's G6 gate) exists anywhere in this workspace: crates/ has 37 members "
        "and none is named or shaped as one, and a repository-wide, case-insensitive scan of crates/, tools/, and "
        "notes/plan/schemas/ for the four tool names finds zero hits (real_run() re-runs that scan for real, not "
        "a prose assertion). continuum-kernel-temporal (128 lines, PR 9, real and landed) checks "
        "ranking/fair-SCC-exclusion certificates a search procedure already inside this repository computed; it "
        "never invokes an external temporal or probabilistic tool and is not the differential comparison against "
        "an independently implemented system docs/19 §5 asks for. ADR-0016 itself defers the whole lane: 'No "
        "implementation before G4 unless a concrete project demands it.' No registry, schema, or release-asset "
        "scanner exists for a declared oracle's ADR-0029 boundary either — ADR-0029's own Validation section "
        "names one and it is unbuilt. The enforced substance is therefore two real things: (1) a repository-wide "
        "drift guard against an undeclared future mention of any of the four tool names, so a lane cannot "
        "silently land outside this policy between one run of this checker and the next, and (2) a freestanding "
        "contract checker — not a real registry, since none exists — over the shape any declared extension-lane "
        "oracle registration would have to satisfy under ADR-0029 rules 1/3/4/6 and ADR-0019's adapter-crate/"
        "process-boundary requirement, self-consistency- and mutation-tested against itself. Neither compares "
        "against a real UPPAAL/IMITATOR/Storm/PRISM run. Follow-up (lead-owned, new bone): land one adapter crate "
        "behind this boundary and bind this module's registration contract to its real manifest the way "
        "s4_protocol_corpus.py's RUST_BINDING binds a Python port to crates/continuumd's real tests."
    ),
}


def real_run() -> dict[str, Any]:
    failures, evidence = _run_5_07()
    return {
        "requirements": {
            "TEST-5-07": {
                "status": "partial",
                "rules": sorted(RULES),
                "failures": failures,
                "absence": _ABSENCE["TEST-5-07"],
                "evidence": evidence,
            }
        },
        "unclaimed": [],
        "note": "TEST-5-07 is 'partial': no extension-lane oracle (UPPAAL/IMITATOR/Storm/PRISM) is landed or "
        "reachable in this workspace, and ADR-0016 defers the lane to G4; see the 'absence' entry.",
    }
