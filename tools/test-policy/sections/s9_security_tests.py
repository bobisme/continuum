"""docs/19 §9 "Security validation" — TEST-9-01 … TEST-9-06 (bn-35hb).

docs/19 §9's first six bullets: dependency and `unsafe` audits; malformed untrusted
trace/certificate inputs; denial-of-service limits; path traversal in crashpacks;
solver sandboxing; secret-redaction tests. (The seventh bullet, "signature/provenance
verification", is `TEST-9-07` and is not claimed here — see `unclaimed` in the
evidence file.)

This is a `--no-cargo`, Python-stdlib-only harness (README-test.md). Two different
kinds of "real" evidence appear below, and the module says which is which for every
ID:

1. **TEST-9-01 is not a port.** The coordinator's own instruction for this bone: "A
   policy harness that runs the real audit checker, and reads the real lint setting
   from Cargo.toml, counts as real evidence here, because the obligation is about the
   repo's own audit gate." So this module *imports and calls* the real, committed
   `tools/governance/check_code_policy.py::rule_unsafe_forbidden` and
   `tools/governance/check_dependency_audit.py::rule_dependency_audit` — the exact
   functions `just check`'s GOV-1-02 and dependency-audit-posture gates run — against
   the real repository tree, and against the same functions' own `Tree.with_changes`
   fixture-overlay mechanism for violating fixtures. No Rust `#[test]` binding is
   named for this ID; none is needed, because the check *is* the production gate, not
   a model of it.
2. **TEST-9-02 … TEST-9-06 are Python models of real, committed Rust code** this
   harness cannot execute. Each names the real source it is drift-tied to (a runtime
   extraction or literal-phrase check, never a hand-copied constant) and the specific,
   real, existing, non-`#[ignore]`d Rust `#[test]` functions that exercise that real
   code — checked for presence by `_rust_test_present` (textual, no cargo), the same
   discipline `s6_fuzzing_targets.py` established. Where no such landed Rust
   implementation exists for a stated part of the obligation, the ID is `partial` and
   the absence is named in `ABSENCE` — never annotated `(delivered: …)`.

| ID | Real surface enforced | Rust/gate evidence named | Status |
|---|---|---|---|
| TEST-9-01 | `tools/governance/check_code_policy.py::rule_unsafe_forbidden` (Cargo.toml `[workspace.lints.rust] unsafe_code`, every crate manifest, every `crates/**/*.rs` source) + `check_dependency_audit.py::rule_dependency_audit` (`tools/governance/dependency-audits.toml` vs. the real `Cargo.lock`) | the production gates themselves, called directly (no port) | enforced |
| TEST-9-02 | the four `continuum-kernel-*` crates' `Reader::token`/`Reader::take` malformed-input grammar (`MAX_TOKEN_BYTES`, empty/oversized/non-printable/truncated rejection), extracted at run time from each kernel's real `wire.rs` | `reader_reports_truncation_instead_of_panicking` + `tokens_reject_empty_oversized_and_non_printable`, once per kernel (8 real tests) — the *certificate* half only | partial |
| TEST-9-03 | `continuum-context`'s real resource ceilings: `PackError::OverNodeCeiling` (`pack.rs`) and `BudgetError::Exhausted` (`budget.rs`) — a request over ceiling is always a typed refusal, never silent growth | `pack.rs::a_root_records_its_node_ceiling_and_refuses_to_break_it`, `budget.rs::a_ceiling_below_the_minimal_child_publishes_nothing`, `pr11_impl06_byte_and_token_budgets.rs::an_over_budget_expansion_publishes_a_smaller_child_recording_the_shortfall` | enforced |
| TEST-9-04 | `continuum-workspace`'s `validate_identity` (the traversal-free `[A-Za-z0-9_-]` character class every `ArtifactHandle`, including `ArtifactClass::Crashpack`, is constructed through) and `ArtifactPath::for_handle`'s three-segment, separator-free derived path | `artifact_path.rs::traversal_and_separator_attempts_are_rejected`, `::no_derived_path_has_a_relative_or_absolute_segment`, `::resolve_stays_under_the_root` | enforced |
| TEST-9-05 | plan.md §18.3 "Sandboxing" (prose only) | none: no sandbox mechanism exists anywhere in `crates/` | partial |
| TEST-9-06 | `notes/plan/schemas/redacted.schema.json` + the real `RedactionPolicy`/`RedactionReason` (`compile.rs`) and `OmissionReason::admits_irretrievability` (`omission.rs`): a withheld candidate is recorded `Redaction` and is never `expandable` | `omission.rs::only_two_reasons_can_spell_irretrievability`, `stage.rs::redaction_cannot_be_recorded_after_root_selection`, `pr11_compiler_stages_5_7.rs::redaction_still_wins_over_every_stage_of_this_group` | enforced |

None of this reads or writes `crates/*/Cargo.toml`, `tsys.py`, the `Justfile`, or
another section's fixtures; it is a new file per README-test.md's extension rule.
"""

from __future__ import annotations

import importlib.util
import json
import re
import sys
from pathlib import Path
from types import ModuleType
from typing import Any

import tsys

SECTION = 9
TITLE = "Security validation"

# tools/test-policy/sections/s9_security_tests.py -> repo root
ROOT = Path(__file__).resolve().parents[3]
GOVERNANCE_DIR = ROOT / "tools/governance"
CARGO_TOML_PATH = ROOT / "Cargo.toml"
CARGO_LOCK_PATH = ROOT / "Cargo.lock"
DEPENDENCY_AUDITS_REL = "tools/governance/dependency-audits.toml"
DEPENDENCY_AUDITS_PATH = ROOT / DEPENDENCY_AUDITS_REL
UNSAFE_SOURCE_REL = "crates/continuum-value/src/lib.rs"

KERNEL_WIRE_PATHS = {
    "Core": ROOT / "crates/continuum-kernel-core/src/wire.rs",
    "Sat": ROOT / "crates/continuum-kernel-sat/src/wire.rs",
    "Smt": ROOT / "crates/continuum-kernel-smt/src/wire.rs",
    "Temporal": ROOT / "crates/continuum-kernel-temporal/src/wire.rs",
}

PACK_RS_PATH = ROOT / "crates/continuum-context/src/pack.rs"
BUDGET_RS_PATH = ROOT / "crates/continuum-context/src/budget.rs"
ARTIFACT_PATH_RS_PATH = ROOT / "crates/continuum-workspace/src/artifact_path.rs"
OMISSION_RS_PATH = ROOT / "crates/continuum-context/src/omission.rs"
COMPILE_RS_PATH = ROOT / "crates/continuum-context/src/compile.rs"
PLAN_MD_PATH = ROOT / "notes/plan/plan.md"
REDACTED_SCHEMA_PATH = ROOT / "notes/plan/schemas/redacted.schema.json"
REDACTED_EXAMPLE_PATH = ROOT / "notes/plan/schemas/examples/redacted.example.json"

OBLIGATIONS = {
    "TEST-9-01": "dependency and unsafe audits",
    "TEST-9-02": "malformed untrusted trace/certificate inputs",
    "TEST-9-03": "denial-of-service limits",
    "TEST-9-04": "path traversal in crashpacks",
    "TEST-9-05": "solver sandboxing",
    "TEST-9-06": "secret-redaction tests",
}

RULES: dict[str, str] = {
    "unsafe-lint-not-forbid-detected": "TEST-9-01",
    "unsafe-in-source-detected": "TEST-9-01",
    "dependency-not-audited-detected": "TEST-9-01",
    "dependency-checksum-mismatch-detected": "TEST-9-01",
    "wire-token-empty-rejected-detected": "TEST-9-02",
    "wire-token-oversized-rejected-detected": "TEST-9-02",
    "wire-token-non-printable-rejected-detected": "TEST-9-02",
    "wire-reader-truncated-rejected-detected": "TEST-9-02",
    "node-ceiling-exceeded-not-refused-detected": "TEST-9-03",
    "byte-ceiling-exhaustion-not-refused-detected": "TEST-9-03",
    "identity-character-class-violation-not-rejected-detected": "TEST-9-04",
    "derived-artifact-path-escapes-root-detected": "TEST-9-04",
    "sandbox-isolation-property-violated-detected": "TEST-9-05",
    "redacted-example-schema-violation-detected": "TEST-9-06",
    "redacted-item-wrongly-marked-expandable-detected": "TEST-9-06",
}

STATUS: dict[str, str] = {
    "TEST-9-01": "enforced",
    "TEST-9-02": "partial",
    "TEST-9-03": "enforced",
    "TEST-9-04": "enforced",
    "TEST-9-05": "partial",
    "TEST-9-06": "enforced",
}

_BOUNDARY_01 = (
    "This ID's checks are not a Python port: `_check_governance_gate` dynamically loads "
    "the real, committed `tools/governance/check_code_policy.py` and "
    "`check_dependency_audit.py` modules (importlib, from their real file paths) and calls "
    "their real `rule_unsafe_forbidden`/`rule_dependency_audit` functions — the exact code "
    "`just check` runs for GOV-1-02 and dependency-audit-posture — against the real "
    "repository tree for the clean case, and against the same functions' own "
    "`Tree.with_changes` fixture-overlay mechanism (a real Cargo.toml/dependency-audits.toml "
    "text, textually mutated by this module, never hand-authored from scratch) for the "
    "violating fixtures. `unsafe-in-source-detected` overlays one real, existing crate "
    "source file with itself plus an appended `unsafe {}` block, so the scan runs the real "
    "`crates/**/*.rs` glob and the real comment-stripping/regex logic. What this cannot "
    "reach: a change to the *shape* of either rule function itself (no cargo means this "
    "module cannot compile a modified copy to check that), and the `unsafe`-in-source scan "
    "is textual (`strip_rust_comments` + a token regex), exactly as the production gate's "
    "own is — a Rust macro that could hide the token is out of scope for both."
)
_BOUNDARY_02 = (
    "The four `continuum-kernel-*` crates each define their own `Reader` independently "
    "(docs/03 §8 diversity: 'this crate depends on nothing'), but every one names the same "
    "`MAX_TOKEN_BYTES` constant and the same `Reader::token` grammar (empty -> "
    "`TokenFault::Empty`, over `MAX_TOKEN_BYTES` -> `TokenFault::TooLong`, insufficient bytes "
    "-> `Rejection::Truncated`, a non-`is_ascii_graphic` byte -> `TokenFault::NonPrintable`), "
    "confirmed identical by grep across all four `wire.rs` files. `_extract_max_token_bytes` "
    "reads each kernel's real `MAX_TOKEN_BYTES` at run time (the drift tie); "
    "`_reader_token_reference` re-implements the four-way outcome exactly in that order. "
    "Eight real, non-`#[ignore]`d Rust tests (two per kernel) corroborate exactly this "
    "grammar. What this module cannot do is call `Reader::token` itself (no cargo), so a "
    "change to the *order* the four checks run in, rather than their outcomes, would not be "
    "caught here — only TEST-6-06's LRAT-specific model executes past the header. This ID "
    "stays `partial` regardless of that strong certificate-side corroboration: see ABSENCE "
    "for the missing 'trace' half."
)
_ABSENCE_02 = (
    "docs/19 §9's TEST-9-02 bullet, and docs/09's trust-boundary list, both name 'trace' and "
    "'certificate' as two distinct untrusted input kinds. The certificate half (kernel "
    "wire.rs malformed-token/truncation rejection, above) is real, landed, and "
    "Rust-test-corroborated. The trace half has no real implementation at all: "
    "`continuum-corpus` and `continuum-observer` (`crates/continuum-corpus/src/lib.rs`, "
    "`crates/continuum-observer/src/lib.rs`) are both ~20-line documented stubs, and no "
    "schema for a trace-import artifact exists under `notes/plan/schemas/` — the identical "
    "absence `s6_fuzzing_targets.py`'s TEST-6-05 already documents for the same two crates. "
    "No Rust test anywhere references trace import, so this ID is `partial` rather than "
    "`enforced` even though its certificate half alone would meet the bar."
)
_BOUNDARY_03 = (
    "`continuum-context` is real, substantial, non-stub code (`pack.rs`, `budget.rs`; "
    "thousands of lines with their own unit and integration test suites), not a scaffold. "
    "`_check_ceiling` models the shared shape of both real refusals — a request whose size "
    "exceeds a stated ceiling is always a typed error (`PackError::OverNodeCeiling` / "
    "`BudgetError::Exhausted`), never a silent truncation, an unbounded allocation, or a "
    "hang — and the drift check confirms both error names and their `selected`/`ceiling` "
    "(`minimal`/`ceiling`) fields are still literally present in the real source. Three "
    "real, non-`#[ignore]`d Rust tests exercise the real refusal at the real boundary "
    "(the ceiling exactly met vs. one unit under it) for both mechanisms. What this cannot "
    "do: call `RootPack::to_json`/`ChildPack::pack_at` itself (no cargo), so it cannot "
    "observe whether a *third*, not-yet-named resource dimension (wall-clock time, thread "
    "count) is bounded — only the two ceilings docs/19 §9's DoS-limit obligation already has "
    "named production mechanisms for."
)
_BOUNDARY_04 = (
    "`crates/continuum-workspace/src/artifact_path.rs`'s own module doc cites docs/19 §9 "
    "and docs/09 by name for exactly this obligation ('the defense is at construction, not "
    "at use'), and `ArtifactClass::Crashpack` (`crash_*`) is one of the twelve classes "
    "`validate_identity` gates uniformly — there is no separate, weaker path for crashpacks. "
    "`identity-character-class-violation-not-rejected-detected` ports `validate_identity` "
    "(a one-line character-class test, drift-tied by checking the literal predicate text is "
    "still present) against the real traversal corpus the production `#[test]` also uses. "
    "`derived-artifact-path-escapes-root-detected` checks the structural property "
    "`no_derived_path_has_a_relative_or_absolute_segment` and `resolve_stays_under_the_root` "
    "assert directly (three non-empty, non-`.`/`..`, separator-free segments) against "
    "supplied path segments, independent of `validate_identity` — the second, independent "
    "layer those two real tests check. Three real, non-`#[ignore]`d Rust tests corroborate "
    "both layers. What this cannot do: extract a real crashpack archive (no cargo, no "
    "filesystem write) — only the path-construction defense, which is the one docs/19 §9 "
    "and the module's own doc comment name."
)
_ABSENCE_05 = (
    "plan.md §18.3 'Sandboxing' states the requirement in prose only: 'Foreign solvers, "
    "Lean workers, generated code, corpus oracles, and agent code run in isolated, "
    "resource-bounded environments with pinned images/toolchains and read-only inputs. "
    "Output is parsed as untrusted data.' A repository-wide search finds no "
    "`continuum-solver` crate, and zero uses anywhere in `crates/**/*.rs` of "
    "`std::process::Command`, `seccomp`, `setrlimit`/`rlimit`, or `landlock` — nothing spawns "
    "a foreign solver process at all yet, so nothing sandboxes one. The check below is a "
    "standalone reference model of the five named properties (isolation/no-ambient-network, "
    "resource bounds, pinned image, read-only inputs, untrusted output), drift-tied to the "
    "cited plan.md sentence, and demonstrates the model itself is not vacuous — it is not, "
    "and cannot yet be, a test of any production sandboxing mechanism, because none exists."
)
_BOUNDARY_06 = (
    "`notes/plan/schemas/redacted.schema.json` + `schemas/examples/redacted.example.json` "
    "are real, committed artifacts; `redacted-example-schema-violation-detected` reads and "
    "validates them directly (required keys, the `reason` enum, the `redacted: true` const) "
    "rather than porting unrelated logic. `continuum-context::compile::RedactionPolicy` / "
    "`RedactionReason` and `omission::OmissionReason::admits_irretrievability` are real, "
    "landed, non-stub production code: a withheld candidate is recorded with "
    "`OmissionReason::Redaction`, and `admits_irretrievability()` is `true` for exactly "
    "`redaction` and `unsupported` — the property that makes a purge permanent rather than "
    "a soft delete a later stage could still surface. Both the wire-token vocabulary "
    "(`compile.rs`) and the irretrievability set (`omission.rs`) are extracted at run time "
    "(drift-tied), not hand-copied. Three real, non-`#[ignore]`d Rust tests corroborate the "
    "ordering guarantee (redaction is a pre-pass, before every compiler stage), the "
    "irretrievability property, and that it survives every later stage on a concrete "
    "compiled example. What this cannot do: run `CausalCompile` itself (no cargo), so it "
    "cannot observe whether a withheld candidate's *content* (rather than its presence in "
    "`selected()`) ever reaches an intermediate structure — only the typed-omission "
    "guarantee, which is what `redacted.schema.json` and the omission manifest make "
    "checkable without executing the compiler."
)
BOUNDARIES: dict[str, list[str]] = {
    "TEST-9-01": [_BOUNDARY_01],
    "TEST-9-02": [_BOUNDARY_02],
    "TEST-9-03": [_BOUNDARY_03],
    "TEST-9-04": [_BOUNDARY_04],
    "TEST-9-06": [_BOUNDARY_06],
}
ABSENCE: dict[str, str] = {
    "TEST-9-02": _ABSENCE_02,
    "TEST-9-05": _ABSENCE_05,
}

# ---------------------------------------------------------------------------
# Real Rust #[test] evidence (textual, no cargo — see s6_fuzzing_targets.py's
# _rust_test_present, the same discipline). TEST-9-01's evidence is the governance
# gates it calls directly (see _BOUNDARY_01); it names no Rust test.
# ---------------------------------------------------------------------------

RUST_TESTS: dict[str, list[tuple[Path, str]]] = {
    "TEST-9-02": [
        (KERNEL_WIRE_PATHS["Core"], "reader_reports_truncation_instead_of_panicking"),
        (KERNEL_WIRE_PATHS["Core"], "tokens_reject_empty_oversized_and_non_printable"),
        (KERNEL_WIRE_PATHS["Sat"], "reader_reports_truncation_instead_of_panicking"),
        (KERNEL_WIRE_PATHS["Sat"], "tokens_reject_empty_oversized_and_non_printable"),
        (KERNEL_WIRE_PATHS["Smt"], "reader_reports_truncation_instead_of_panicking"),
        (KERNEL_WIRE_PATHS["Smt"], "tokens_reject_empty_oversized_and_non_printable"),
        (KERNEL_WIRE_PATHS["Temporal"], "reader_reports_truncation_instead_of_panicking"),
        (KERNEL_WIRE_PATHS["Temporal"], "tokens_reject_empty_oversized_and_non_printable"),
    ],
    "TEST-9-03": [
        (PACK_RS_PATH, "a_root_records_its_node_ceiling_and_refuses_to_break_it"),
        (BUDGET_RS_PATH, "a_ceiling_below_the_minimal_child_publishes_nothing"),
        (
            ROOT / "crates/continuum-context/tests/pr11_impl06_byte_and_token_budgets.rs",
            "an_over_budget_expansion_publishes_a_smaller_child_recording_the_shortfall",
        ),
    ],
    "TEST-9-04": [
        (ARTIFACT_PATH_RS_PATH, "traversal_and_separator_attempts_are_rejected"),
        (ARTIFACT_PATH_RS_PATH, "no_derived_path_has_a_relative_or_absolute_segment"),
        (ARTIFACT_PATH_RS_PATH, "resolve_stays_under_the_root"),
    ],
    "TEST-9-06": [
        (OMISSION_RS_PATH, "only_two_reasons_can_spell_irretrievability"),
        (ROOT / "crates/continuum-context/src/stage.rs", "redaction_cannot_be_recorded_after_root_selection"),
        (
            ROOT / "crates/continuum-context/tests/pr11_compiler_stages_5_7.rs",
            "redaction_still_wins_over_every_stage_of_this_group",
        ),
    ],
}


def _rust_test_present(path: Path, fn_name: str) -> tuple[bool, str]:
    """Textual, no-cargo check: `fn <fn_name>(` exists in `path`, immediately preceded
    (skipping only blank lines and `//`/`///` comments) by an attribute run that
    includes `#[test]` and no `#[ignore]`."""
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        return False, f"could not read {path}: {exc}"
    marker = f"fn {fn_name}("
    at = text.find(marker)
    if at == -1:
        return False, f"{path}: no `fn {fn_name}(` found"
    attrs: list[str] = []
    for line in reversed(text[:at].splitlines()):
        s = line.strip()
        if s.startswith("#["):
            attrs.append(s)
            continue
        if s == "" or s.startswith("///") or s.startswith("//"):
            continue
        break
    if not any(a.startswith("#[test") for a in attrs):
        return False, f"{path}: `fn {fn_name}` is not attributed #[test]"
    if any("ignore" in a for a in attrs):
        return False, f"{path}: `fn {fn_name}` carries #[ignore]"
    return True, ""


def _finding(rule: str, subject: str, message: str) -> dict[str, str]:
    return tsys.Finding(rule, RULES[rule], subject, message).as_json()


def _load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


# ---------------------------------------------------------------------------
# TEST-9-01: dependency and unsafe audits — the real governance gates, called
# directly (not ported). See module docstring and _BOUNDARY_01.
# ---------------------------------------------------------------------------

_GOVERNANCE_MODULES: dict[str, ModuleType] = {}


def _load_governance_module(stem: str) -> ModuleType:
    if stem in _GOVERNANCE_MODULES:
        return _GOVERNANCE_MODULES[stem]
    path = GOVERNANCE_DIR / f"{stem}.py"
    spec = importlib.util.spec_from_file_location(f"test_policy_s9_{stem}", path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    # Registered in sys.modules before exec: dataclasses (order=True, with `from
    # __future__ import annotations`) resolves field types via
    # sys.modules[cls.__module__], which is only set once a module is registered.
    sys.modules[spec.name] = module
    try:
        spec.loader.exec_module(module)
    except BaseException:
        del sys.modules[spec.name]
        raise
    _GOVERNANCE_MODULES[stem] = module
    return module


_UNSAFE_CHECK_RULE = {
    "workspace-lint-forbids-unsafe": "unsafe-lint-not-forbid-detected",
    "no-unsafe-in-sources": "unsafe-in-source-detected",
}
_AUDIT_CHECK_RULE = {
    "every-external-dependency-is-audited": "dependency-not-audited-detected",
    "checksum-matches-lockfile": "dependency-checksum-mismatch-detected",
}


_UNSAFE_CODE_LINE_RE = re.compile(r'(?m)^unsafe_code = "forbid"$')


def _cargo_toml_with_unsafe_level(level: str) -> str:
    # The marker text also appears quoted inside the surrounding doc comment
    # (`# \`unsafe_code = "forbid"\` applies to every workspace crate...`), so a plain
    # string replace would silently mutate the comment instead of the real TOML
    # assignment. Anchor to a line that is exactly the assignment (no leading `#`).
    text = CARGO_TOML_PATH.read_text(encoding="utf-8")
    new_text, n = _UNSAFE_CODE_LINE_RE.subn(f'unsafe_code = "{level}"', text, count=1)
    if n != 1:
        raise ValueError('drift: Cargo.toml no longer has a line reading exactly unsafe_code = "forbid"')
    return new_text


def _source_with_injected_unsafe(rel_path: str) -> str:
    text = (ROOT / rel_path).read_text(encoding="utf-8")
    return text + "\n\nfn __test_policy_injected_unsafe_marker() {\n    unsafe {}\n}\n"


def _audits_toml_removing_package(package: str) -> str:
    text = DEPENDENCY_AUDITS_PATH.read_text(encoding="utf-8")
    pattern = re.compile(rf"\[\[audits\.{re.escape(package)}\]\]\n.*?(?=\n\[\[audits\.|\Z)", re.S)
    new_text, n = pattern.subn("", text, count=1)
    if n != 1:
        raise ValueError(f"drift: could not find exactly one [[audits.{package}]] block to remove")
    return new_text


def _audits_toml_corrupting_checksum(package: str) -> str:
    text = DEPENDENCY_AUDITS_PATH.read_text(encoding="utf-8")
    pattern = re.compile(rf'(\[\[audits\.{re.escape(package)}\]\]\nversion = "[^"]+"\nchecksum = ")[0-9a-f]{{64}}(")')
    new_text, n = pattern.subn(lambda m: m.group(1) + "0" * 64 + m.group(2), text, count=1)
    if n != 1:
        raise ValueError(f"drift: could not find [[audits.{package}]]'s checksum to corrupt")
    return new_text


def _check_governance_gate(payload: dict) -> list[dict[str, str]]:
    cp = _load_governance_module("check_code_policy")
    da = _load_governance_module("check_dependency_audit")
    gate = payload["gate"]
    if gate == "clean":
        violations: list[Any] = list(cp.rule_unsafe_forbidden(cp.Tree(ROOT))) + list(
            da.rule_dependency_audit(da.Tree(ROOT))
        )
    elif gate == "unsafe-lint-level":
        overlay = {"Cargo.toml": _cargo_toml_with_unsafe_level(payload["level"])}
        violations = list(cp.rule_unsafe_forbidden(cp.Tree(ROOT).with_changes(overlay)))
    elif gate == "unsafe-source":
        overlay = {payload["path"]: _source_with_injected_unsafe(payload["path"])}
        violations = list(cp.rule_unsafe_forbidden(cp.Tree(ROOT).with_changes(overlay)))
    elif gate == "dependency-missing":
        overlay = {DEPENDENCY_AUDITS_REL: _audits_toml_removing_package(payload["package"])}
        violations = list(da.rule_dependency_audit(da.Tree(ROOT).with_changes(overlay)))
    elif gate == "dependency-checksum":
        overlay = {DEPENDENCY_AUDITS_REL: _audits_toml_corrupting_checksum(payload["package"])}
        violations = list(da.rule_dependency_audit(da.Tree(ROOT).with_changes(overlay)))
    else:
        raise ValueError(f"unknown gate {gate!r}")
    out: list[dict[str, str]] = []
    for v in violations:
        rule = _UNSAFE_CHECK_RULE.get(v.check) or _AUDIT_CHECK_RULE.get(v.check)
        if rule is not None:
            out.append(_finding(rule, payload.get("subject", gate), v.render()))
    return out


# ---------------------------------------------------------------------------
# TEST-9-02: malformed untrusted certificate inputs (the kernel wire.rs Reader/token
# grammar). See module docstring, _BOUNDARY_02, _ABSENCE_02.
# ---------------------------------------------------------------------------

_MAX_TOKEN_BYTES_RE = re.compile(r"pub const MAX_TOKEN_BYTES: usize = (\d+);")


def _extract_max_token_bytes(kernel: str) -> int:
    text = KERNEL_WIRE_PATHS[kernel].read_text(encoding="utf-8")
    m = _MAX_TOKEN_BYTES_RE.search(text)
    if not m:
        raise ValueError(f"drift: no MAX_TOKEN_BYTES constant found in {KERNEL_WIRE_PATHS[kernel]}")
    if "is_ascii_graphic" not in text:
        raise ValueError(f"drift: {KERNEL_WIRE_PATHS[kernel]} no longer mentions is_ascii_graphic")
    return int(m.group(1))


def _reader_token_reference(max_len: int, data: bytes) -> str:
    """Port of Reader::token's four-way outcome, in the real declaration order:
    length-prefix truncation, empty, oversized, body truncation, non-printable."""
    if len(data) < 2:
        return "truncated"
    declared = int.from_bytes(data[:2], "big")
    if declared == 0:
        return "empty"
    if declared > max_len:
        return "oversized"
    body = data[2:]
    if len(body) < declared:
        return "truncated"
    token = body[:declared]
    if any(not (0x21 <= b <= 0x7E) for b in token):
        return "non_printable"
    return "ok"


_WIRE_OUTCOME_RULE = {
    "empty": "wire-token-empty-rejected-detected",
    "oversized": "wire-token-oversized-rejected-detected",
    "truncated": "wire-reader-truncated-rejected-detected",
    "non_printable": "wire-token-non-printable-rejected-detected",
}


def _check_wire_token(payload: dict) -> list[dict[str, str]]:
    kernel = payload["kernel"]
    max_len = _extract_max_token_bytes(kernel)
    data = bytes.fromhex(payload["raw_hex"])
    actual = _reader_token_reference(max_len, data)
    claimed = payload["claimed"]
    if actual == claimed:
        return []
    rule = _WIRE_OUTCOME_RULE.get(actual)
    if rule is None:
        return []
    return [
        _finding(
            rule,
            f"{kernel}:{payload['raw_hex'][:16]}",
            f"claimed outcome {claimed!r} does not match the real Reader::token contract "
            f"(MAX_TOKEN_BYTES={max_len} extracted from the real {kernel} wire.rs): actual {actual!r}",
        )
    ]


def _gen_wire_token_case(seed: int) -> tuple[bytes, str]:
    kind = seed % 5
    if kind == 0:
        return (0).to_bytes(2, "big"), "empty"
    if kind == 1:
        declared = 200 + (seed % 50)
        body = bytes([0x41]) * (declared % 30)
        return declared.to_bytes(2, "big") + body, "oversized"
    if kind == 2:
        declared = 10 + seed % 5
        short_by = 1 + (seed % 3)
        body = bytes([0x41]) * max(0, declared - short_by)
        return declared.to_bytes(2, "big") + body, "truncated"
    if kind == 3:
        declared = 4 + seed % 4
        body = bytearray([0x41] * declared)
        body[seed % declared] = 0x00 if seed % 2 == 0 else 0x20
        return declared.to_bytes(2, "big") + bytes(body), "non_printable"
    declared = 1 + seed % 8
    body = bytes([0x30 + (i % 10) for i in range(declared)])
    return declared.to_bytes(2, "big") + body, "ok"


# ---------------------------------------------------------------------------
# TEST-9-03: denial-of-service limits (continuum-context resource ceilings).
# See _BOUNDARY_03.
# ---------------------------------------------------------------------------


def _check_ceiling_drift() -> list[str]:
    problems: list[str] = []
    pack_text = PACK_RS_PATH.read_text(encoding="utf-8")
    if "OverNodeCeiling {" not in pack_text or "selected: u64" not in pack_text:
        problems.append(f"drift: {PACK_RS_PATH} no longer declares PackError::OverNodeCeiling { {'selected', 'ceiling'} }")
    budget_text = BUDGET_RS_PATH.read_text(encoding="utf-8")
    if "Exhausted {" not in budget_text or "minimal: u64" not in budget_text:
        problems.append(f"drift: {BUDGET_RS_PATH} no longer declares BudgetError::Exhausted {{ minimal, ceiling }}")
    return problems


def _check_ceiling(payload: dict) -> list[dict[str, str]]:
    kind = payload["ceiling_kind"]
    ceiling = payload["ceiling"]
    requested = payload["requested"]
    over = requested > ceiling
    claimed_refused = payload["claimed_refused"]
    if over == claimed_refused:
        return []
    rule = "node-ceiling-exceeded-not-refused-detected" if kind == "node" else "byte-ceiling-exhaustion-not-refused-detected"
    return [
        _finding(
            rule,
            f"{kind}:{requested}/{ceiling}",
            f"requested {requested} against ceiling {ceiling}: real {kind} ceiling refusal is "
            f"{'required' if over else 'not required'} (PackError::OverNodeCeiling / BudgetError::Exhausted), "
            f"fixture claims refused={claimed_refused}",
        )
    ]


# ---------------------------------------------------------------------------
# TEST-9-04: path traversal in crashpacks (continuum-workspace artifact_path.rs).
# See _BOUNDARY_04.
# ---------------------------------------------------------------------------

_IDENTITY_CHARCLASS_PHRASE = "c.is_ascii_alphanumeric() || c == '_' || c == '-'"
_SHARD_LEN_RE = re.compile(r"pub const SHARD_LEN: usize = (\d+);")
_IDENTITY_RE = re.compile(r"^[A-Za-z0-9_-]+$")


def _check_identity_drift() -> list[str]:
    text = ARTIFACT_PATH_RS_PATH.read_text(encoding="utf-8")
    problems = []
    if _IDENTITY_CHARCLASS_PHRASE not in text:
        problems.append(f"drift: {ARTIFACT_PATH_RS_PATH} no longer states {_IDENTITY_CHARCLASS_PHRASE!r}")
    if '"crash"' not in text or 'Self::Crashpack => "crash_"' not in text:
        problems.append(f"drift: {ARTIFACT_PATH_RS_PATH} no longer declares ArtifactClass::Crashpack's crash/crash_ tokens")
    if not _SHARD_LEN_RE.search(text):
        problems.append(f"drift: {ARTIFACT_PATH_RS_PATH} no longer declares SHARD_LEN")
    return problems


def _validate_identity_reference(identity: str) -> bool:
    """Port of validate_identity: True when the identity is accepted."""
    return bool(identity) and bool(_IDENTITY_RE.match(identity))


def _check_crashpack_identity(payload: dict) -> list[dict[str, str]]:
    identity = payload["identity"]
    accepted_ref = _validate_identity_reference(identity)
    claimed_accepted = payload["claimed"] == "accepted"
    if accepted_ref == claimed_accepted:
        return []
    return [
        _finding(
            "identity-character-class-violation-not-rejected-detected",
            identity,
            f"identity {identity!r}: real validate_identity acceptance is {accepted_ref}, "
            f"fixture claims {claimed_accepted}",
        )
    ]


def _derived_path_is_safe(segments: list[str]) -> bool:
    if len(segments) != 3:
        return False
    for seg in segments:
        if seg in ("", ".", ".."):
            return False
        if any(c in seg for c in "/\\:"):
            return False
    return True


def _check_crashpack_derived_path(payload: dict) -> list[dict[str, str]]:
    segments = payload["segments"]
    safe_ref = _derived_path_is_safe(segments)
    claimed_safe = payload["claimed_safe"]
    if safe_ref == claimed_safe:
        return []
    return [
        _finding(
            "derived-artifact-path-escapes-root-detected",
            "/".join(str(s) for s in segments),
            f"segments {segments!r}: real store-path safety is {safe_ref}, fixture claims {claimed_safe}",
        )
    ]


# ---------------------------------------------------------------------------
# TEST-9-05: solver sandboxing — no production mechanism exists. Standalone
# reference model of plan.md §18.3, drift-tied. See _ABSENCE_05.
# ---------------------------------------------------------------------------

_SANDBOX_PLAN_PHRASE = (
    "Foreign solvers, Lean workers, generated code, corpus oracles, and agent code run in "
    "isolated, resource-bounded environments with pinned images/toolchains and read-only "
    "inputs. Output is parsed as untrusted data."
)
_SANDBOX_PROPERTIES = ("isolated", "resource_bounded", "pinned_image", "read_only_inputs", "output_untrusted")


def _check_sandbox_drift() -> list[str]:
    text = PLAN_MD_PATH.read_text(encoding="utf-8")
    if _SANDBOX_PLAN_PHRASE not in text:
        return [f"drift: {PLAN_MD_PATH} §18.3 no longer states the cited Sandboxing sentence verbatim"]
    return []


def _check_sandbox_config(payload: dict) -> list[dict[str, str]]:
    config = payload["config"]
    out: list[dict[str, str]] = []
    for prop in _SANDBOX_PROPERTIES:
        if not config.get(prop, False):
            out.append(
                _finding(
                    "sandbox-isolation-property-violated-detected",
                    prop,
                    f"declared sandbox config does not satisfy {prop!r} "
                    f"(plan.md §18.3: {_SANDBOX_PLAN_PHRASE!r})",
                )
            )
    return out


# ---------------------------------------------------------------------------
# TEST-9-06: secret-redaction tests. See _BOUNDARY_06.
# ---------------------------------------------------------------------------


def _check_redacted_instance(instance: Any) -> list[str]:
    schema = _load_json(REDACTED_SCHEMA_PATH)
    errors: list[str] = []
    if not isinstance(instance, dict):
        return ["instance is not an object"]
    for key in schema.get("required", []):
        if key not in instance:
            errors.append(f"missing required property {key!r}")
    props = schema.get("properties", {})
    for key, value in instance.items():
        spec = props.get(key)
        if spec is None:
            errors.append(f"unexpected property {key!r} (additionalProperties: false)")
            continue
        if "const" in spec and value != spec["const"]:
            errors.append(f"{key}: expected const {spec['const']!r}, got {value!r}")
        if "enum" in spec and value not in spec["enum"]:
            errors.append(f"{key}: {value!r} not in enum {spec['enum']!r}")
        if spec.get("type") == "string" and not isinstance(value, str):
            errors.append(f"{key}: expected string, got {type(value).__name__}")
    return errors


def _check_redacted_instance_fixture(payload: dict) -> list[dict[str, str]]:
    errs = _check_redacted_instance(payload["instance"])
    return [_finding("redacted-example-schema-violation-detected", "$", e) for e in errs]


def _mutate_redacted(golden: dict, kind: str) -> Any:
    import copy as _copy

    m = _copy.deepcopy(golden)
    if kind == "missing-reason":
        del m["reason"]
    elif kind == "bad-reason":
        m["reason"] = "not-a-real-reason"
    elif kind == "redacted-not-true":
        m["redacted"] = False
    elif kind == "unknown-field":
        m["secret_payload_that_should_have_stayed_redacted"] = "oops"
    elif kind == "commitment-not-string":
        m["commitment"] = 12345
    else:
        raise ValueError(kind)
    return m


def _extract_omission_wire_and_irretrievable() -> tuple[dict[str, str], set[str]]:
    text = OMISSION_RS_PATH.read_text(encoding="utf-8")
    wire = dict(re.findall(r'Self::(\w+) => "([a-z-]+)"', text))
    m = re.search(r"admits_irretrievability\(self\) -> bool \{\s*matches!\(self, ([^)]+)\)", text)
    if not m:
        raise ValueError(f"drift: {OMISSION_RS_PATH}'s admits_irretrievability matches! arm not found")
    irretrievable_variants = set(re.findall(r"Self::(\w+)", m.group(1)))
    irretrievable_wire = {wire[v] for v in irretrievable_variants if v in wire}
    if not wire or not irretrievable_wire:
        raise ValueError(f"drift: {OMISSION_RS_PATH}'s OmissionReason vocabulary extraction is empty")
    return wire, irretrievable_wire


def _check_redaction_reason_vocabulary_drift() -> list[str]:
    text = COMPILE_RS_PATH.read_text(encoding="utf-8")
    wire = dict(re.findall(r'Self::(\w+) => "(summarized|lost|purged)"', text))
    rust_reasons = sorted(wire.values())
    if len(rust_reasons) != 3:
        return [f"drift: {COMPILE_RS_PATH}'s RedactionReason::as_wire_str no longer has exactly 3 arms (found {rust_reasons})"]
    schema = _load_json(REDACTED_SCHEMA_PATH)
    schema_reasons = sorted(schema.get("properties", {}).get("reason", {}).get("enum", []))
    if rust_reasons != schema_reasons:
        return [
            f"RedactionReason's real wire vocabulary {rust_reasons} != {REDACTED_SCHEMA_PATH}'s reason enum {schema_reasons}"
        ]
    return []


def _check_redacted_irretrievability(payload: dict) -> list[dict[str, str]]:
    _wire, irretrievable_wire = _extract_omission_wire_and_irretrievable()
    reason_wire = payload["reason"]
    ref_irretrievable = reason_wire in irretrievable_wire
    claimed_expandable = payload["claimed_expandable"]
    if claimed_expandable != ref_irretrievable:
        return []
    return [
        _finding(
            "redacted-item-wrongly-marked-expandable-detected",
            reason_wire,
            f"omission reason {reason_wire!r}: real admits_irretrievability()={ref_irretrievable}, so "
            f"expandable must be {not ref_irretrievable}, fixture claims expandable={claimed_expandable}",
        )
    ]


# ---------------------------------------------------------------------------
# Fixtures dispatch.
# ---------------------------------------------------------------------------


def check_fixture(system: Any) -> list[dict[str, str]]:
    if not isinstance(system, dict) or "kind" not in system:
        return [_finding("unsafe-lint-not-forbid-detected", "<payload>", "payload must be an object with a 'kind'")]
    kind = system["kind"]
    payload = {k: v for k, v in system.items() if k != "kind"}
    if kind == "governance-gate":
        return _check_governance_gate(payload)
    if kind == "wire-token":
        return _check_wire_token(payload)
    if kind == "ceiling":
        return _check_ceiling(payload)
    if kind == "crashpack-identity":
        return _check_crashpack_identity(payload)
    if kind == "crashpack-derived-path":
        return _check_crashpack_derived_path(payload)
    if kind == "sandbox-config":
        return _check_sandbox_config(payload)
    if kind == "redacted-instance":
        return _check_redacted_instance_fixture(payload)
    if kind == "redacted-irretrievability":
        return _check_redacted_irretrievability(payload)
    return [_finding("unsafe-lint-not-forbid-detected", "<payload>", f"unknown fixture kind {kind!r}")]


# ---------------------------------------------------------------------------
# The real run.
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
            failures[rid].append(f"mutant on {subject} was not detected: the checker reported it clean")

    def need(rid: str, key: str, what: str) -> None:
        if corpus[rid].get(key, 0) == 0:
            failures[rid].append(f"corpus exercises no {what} ({key} = 0): the pass would be vacuous")

    # -- TEST-9-01: dependency and unsafe audits (real governance gates) ------
    try:
        cp = _load_governance_module("check_code_policy")
        da = _load_governance_module("check_dependency_audit")
    except OSError as exc:
        failures["TEST-9-01"].append(f"could not load a governance module: {exc}")
        cp = da = None  # type: ignore[assignment]
    if cp is not None and da is not None:
        baseline_unsafe = cp.rule_unsafe_forbidden(cp.Tree(ROOT))
        baseline_audit = da.rule_dependency_audit(da.Tree(ROOT))
        if baseline_unsafe:
            failures["TEST-9-01"].append(
                f"the real GOV-1-02 unsafe-forbidden gate already fails on the unmodified tree: {baseline_unsafe[0].render()}"
            )
        if baseline_audit:
            failures["TEST-9-01"].append(
                f"the real dependency-audit-posture gate already fails on the unmodified tree: {baseline_audit[0].render()}"
            )
        try:
            lock_doc = da.tomllib.loads(CARGO_LOCK_PATH.read_text(encoding="utf-8"))
            bump("TEST-9-01", "externally_sourced_dependencies", len(da.externally_sourced_packages(lock_doc)))
        except OSError as exc:
            failures["TEST-9-01"].append(f"could not read {CARGO_LOCK_PATH}: {exc}")
        bump("TEST-9-01", "workspace_crates_scanned", len(cp.load_workspace(cp.Tree(ROOT)).crates))

        for gate, subject in (
            ({"gate": "unsafe-lint-level", "level": "warn"}, "unsafe-lint-level"),
            ({"gate": "unsafe-source", "path": UNSAFE_SOURCE_REL}, "unsafe-source"),
            ({"gate": "dependency-missing", "package": "blake3"}, "dependency-missing"),
            ({"gate": "dependency-checksum", "package": "blake3"}, "dependency-checksum"),
        ):
            found = _check_governance_gate(gate)
            mutate("TEST-9-01", True, bool(found), subject)
            bump("TEST-9-01", "mutations")

    need("TEST-9-01", "externally_sourced_dependencies", "a real Cargo.lock package from an external source")
    need("TEST-9-01", "workspace_crates_scanned", "a real workspace crate manifest")

    # -- TEST-9-02: malformed certificate inputs (kernel wire.rs) --------------
    outcome_counts: dict[str, int] = {}
    for kernel in KERNEL_WIRE_PATHS:
        try:
            max_len = _extract_max_token_bytes(kernel)
        except (OSError, ValueError) as exc:
            failures["TEST-9-02"].append(str(exc))
            continue
        bump("TEST-9-02", "kernels_checked")
        for seed in range(20):
            data, expected = _gen_wire_token_case(seed * 7 + hash(kernel) % 5)
            actual = _reader_token_reference(max_len, data)
            outcome_counts[actual] = outcome_counts.get(actual, 0) + 1
            clean = _check_wire_token({"kernel": kernel, "raw_hex": data.hex(), "claimed": actual})
            if clean:
                failures["TEST-9-02"].append(f"{kernel} seed {seed}: the reference's own honest claim {actual!r} was flagged")
            if actual != "ok":
                mut = _check_wire_token({"kernel": kernel, "raw_hex": data.hex(), "claimed": "ok"})
                mutate("TEST-9-02", True, bool(mut), f"{kernel} seed {seed} ({actual} claimed ok)")
    for outcome in ("empty", "oversized", "truncated", "non_printable", "ok"):
        bump("TEST-9-02", f"outcomes_{outcome}", outcome_counts.get(outcome, 0))
        need("TEST-9-02", f"outcomes_{outcome}", f"a generated case with outcome {outcome!r}")

    rust_evidence: dict[str, list[dict[str, Any]]] = {}
    for rid, tests in RUST_TESTS.items():
        entries = []
        for path, fn_name in tests:
            ok, msg = _rust_test_present(path, fn_name)
            entries.append({"path": str(path.relative_to(ROOT)), "test": fn_name, "present": ok})
            bump(rid, "rust_tests_named")
            if ok:
                bump(rid, "rust_tests_present")
            else:
                failures[rid].append(f"named Rust test evidence missing: {msg}")
        rust_evidence[rid] = entries
        need(rid, "rust_tests_present", "named real, existing, non-#[ignore]d Rust test")

    # -- TEST-9-03: denial-of-service limits (continuum-context ceilings) ------
    drift = _check_ceiling_drift()
    if drift:
        failures["TEST-9-03"].extend(drift)
    ceiling_cases = [
        ("node", 4, 4, False),
        ("node", 4, 7, True),
        ("node", 0, 1, True),
        ("byte", 1000, 500, False),
        ("byte", 500, 1000, True),
        ("byte", 0, 1, True),
    ]
    for kind, ceiling, requested, over in ceiling_cases:
        bump("TEST-9-03", f"{kind}_cases")
        clean = _check_ceiling({"ceiling_kind": kind, "ceiling": ceiling, "requested": requested, "claimed_refused": over})
        if clean:
            failures["TEST-9-03"].append(f"{kind} ceiling={ceiling} requested={requested}: the reference's own honest claim was flagged")
        mut = _check_ceiling({"ceiling_kind": kind, "ceiling": ceiling, "requested": requested, "claimed_refused": not over})
        mutate("TEST-9-03", True, bool(mut), f"{kind} ceiling={ceiling} requested={requested} (claim inverted)")
    need("TEST-9-03", "node_cases", "a node-ceiling case")
    need("TEST-9-03", "byte_cases", "a byte-ceiling case")

    # -- TEST-9-04: path traversal in crashpacks --------------------------------
    drift = _check_identity_drift()
    if drift:
        failures["TEST-9-04"].extend(drift)
    identity_cases = [
        ("cp7m3x9", True),
        ("Receipt_02-b", True),
        ("..", False),
        ("../../etc/passwd", False),
        ("a/b", False),
        ("a\\b", False),
        ("C:", False),
        ("", False),
        ("a\x00b", False),
        ("héllo", False),
    ]
    for identity, accepted in identity_cases:
        bump("TEST-9-04", "identities_checked")
        if not accepted:
            bump("TEST-9-04", "traversal_identities")
        clean = _check_crashpack_identity({"identity": identity, "claimed": "accepted" if accepted else "rejected"})
        if clean:
            failures["TEST-9-04"].append(f"identity {identity!r}: the reference's own honest claim was flagged")
        mut = _check_crashpack_identity({"identity": identity, "claimed": "rejected" if accepted else "accepted"})
        mutate("TEST-9-04", True, bool(mut), f"identity {identity!r} (claim inverted)")

    golden_paths = [
        ["ws", "7m", "7m3x9"],
        ["crash", "cp", "cp7m3x9"],
        ["receipt", "2c", "2c_ab-9"],
        ["ev", "a", "a"],
    ]
    for segments in golden_paths:
        bump("TEST-9-04", "derived_paths_checked")
        clean = _check_crashpack_derived_path({"segments": segments, "claimed_safe": True})
        if clean:
            failures["TEST-9-04"].append(f"segments {segments!r}: a real, safe derived path was flagged")
        broken = [segments[0], "..", segments[2]]
        bump("TEST-9-04", "traversal_paths")
        mut = _check_crashpack_derived_path({"segments": broken, "claimed_safe": True})
        mutate("TEST-9-04", True, bool(mut), f"segments {broken!r} (.. injected, claimed safe)")

    need("TEST-9-04", "traversal_identities", "an identity a traversal/separator character makes unsafe")
    need("TEST-9-04", "derived_paths_checked", "a real class/shard/identity derived path")
    need("TEST-9-04", "traversal_paths", "a derived path with an unsafe segment")

    # -- TEST-9-05: solver sandboxing (standalone model; no production mechanism) --
    drift = _check_sandbox_drift()
    if drift:
        failures["TEST-9-05"].extend(drift)
    compliant = {p: True for p in _SANDBOX_PROPERTIES}
    bump("TEST-9-05", "compliant_configs")
    clean = _check_sandbox_config({"config": compliant})
    if clean:
        failures["TEST-9-05"].append("a fully compliant sandbox config was itself flagged")
    for prop in _SANDBOX_PROPERTIES:
        broken = dict(compliant)
        broken[prop] = False
        bump("TEST-9-05", "violating_configs")
        found = _check_sandbox_config({"config": broken})
        mutate("TEST-9-05", True, bool(found), f"sandbox config missing {prop!r}")
    need("TEST-9-05", "compliant_configs", "a config satisfying every plan §18.3 property")
    need("TEST-9-05", "violating_configs", "a config violating one plan §18.3 property")

    # -- TEST-9-06: secret-redaction tests --------------------------------------
    drift = _check_redaction_reason_vocabulary_drift()
    if drift:
        failures["TEST-9-06"].extend(drift)
    if not REDACTED_SCHEMA_PATH.exists() or not REDACTED_EXAMPLE_PATH.exists():
        failures["TEST-9-06"].append("redacted.schema.json or redacted.example.json is missing")
    else:
        golden = _load_json(REDACTED_EXAMPLE_PATH)
        clean = _check_redacted_instance_fixture({"instance": golden})
        if clean:
            failures["TEST-9-06"].append(f"the real redacted.example.json golden was itself flagged: {clean[0]['message']}")
        bump("TEST-9-06", "golden_checked")
        for kind in ("missing-reason", "bad-reason", "redacted-not-true", "unknown-field", "commitment-not-string"):
            mutant = _mutate_redacted(golden, kind)
            found = _check_redacted_instance_fixture({"instance": mutant})
            mutate("TEST-9-06", True, bool(found), f"redacted.example.json ({kind})")
            bump("TEST-9-06", "mutations")

    try:
        _wire, irretrievable_wire = _extract_omission_wire_and_irretrievable()
    except ValueError as exc:
        failures["TEST-9-06"].append(str(exc))
        irretrievable_wire = set()
    for reason_wire in ("budget", "redaction", "unsupported", "heuristic-cutoff", "slice-irrelevant"):
        expandable_ref = reason_wire not in irretrievable_wire
        bump("TEST-9-06", "omission_reasons_checked")
        if reason_wire in ("redaction", "unsupported"):
            bump("TEST-9-06", "irretrievable_reasons_checked")
        clean = _check_redacted_irretrievability({"reason": reason_wire, "claimed_expandable": expandable_ref})
        if clean:
            failures["TEST-9-06"].append(f"reason {reason_wire!r}: the reference's own honest claim was flagged")
        mut = _check_redacted_irretrievability({"reason": reason_wire, "claimed_expandable": not expandable_ref})
        mutate("TEST-9-06", True, bool(mut), f"reason {reason_wire!r} (expandable claim inverted)")

    need("TEST-9-06", "golden_checked", "the real redacted.example.json golden")
    need("TEST-9-06", "irretrievable_reasons_checked", "an irretrievable omission reason (redaction/unsupported)")

    for rid in OBLIGATIONS:
        if mutants[rid]["applied"] and mutants[rid]["detected"] == 0:
            failures[rid].append(f"{rid}: the mutant was never detected across the corpus (the checker would be vacuous)")

    results: dict[str, Any] = {}
    for rid, summary in OBLIGATIONS.items():
        entry: dict[str, Any] = {
            "status": STATUS[rid],
            "rules": sorted(r for r, q in RULES.items() if q == rid),
            "failures": failures[rid],
            "corpus": corpus[rid],
            "mutant": mutants[rid],
        }
        if rid in rust_evidence:
            entry["rust_tests"] = rust_evidence[rid]
        if rid in BOUNDARIES:
            entry["boundaries"] = BOUNDARIES[rid]
        if rid in ABSENCE:
            entry["absence"] = ABSENCE[rid]
        results[rid] = entry
    return {
        "requirements": results,
        "unclaimed": ["TEST-9-07"],
    }
