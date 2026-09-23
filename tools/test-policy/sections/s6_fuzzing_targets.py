"""docs/19 §6 "Fuzzing" — TEST-6-01 … TEST-6-06.

docs/19 §6 lists eight fuzz targets; this module claims the first six: parser
and canonical encodings; CIR validator; certificate formats; domain-pack
commands/faults; trace importers; solver proof parsers. (TEST-6-07 "replay
state machine" and TEST-6-08 "schema migrations" are left to a later module;
see `unclaimed` in the evidence file — README-test.md "Adding a section".)

This is a `--no-cargo`, Python-stdlib-only harness (README-test.md). It
cannot fuzz a Rust binary. What it can do, and does, is read the same
committed artifacts a real fuzz target would be seeded from — the schemas
under `notes/plan/schemas/` (INV-003, "schemas decide, prose does not"), the
real committed golden instances under `notes/plan/schemas/examples/`, and the
literal wire-format constants declared in `crates/continuum-kernel-*/src/
wire.rs` — and enforce the exact malformed-input handling those artifacts
already specify, against adversarial fixtures built to violate them.

Per README-test.md step 5, an ID is claimed `enforced` only where two things
both hold: the check runs directly against a real, machine-normative
artifact this revision reads from disk (a committed schema file interpreted
by this module's own small JSON-Schema-subset evaluator, a real committed
example, or a real kernel source constant extracted at run time — never a
hand-authored guess at its shape), AND — because a Python re-implementation
of a Rust predicate is not, by itself, evidence about the Rust code — this
module names specific, real, existing, non-`#[ignore]`d Rust `#[test]`
functions that exercise the corresponding real implementation, checked for
presence by `_rust_test_present` (a textual check: no cargo, no compiling,
just confirming the named test still exists and is not disabled). Wherever
this module's own check is itself a port of Rust logic (not just a reader of
a real artifact), a drift check ties it back to its Rust source at run time
(TEST-6-03 extracts the kernels' real `MAGIC` bytes instead of hand-copying
them; TEST-6-06 asserts the real wire.rs doc-comment phrases its model is
built from are still present). An ID is claimed `partial` where no such
named, checked Rust test exists for what the obligation is about — the check
still runs, with fixtures, and still passes with zero failures; `partial`
states an honesty boundary about production coverage, not a broken check
(see `s4_mutation_testing.py`'s `BOUNDARIES` for the precedent this follows).

| ID | Real surface enforced | Rust test evidence named | Status |
|---|---|---|---|
| TEST-6-01 | every `notes/plan/schemas/*.schema.json` document's `$id`/`schema_epoch`/`schema_kind` identity grammar (schemas/README.md) | `gate_g2_01_acceptance.rs::every_shipped_schema_document_declares_the_readme_identity_triple` (the same check, in Rust, against the same real files) + `continuum-value/src/identity.rs`'s real canonical-identity-encoding tests | enforced |
| TEST-6-02 | `notes/plan/schemas/cir.schema.json` + `schemas/examples/minimal.cir.json`, via this module's own Draft-2020-12-subset evaluator | only `value.rs::value_kinds_are_the_cir_schema_value_kinds`, which checks one enum's vocabulary parity, not CIR structural validity (`continuum-cir` itself is a 20-line stub: no Rust code validates a CIR document) | partial |
| TEST-6-03 | `Family::route`'s documented algorithm (`crates/continuum-certificate/src/family.rs`) against the four kernels' real `MAGIC` constants, extracted from `crates/continuum-kernel-*/src/wire.rs` at run time (drift-tied, not hand-copied) | `family_routing.rs`'s four routing tests (partition, too-short, unclaimed magic, wrong-family magic) | enforced |
| TEST-6-04 | `notes/plan/schemas/domain-pack.schema.json` + `schemas/examples/storage-pack.manifest.json` | none content-level: `inv003_no_prose_only_evidence.rs::every_schema_file_is_claimed_by_exactly_one_table_row_and_vice_versa` only checks that the schema *file* is inventoried, not that a manifest's operations/faults are validated; `continuum-pack-storage`, the adapter the manifest names, does not exist under `crates/` | partial |
| TEST-6-05 | `notes/plan/rfcs/0006-production-trace-conformance.md`'s typed outcome vocabulary and "MUST NOT invent a total event order" rule, modeled standalone | none: no crate exists (`continuum-corpus`, `continuum-observer` are 20-line stubs) | partial |
| TEST-6-06 | the LRAT body grammar's canonicity rules, quoted verbatim from `crates/continuum-kernel-sat/src/wire.rs`'s own doc comment, modeled against that cited grammar (drift-tied: the exact grammar phrases are re-checked present at run time) | `wire.rs::the_canonical_literal_order_is_by_variable_then_polarity` + `check.rs`'s five malformed-proof rejection tests (unordered/duplicated literals, descending clause id, unordered deletion list, undefined-clause antecedent, undefined-clause deletion) | enforced |

None of this reads or writes `crates/*/Cargo.toml`, `tsys.py`, or another
section's fixtures; it is a new file per README-test.md's extension rule.
"""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

import tsys

SECTION = 6
TITLE = "Fuzzing"

# tools/test-policy/sections/s6_fuzzing_targets.py -> repo root
ROOT = Path(__file__).resolve().parents[3]
SCHEMAS_DIR = ROOT / "notes/plan/schemas"
EXAMPLES_DIR = SCHEMAS_DIR / "examples"
CIR_SCHEMA_PATH = SCHEMAS_DIR / "cir.schema.json"
CIR_EXAMPLE_PATH = EXAMPLES_DIR / "minimal.cir.json"
DOMAIN_PACK_SCHEMA_PATH = SCHEMAS_DIR / "domain-pack.schema.json"
DOMAIN_PACK_EXAMPLE_PATH = EXAMPLES_DIR / "storage-pack.manifest.json"
KERNEL_WIRE_PATHS = {
    "Core": ROOT / "crates/continuum-kernel-core/src/wire.rs",
    "Sat": ROOT / "crates/continuum-kernel-sat/src/wire.rs",
    "Smt": ROOT / "crates/continuum-kernel-smt/src/wire.rs",
    "Temporal": ROOT / "crates/continuum-kernel-temporal/src/wire.rs",
}
RFC_0006_PATH = ROOT / "notes/plan/rfcs/0006-production-trace-conformance.md"

OBLIGATIONS = {
    "TEST-6-01": "parser and canonical encodings",
    "TEST-6-02": "CIR validator",
    "TEST-6-03": "certificate formats",
    "TEST-6-04": "domain-pack commands/faults",
    "TEST-6-05": "trace importers",
    "TEST-6-06": "solver proof parsers",
}
RULES: dict[str, str] = {
    "schema-id-grammar-detected": "TEST-6-01",
    "instance-header-mismatch-detected": "TEST-6-01",
    "canonical-encoding-not-value-identical-detected": "TEST-6-01",
    "cir-schema-violation-detected": "TEST-6-02",
    "certificate-routing-mismatch-detected": "TEST-6-03",
    "domain-pack-schema-violation-detected": "TEST-6-04",
    "trace-fabricated-order-detected": "TEST-6-05",
    "trace-outcome-not-typed-detected": "TEST-6-05",
    "lrat-clause-literals-not-ascending-detected": "TEST-6-06",
    "lrat-deletion-ids-not-ascending-detected": "TEST-6-06",
    "lrat-clause-id-not-increasing-detected": "TEST-6-06",
    "lrat-reference-unknown-clause-detected": "TEST-6-06",
}

_BOUNDARY_01 = (
    "This module's schema-id-grammar-detected rule is independently corroborated by a "
    "real, non-#[ignore]d Rust test: crates/continuumd/tests/gate_g2_01_acceptance.rs's "
    "every_shipped_schema_document_declares_the_readme_identity_triple() asserts, over "
    "the same real notes/plan/schemas/*.schema.json files, that each one declares the "
    "exact $id/schema_epoch/schema_kind triple this module also checks. The canonical- "
    "encoding half of the concept is independently corroborated by continuum-value's "
    "own canonical-content-identity test suite (crates/continuum-value/src/identity.rs, "
    "ADR-0013). Two rules this module also runs — instance-header-mismatch-detected and "
    "canonical-encoding-not-value-identical-detected — have no equivalent named Rust "
    "test: notes/plan/schemas/README.md itself assigns instance-level validation to "
    "validate_dossier.py (a separate Python tool using the real jsonschema library), not "
    "to Rust, and the byte-identity-vs-canonical re-encoding check is a freestanding "
    "property demonstration over real example content, not a port of any Rust logic."
)
_ABSENT_02 = (
    "continuum-cir/src/lib.rs (crates/continuum-cir/src/lib.rs) is a 20-line documented "
    "stub: no Rust code anywhere validates a CIR document's structure. The one related "
    "real, non-#[ignore]d Rust test is crates/continuum-value/src/value.rs's "
    "value_kinds_are_the_cir_schema_value_kinds(), which checks that Value's own "
    "ValueKind enum matches cir.schema.json's $defs.value tagged-kind vocabulary — real "
    "corroboration of one enum, not of required fields, patterns, event structure, or "
    "obligation shape, which is what this module's cir-schema-violation-detected rule "
    "actually enforces (via cir.schema.json directly, INV-003). The schema remains the "
    "normative source under INV-003, but per the coordinator's rule that a Python check "
    "of a real artifact still needs a corresponding real-implementation test to be "
    "claimed delivered, and no such test exists for CIR structural validity, this ID is "
    "partial."
)
_BOUNDARY_03 = (
    "crates/continuum-certificate/src/family.rs's Family::route dispatches on eight "
    "magic bytes read straight from each continuum-kernel-* crate's own wire::MAGIC "
    "(family.rs: 'The constants below are therefore taken from the kernels rather "
    "than restated'). This module extracts those four MAGIC literals from the real, "
    "committed crates/continuum-kernel-{core,sat,smt,temporal}/src/wire.rs at run "
    "time (regex over the source text, not a hand-copied constant — the drift tie), "
    "and re-implements route()'s documented algorithm (read <=8 bytes, compare in "
    "Core/Sat/Smt/Temporal declaration order, TooShortForMagic below length, "
    "UnknownFamily otherwise) to check it against adversarial byte strings. Four real, "
    "non-#[ignore]d Rust tests in crates/continuum-certificate/tests/family_routing.rs "
    "exercise the real Family::route/check_certificate composition over exactly these "
    "cases (partition, too-short, unclaimed magic, wrong-family magic) — named and "
    "existence-checked below. What this module cannot do is call Family::route itself "
    "(no cargo), so a change to the routing algorithm's *shape* (not just its magic "
    "values) would not be caught here, and the four kernels' own wire-body decoders "
    "(LRAT/Alethe/ranking-witness bytes past the first eight bytes) are exercised only "
    "for the LRAT case, by TEST-6-06."
)
_ABSENT_04 = (
    "notes/plan/schemas/domain-pack.schema.json + schemas/examples/storage-pack."
    "manifest.json are real, committed artifacts, and this module's "
    "domain-pack-schema-violation-detected rule validates real content against the real "
    "schema. But the only related Rust test is "
    "crates/continuumd/tests/inv003_no_prose_only_evidence.rs's "
    "every_schema_file_is_claimed_by_exactly_one_table_row_and_vice_versa(), which "
    "checks only that the schema *file* is inventoried in a hand-maintained table — it "
    "never reads a manifest's operations or faults. continuum-pack-storage, the adapter "
    "crate the manifest itself names, does not exist anywhere under crates/. There is no "
    "real Rust code that validates domain-pack commands or faults content, so this ID is "
    "partial."
)
_ABSENT_05 = (
    "notes/plan/rfcs/0006-production-trace-conformance.md is a real, committed RFC "
    "with a typed outcome vocabulary (Conforms/Violates/Inconclusive/Unsupported/"
    "ResourceExhausted) and an explicit rule ('MUST NOT invent a total event order "
    "that telemetry does not establish'), but continuum-corpus and "
    "continuum-observer (crates/continuum-corpus/src/lib.rs, crates/"
    "continuum-observer/src/lib.rs) are both acknowledged ~20-line stubs, and no "
    "schema for a trace-import artifact exists under notes/plan/schemas/. No Rust test "
    "anywhere references trace import. The check below models the RFC's own vocabulary "
    "and no-fabrication rule as a standalone reference importer; it is not a test of any "
    "production import layer, because none has landed yet."
)
_BOUNDARY_06 = (
    "crates/continuum-kernel-sat/src/wire.rs's own doc comment gives the LRAT body "
    "grammar as an explicit BNF plus a 'Canonicity' section naming exactly which "
    "sequences must be strictly-ascending sets (clause literals, ordered by (variable, "
    "sign); a deletion step's id list) and which are meaning-bearing sequences left "
    "unsorted (the antecedent hint chain), plus the LRAT convention that an added "
    "clause's own id must exceed every id already in use. This module re-implements "
    "exactly those rules and runs them against adversarial bodies — a from-scratch "
    "Python model, not an execution of the real 846-line decoder (no cargo) — but a "
    "run-time drift check (_check_lrat_grammar_drift) re-reads wire.rs and fails if the "
    "exact phrases this model is built from are no longer present. Six real, "
    "non-#[ignore]d Rust tests exercise the real decoder over the matching cases: "
    "wire.rs's own the_canonical_literal_order_is_by_variable_then_polarity, and "
    "check.rs's unordered_or_duplicated_literals_are_rejected, "
    "a_reused_or_descending_clause_identifier_is_rejected, "
    "an_unordered_deletion_list_is_rejected, "
    "an_antecedent_naming_an_undefined_clause_is_rejected, and "
    "deleting_an_undefined_clause_is_rejected — named and existence-checked below. "
    "continuum-kernel-smt's Alethe grammar is not modeled at all."
)
BOUNDARIES: dict[str, list[str]] = {
    "TEST-6-01": [_BOUNDARY_01],
    "TEST-6-02": [_ABSENT_02],
    "TEST-6-03": [_BOUNDARY_03],
    "TEST-6-04": [_ABSENT_04],
    "TEST-6-05": [_ABSENT_05],
    "TEST-6-06": [_BOUNDARY_06],
}
ABSENCE: dict[str, str] = {
    "TEST-6-02": "continuum-cir is a 20-line stub; no Rust code validates CIR structure beyond one enum's vocabulary parity; see BOUNDARIES",
    "TEST-6-04": "no Rust code validates domain-pack commands/faults content; continuum-pack-storage does not exist under crates/; see BOUNDARIES",
    "TEST-6-05": "no crate or schema implements RFC 0006's import layer yet; see BOUNDARIES",
}
STATUS: dict[str, str] = {
    "TEST-6-01": "enforced",
    "TEST-6-02": "partial",
    "TEST-6-03": "enforced",
    "TEST-6-04": "partial",
    "TEST-6-05": "partial",
    "TEST-6-06": "enforced",
}

# ---------------------------------------------------------------------------
# Real Rust #[test] evidence. A Python re-implementation of a Rust predicate is
# not, by itself, evidence about the Rust code (coordinator directive on
# bn-1zb9). For every ID claimed `enforced` above, this names specific real
# tests and checks — textually, no cargo, no compiling — that each one still
# exists as `fn <name>(` immediately preceded by a `#[test]` attribute run
# containing no `#[ignore]`. A missing or disabled test is a real_run()
# failure, not just a stale docstring claim.
# ---------------------------------------------------------------------------

RUST_TESTS: dict[str, list[tuple[Path, str]]] = {
    "TEST-6-01": [
        (ROOT / "crates/continuumd/tests/gate_g2_01_acceptance.rs", "every_shipped_schema_document_declares_the_readme_identity_triple"),
        (ROOT / "crates/continuum-value/src/identity.rs", "a_certified_identity_is_the_canonical_encoding"),
        (ROOT / "crates/continuum-value/src/identity.rs", "non_canonical_bytes_are_not_an_identity"),
    ],
    "TEST-6-03": [
        (ROOT / "crates/continuum-certificate/tests/family_routing.rs", "the_routing_table_is_the_kernels_own_magics_and_they_partition_the_input_space"),
        (ROOT / "crates/continuum-certificate/tests/family_routing.rs", "bytes_too_short_to_carry_a_magic_reach_no_checker"),
        (ROOT / "crates/continuum-certificate/tests/family_routing.rs", "an_unclaimed_magic_is_a_routing_fault_carrying_the_bytes_that_arrived"),
        (ROOT / "crates/continuum-certificate/tests/family_routing.rs", "a_certificate_stamped_with_another_familys_magic_is_refused_by_the_kernel_it_names"),
    ],
    "TEST-6-06": [
        (ROOT / "crates/continuum-kernel-sat/src/wire.rs", "the_canonical_literal_order_is_by_variable_then_polarity"),
        (ROOT / "crates/continuum-kernel-sat/src/check.rs", "unordered_or_duplicated_literals_are_rejected"),
        (ROOT / "crates/continuum-kernel-sat/src/check.rs", "a_reused_or_descending_clause_identifier_is_rejected"),
        (ROOT / "crates/continuum-kernel-sat/src/check.rs", "an_unordered_deletion_list_is_rejected"),
        (ROOT / "crates/continuum-kernel-sat/src/check.rs", "an_antecedent_naming_an_undefined_clause_is_rejected"),
        (ROOT / "crates/continuum-kernel-sat/src/check.rs", "deleting_an_undefined_clause_is_rejected"),
    ],
}


def _rust_test_present(path: Path, fn_name: str) -> tuple[bool, str]:
    """Textual, no-cargo check: `fn <fn_name>(` exists in `path`, immediately
    preceded (skipping only blank lines and `//`/`///` comments) by an
    attribute run that includes `#[test]` and no `#[ignore]`."""
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


_LRAT_GRAMMAR_PHRASES = [
    "ordered by variable, and within a variable the negative literal precedes the",
    "the identifiers of a deletion step",
    "must exceed every identifier already in use",
]


def _check_lrat_grammar_drift() -> list[str]:
    """Drift tie for TEST-6-06's Python model: the exact wire.rs doc-comment
    phrases the LRAT canonicity model is built from must still be present."""
    try:
        text = KERNEL_WIRE_PATHS["Sat"].read_text(encoding="utf-8")
    except OSError as exc:
        return [f"could not read {KERNEL_WIRE_PATHS['Sat']}: {exc}"]
    return [
        f"crates/continuum-kernel-sat/src/wire.rs no longer states {phrase!r}: "
        "this module's LRAT canonicity model may have drifted from the real grammar"
        for phrase in _LRAT_GRAMMAR_PHRASES
        if phrase not in text
    ]


def _finding(rule: str, subject: str, message: str) -> dict[str, str]:
    return tsys.Finding(rule, RULES[rule], subject, message).as_json()


# ---------------------------------------------------------------------------
# Shared: a minimal JSON Schema (Draft 2020-12 subset) evaluator.
#
# Supports exactly the keywords notes/plan/schemas/cir.schema.json and
# domain-pack.schema.json use: type, required, properties,
# additionalProperties (boolean), enum, const, pattern, items, uniqueItems,
# minItems, minLength, minimum, $ref (local "#/$defs/<name>" only), oneOf,
# and the empty schema `{}` (always valid). It is deliberately not a general
# Draft 2020-12 implementation (no jsonschema dependency: README-test.md,
# "Python 3 stdlib only").
# ---------------------------------------------------------------------------


def _schema_errors(schema: Any, instance: Any, defs: dict, path: str) -> list[str]:
    errors: list[str] = []
    if schema == {}:
        return errors
    if "$ref" in schema:
        ref = schema["$ref"]
        if not ref.startswith("#/$defs/") or ref[len("#/$defs/"):] not in defs:
            return [f"{path}: unresolvable $ref {ref!r}"]
        return _schema_errors(defs[ref[len("#/$defs/"):]], instance, defs, path)
    if "oneOf" in schema:
        branches = [_schema_errors(s, instance, defs, path) for s in schema["oneOf"]]
        matched = [e for e in branches if not e]
        if len(matched) != 1:
            errors.append(f"{path}: oneOf matched {len(matched)} of {len(schema['oneOf'])} branches (want exactly 1)")
        return errors
    if "const" in schema and instance != schema["const"]:
        errors.append(f"{path}: expected const {schema['const']!r}, got {instance!r}")
    if "enum" in schema and instance not in schema["enum"]:
        errors.append(f"{path}: {instance!r} not in enum {schema['enum']!r}")
    t = schema.get("type")
    if t is not None:
        ok = {
            "object": isinstance(instance, dict),
            "array": isinstance(instance, list),
            "string": isinstance(instance, str),
            "integer": isinstance(instance, int) and not isinstance(instance, bool),
            "boolean": isinstance(instance, bool),
            "null": instance is None,
        }.get(t)
        if ok is None:
            errors.append(f"{path}: schema names unsupported type {t!r}")
        elif not ok:
            errors.append(f"{path}: expected type {t}, got {type(instance).__name__}")
            return errors  # further keyword checks are meaningless on the wrong shape
    if isinstance(instance, str):
        if "minLength" in schema and len(instance) < schema["minLength"]:
            errors.append(f"{path}: length {len(instance)} < minLength {schema['minLength']}")
        if "pattern" in schema and re.search(schema["pattern"], instance) is None:
            errors.append(f"{path}: {instance!r} does not match pattern {schema['pattern']!r}")
    if isinstance(instance, int) and not isinstance(instance, bool):
        if "minimum" in schema and instance < schema["minimum"]:
            errors.append(f"{path}: {instance} < minimum {schema['minimum']}")
    if isinstance(instance, list):
        if "minItems" in schema and len(instance) < schema["minItems"]:
            errors.append(f"{path}: {len(instance)} items < minItems {schema['minItems']}")
        if schema.get("uniqueItems"):
            seen: list[str] = []
            for item in instance:
                key = json.dumps(item, sort_keys=True)
                if key in seen:
                    errors.append(f"{path}: duplicate item {item!r} violates uniqueItems")
                seen.append(key)
        if "items" in schema:
            for i, item in enumerate(instance):
                errors.extend(_schema_errors(schema["items"], item, defs, f"{path}[{i}]"))
    if isinstance(instance, dict):
        for key in schema.get("required", []):
            if key not in instance:
                errors.append(f"{path}: missing required property {key!r}")
        props = schema.get("properties", {})
        for key, value in instance.items():
            if key in props:
                errors.extend(_schema_errors(props[key], value, defs, f"{path}.{key}"))
            elif schema.get("additionalProperties") is False:
                errors.append(f"{path}: unexpected property {key!r} (additionalProperties: false)")
    return errors


def _validate(schema_doc: dict, instance: Any) -> list[str]:
    return _schema_errors(schema_doc, instance, schema_doc.get("$defs", {}), "$")


def _load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


# ---------------------------------------------------------------------------
# TEST-6-01: parser and canonical encodings.
# ---------------------------------------------------------------------------

_SCHEMA_ID_RE = re.compile(r"^https://continuum\.dev/schema/v(\d+)/([a-z0-9][a-z0-9-]*)\.json$")


def _check_schema_doc_identity(doc: dict, subject: str) -> list[dict[str, str]]:
    doc_id = doc.get("$id")
    m = _SCHEMA_ID_RE.match(doc_id) if isinstance(doc_id, str) else None
    if not m:
        return [
            _finding(
                "schema-id-grammar-detected",
                subject,
                f"$id {doc_id!r} does not match https://continuum.dev/schema/v<epoch>/<name>.json (schemas/README.md)",
            )
        ]
    epoch, name = int(m.group(1)), m.group(2)
    out: list[dict[str, str]] = []
    if doc.get("schema_epoch") != epoch:
        out.append(
            _finding(
                "schema-id-grammar-detected",
                subject,
                f"schema_epoch {doc.get('schema_epoch')!r} != the v{epoch} segment of its own $id",
            )
        )
    if doc.get("schema_kind") == "artifact":
        class_id = f"https://continuum.dev/schema/{name}.json"
        required = doc.get("required", [])
        props = doc.get("properties", {})
        if "schema_id" not in required or props.get("schema_id", {}).get("const") != class_id:
            out.append(
                _finding(
                    "schema-id-grammar-detected",
                    subject,
                    f"schema_kind is 'artifact' but schema_id is not required and const-pinned to the class identity {class_id!r}",
                )
            )
        if "schema_epoch" not in required or props.get("schema_epoch", {}).get("const") != epoch:
            out.append(
                _finding(
                    "schema-id-grammar-detected",
                    subject,
                    f"schema_kind is 'artifact' but schema_epoch is not required and const-pinned to {epoch}",
                )
            )
    return out


def _class_id_of(doc: dict) -> str | None:
    doc_id = doc.get("$id")
    m = _SCHEMA_ID_RE.match(doc_id) if isinstance(doc_id, str) else None
    if not m:
        return None
    return f"https://continuum.dev/schema/{m.group(2)}.json"


def _check_instance_header(doc: dict, instance: Any, subject: str) -> list[dict[str, str]]:
    if doc.get("schema_kind") != "artifact" or not isinstance(instance, dict):
        return []
    class_id = _class_id_of(doc)
    if class_id is None:
        return []
    m = _SCHEMA_ID_RE.match(doc["$id"])
    epoch = int(m.group(1)) if m else None
    out: list[dict[str, str]] = []
    if instance.get("schema_id") != class_id:
        out.append(
            _finding(
                "instance-header-mismatch-detected",
                subject,
                f"instance schema_id {instance.get('schema_id')!r} != governing class identity {class_id!r}",
            )
        )
    if instance.get("schema_epoch") != epoch:
        out.append(
            _finding(
                "instance-header-mismatch-detected",
                subject,
                f"instance schema_epoch {instance.get('schema_epoch')!r} != governing schema_epoch {epoch!r}",
            )
        )
    return out


def _canonicalize(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def _check_canonical_encoding(raw_a: str, raw_b: str, policy: str, subject: str) -> list[dict[str, str]]:
    try:
        value_a, value_b = json.loads(raw_a), json.loads(raw_b)
    except json.JSONDecodeError as exc:
        return [_finding("canonical-encoding-not-value-identical-detected", subject, f"fixture JSON did not parse: {exc}")]
    if value_a != value_b:
        return []  # not a same-content pair; nothing to compare
    if policy == "canonical":
        identity_a, identity_b = _canonicalize(value_a), _canonicalize(value_b)
    elif policy == "byte-identity":
        identity_a, identity_b = raw_a, raw_b
    else:
        return [_finding("canonical-encoding-not-value-identical-detected", subject, f"unknown policy {policy!r}")]
    if identity_a != identity_b:
        return [
            _finding(
                "canonical-encoding-not-value-identical-detected",
                subject,
                f"policy {policy!r}: two byte-different encodings of equal parsed content produced different "
                "identities (ADR-0013 content identity requires the same identity for the same content)",
            )
        ]
    return []


def _reordered_json(raw: str) -> str:
    """A structurally different re-serialization of the same parsed value: reversed
    top-level key order and 4-space indentation instead of the file's own
    formatting. Same parsed value, different raw bytes — exactly the pair
    ADR-0013 content identity must treat as one identity."""
    value = json.loads(raw)
    if isinstance(value, dict):
        value = dict(reversed(list(value.items())))
    return json.dumps(value, indent=4, sort_keys=False)


def _iter_real_schema_docs() -> list[tuple[str, dict]]:
    out = []
    for path in sorted(SCHEMAS_DIR.glob("*.schema.json")):
        out.append((f"notes/plan/schemas/{path.name}", _load_json(path)))
    return out


def _iter_real_examples() -> list[tuple[str, Any]]:
    out = []
    for path in sorted(EXAMPLES_DIR.glob("*.json")):
        out.append((f"notes/plan/schemas/examples/{path.name}", _load_json(path)))
    return out


# ---------------------------------------------------------------------------
# TEST-6-02: CIR validator.
# ---------------------------------------------------------------------------


def _check_cir_instance(instance: Any) -> list[dict[str, str]]:
    schema = _load_json(CIR_SCHEMA_PATH)
    return [
        _finding("cir-schema-violation-detected", "$", err)
        for err in _validate(schema, instance)
    ]


_CIR_MUTATIONS: list[tuple[str, Any]] = []  # filled in real_run from the real golden


def _mutate_cir(golden: dict, kind: str) -> Any:
    import copy as _copy

    m = _copy.deepcopy(golden)
    if kind == "missing-roots":
        del m["roots"]
    elif kind == "bad-hash-pattern":
        m["model_hash"] = "not-a-hash"
    elif kind == "unknown-property":
        m["not_a_real_field"] = True
    elif kind == "bad-event-kind":
        m["events"][0]["label"]["payload"] = {"kind": "not-a-real-kind", "value": 1}
    elif kind == "negative-epoch":
        m["events"][0]["epoch"] = -1
    elif kind == "bad-event-id-pattern":
        m["events"][0]["id"] = "not-an-event-id"
    else:
        raise ValueError(kind)
    return m


# ---------------------------------------------------------------------------
# TEST-6-03: certificate formats (magic-byte routing).
# ---------------------------------------------------------------------------

_MAGIC_RE = re.compile(r'pub const MAGIC: \[u8; 8\] = \*b"([^"]{8})";')
_FAMILY_ORDER = ("Core", "Sat", "Smt", "Temporal")  # Family::ALL's declaration order


def _extract_kernel_magics() -> list[tuple[str, bytes]]:
    out = []
    for family in _FAMILY_ORDER:
        text = KERNEL_WIRE_PATHS[family].read_text(encoding="utf-8")
        m = _MAGIC_RE.search(text)
        if not m:
            raise ValueError(f"no MAGIC constant found in {KERNEL_WIRE_PATHS[family]}")
        out.append((family, m.group(1).encode("ascii")))
    return out


def _route(data: bytes, magics: list[tuple[str, bytes]]) -> dict[str, Any]:
    """Mirrors crates/continuum-certificate/src/family.rs's Family::route: read at
    most 8 bytes, compare against the real magics in declaration order, else a
    typed routing fault. Consumes nothing beyond the first 8 bytes."""
    if len(data) < 8:
        return {"outcome": "too_short", "available": len(data)}
    head = data[:8]
    for family, magic in magics:
        if head == magic:
            return {"outcome": "routed", "family": family}
    return {"outcome": "unknown", "magic": head.hex()}


def _check_certificate_routing(payload: dict) -> list[dict[str, str]]:
    magics = _extract_kernel_magics()
    data = bytes.fromhex(payload["bytes_hex"])
    actual = _route(data, magics)
    claimed = payload["claimed"]
    if actual != claimed:
        return [
            _finding(
                "certificate-routing-mismatch-detected",
                payload["bytes_hex"][:16],
                f"claimed routing outcome {claimed!r} does not match the real Family::route "
                f"contract over the extracted kernel magics: actual {actual!r}",
            )
        ]
    return []


# ---------------------------------------------------------------------------
# TEST-6-04: domain-pack commands/faults.
# ---------------------------------------------------------------------------


def _check_domain_pack_instance(instance: Any) -> list[dict[str, str]]:
    schema = _load_json(DOMAIN_PACK_SCHEMA_PATH)
    return [
        _finding("domain-pack-schema-violation-detected", "$", err)
        for err in _validate(schema, instance)
    ]


def _mutate_domain_pack(golden: dict, kind: str) -> Any:
    import copy as _copy

    m = _copy.deepcopy(golden)
    if kind == "fault-not-string":
        m["profiles"][0]["faults"] = [1, 2, 3]
    elif kind == "bad-profile-class":
        m["profiles"][0]["class"] = "not-a-real-class"
    elif kind == "operation-missing-effect-phases":
        del m["operations"][0]["effect_phases"]
    elif kind == "unknown-top-level-field":
        m["not_a_real_field"] = True
    elif kind == "operation-command-type-not-string":
        m["operations"][0]["command_type"] = 42
    elif kind == "missing-pack-name":
        del m["pack"]["name"]
    else:
        raise ValueError(kind)
    return m


# ---------------------------------------------------------------------------
# TEST-6-05: trace importers (RFC 0006 model).
# ---------------------------------------------------------------------------

_OUTCOMES = {"Conforms", "Violates", "Inconclusive", "Unsupported", "ResourceExhausted"}


def _evidenced_order(a: dict, b: dict) -> tuple[str, str] | None:
    """The pair (earlier_id, later_id) if the two observations carry evidence
    that orders them (a shared clock or sequence field with differing values),
    else None — RFC 0006's observation-constraint model, standalone (no
    landed import crate exists to enforce this against; see ABSENCE)."""
    if a.get("clock") is not None and b.get("clock") is not None and a["clock"] != b["clock"]:
        return (a["id"], b["id"]) if a["clock"] < b["clock"] else (b["id"], a["id"])
    if a.get("seq") is not None and b.get("seq") is not None and a["seq"] != b["seq"]:
        return (a["id"], b["id"]) if a["seq"] < b["seq"] else (b["id"], a["id"])
    return None


def _reference_outcome(observations: dict[str, dict], claimed_order: list[str]) -> str:
    for a_id, b_id in zip(claimed_order, claimed_order[1:]):
        a, b = observations[a_id], observations[b_id]
        ev = _evidenced_order(a, b)
        if ev is None:
            return "Inconclusive"
        if ev != (a_id, b_id):
            return "Violates"
    return "Conforms"


def _check_trace_import(payload: dict) -> list[dict[str, str]]:
    outcome = payload["outcome"]
    subject = ",".join(payload["claimed_order"])
    out: list[dict[str, str]] = []
    if outcome not in _OUTCOMES:
        out.append(
            _finding(
                "trace-outcome-not-typed-detected",
                subject,
                f"outcome {outcome!r} is not one of RFC 0006's typed outcomes {sorted(_OUTCOMES)}",
            )
        )
        return out
    observations = {oid: {**o, "id": oid} for oid, o in payload["observations"].items()}
    reference = _reference_outcome(observations, payload["claimed_order"])
    if reference == "Inconclusive" and outcome in ("Conforms", "Violates"):
        out.append(
            _finding(
                "trace-fabricated-order-detected",
                subject,
                f"claimed outcome {outcome!r} asserts an order the observations carry no clock/seq evidence for "
                "(RFC 0006: 'MUST NOT invent a total event order that telemetry does not establish')",
            )
        )
    return out


def _gen_trace_scenario(seed: int) -> dict:
    n = 2 + seed % 3
    ids = [f"o{i}" for i in range(n)]
    has_evidence = seed % 2 == 0
    observations = {}
    for i, oid in enumerate(ids):
        observations[oid] = {"id": oid, "clock": i if has_evidence else None}
    return {"observations": observations, "claimed_order": ids, "has_evidence": has_evidence}


# ---------------------------------------------------------------------------
# TEST-6-06: solver proof parsers (LRAT body canonicity, cited from wire.rs).
# ---------------------------------------------------------------------------


def _lrat_canon_key(lit: int) -> tuple[int, int]:
    return (abs(lit), 0 if lit < 0 else 1)


def _lrat_make_clause(vars_signs: list[tuple[int, bool]]) -> list[int]:
    lits = [-v if neg else v for v, neg in vars_signs]
    return sorted(set(lits), key=_lrat_canon_key)


def _check_lrat_body(payload: dict) -> list[dict[str, str]]:
    vc = payload["variable_count"]
    formula = payload["formula"]
    steps = payload["steps"]
    out: list[dict[str, str]] = []

    def check_clause(lits: list[int], subject: str) -> None:
        bounded = all(l != 0 and abs(l) <= vc for l in lits)
        keys = [_lrat_canon_key(l) for l in lits]
        ascending = all(keys[i] < keys[i + 1] for i in range(len(keys) - 1))
        if not (bounded and ascending):
            out.append(
                _finding(
                    "lrat-clause-literals-not-ascending-detected",
                    subject,
                    f"clause {lits} is not a strictly-ascending (variable, sign) set within [1, {vc}] "
                    "(wire.rs 'Canonicity': literals are ordered by variable, negative before positive)",
                )
            )

    known_ids: set[int] = set()
    max_id = 0
    for i, clause in enumerate(formula, start=1):
        check_clause(clause, f"formula clause {i}")
        known_ids.add(i)
        max_id = i

    for si, step in enumerate(steps):
        if step["kind"] == "add":
            cid = step["id"]
            if cid <= max_id:
                out.append(
                    _finding(
                        "lrat-clause-id-not-increasing-detected",
                        f"step {si}",
                        f"add step declares id {cid}, which does not exceed the highest id already in use "
                        f"({max_id}) — LRAT's own convention (wire.rs 'Clause identifiers')",
                    )
                )
            check_clause(step["clause"], f"step {si} add clause")
            for h in step["hints"]:
                if h not in known_ids:
                    out.append(
                        _finding(
                            "lrat-reference-unknown-clause-detected",
                            f"step {si}",
                            f"hint {h} names a clause id not yet declared",
                        )
                    )
            known_ids.add(cid)
            max_id = max(max_id, cid)
        elif step["kind"] == "delete":
            ids = step["ids"]
            if not all(ids[i] < ids[i + 1] for i in range(len(ids) - 1)):
                out.append(
                    _finding(
                        "lrat-deletion-ids-not-ascending-detected",
                        f"step {si}",
                        f"deletion id list {ids} is not a strictly-ascending set (wire.rs 'Canonicity': "
                        "'the identifiers of a deletion step' must be strictly ascending)",
                    )
                )
            for d in ids:
                if d not in known_ids:
                    out.append(
                        _finding(
                            "lrat-reference-unknown-clause-detected",
                            f"step {si}",
                            f"deletion names clause id {d}, which was never declared",
                        )
                    )
        else:
            out.append(_finding("lrat-clause-literals-not-ascending-detected", f"step {si}", f"unknown step kind {step['kind']!r}"))
    return out


def _gen_lrat_body(seed: int) -> dict:
    vc = 2 + seed % 5
    nclauses = 1 + seed % 3
    formula = []
    for i in range(nclauses):
        nvars = 1 + (seed + i) % 3
        vars_ = sorted({((seed * 13 + i * 7 + k * 5) % vc) + 1 for k in range(nvars)})
        vs = [(v, (seed + i + v) % 2 == 0) for v in vars_]
        formula.append(_lrat_make_clause(vs))
    add_id = nclauses + 1
    nvars2 = 1 + seed % 2
    vars2 = sorted({((seed * 5 + k * 3) % vc) + 1 for k in range(nvars2)})
    add_clause = _lrat_make_clause([(v, (seed + v) % 2 == 0) for v in vars2])
    steps: list[dict] = [{"kind": "add", "id": add_id, "clause": add_clause, "hints": list(range(1, nclauses + 1))}]
    if nclauses >= 2:
        steps.append({"kind": "delete", "ids": sorted([1, 2])})
    return {"variable_count": vc, "formula": formula, "steps": steps}


# ---------------------------------------------------------------------------
# Fixtures dispatch.
# ---------------------------------------------------------------------------


def check_fixture(system: Any) -> list[dict[str, str]]:
    if not isinstance(system, dict) or "kind" not in system:
        return [_finding("schema-id-grammar-detected", "<payload>", "payload must be an object with a 'kind'")]
    kind = system["kind"]
    payload = {k: v for k, v in system.items() if k != "kind"}
    if kind == "schema-doc":
        return _check_schema_doc_identity(payload["doc"], payload.get("subject", "<fixture>"))
    if kind == "instance-header":
        return _check_instance_header(payload["doc"], payload["instance"], payload.get("subject", "<fixture>"))
    if kind == "canonical-encoding":
        return _check_canonical_encoding(payload["raw_a"], payload["raw_b"], payload["policy"], payload.get("subject", "<fixture>"))
    if kind == "cir":
        return _check_cir_instance(payload["instance"])
    if kind == "certificate-routing":
        return _check_certificate_routing(payload)
    if kind == "domain-pack":
        return _check_domain_pack_instance(payload["instance"])
    if kind == "trace-import":
        return _check_trace_import(payload)
    if kind == "lrat-body":
        return _check_lrat_body(payload)
    return [_finding("schema-id-grammar-detected", "<payload>", f"unknown fixture kind {kind!r}")]


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

    # -- TEST-6-01: parser and canonical encodings -----------------------
    try:
        schema_docs = _iter_real_schema_docs()
    except OSError as exc:
        failures["TEST-6-01"].append(f"could not read notes/plan/schemas/*.schema.json: {exc}")
        schema_docs = []
    bump("TEST-6-01", "schema_documents", len(schema_docs))
    class_ids: dict[str, dict] = {}
    for subject, doc in schema_docs:
        errs = _check_schema_doc_identity(doc, subject)
        if errs:
            failures["TEST-6-01"].append(f"{subject}: real schema document failed its own identity grammar: {errs[0]['message']}")
        cid = _class_id_of(doc)
        if cid is not None:
            class_ids[cid] = doc

    try:
        examples = _iter_real_examples()
    except OSError as exc:
        failures["TEST-6-01"].append(f"could not read notes/plan/schemas/examples/*.json: {exc}")
        examples = []
    matched = 0
    for subject, instance in examples:
        if not isinstance(instance, dict) or instance.get("schema_id") not in class_ids:
            continue  # schema_kind "value" instances (e.g. redacted.example.json) carry no header
        matched += 1
        doc = class_ids[instance["schema_id"]]
        errs = _check_instance_header(doc, instance, subject)
        if errs:
            failures["TEST-6-01"].append(f"{subject}: real example instance failed its own header check: {errs[0]['message']}")
        # Canonical re-encoding: a structurally different but value-equal
        # re-serialization of this real example must canonicalize identically.
        raw = (EXAMPLES_DIR / Path(subject).name).read_text(encoding="utf-8")
        reordered = _reordered_json(raw)
        clean = _check_canonical_encoding(raw, reordered, "canonical", subject)
        if clean:
            failures["TEST-6-01"].append(f"{subject}: the canonical policy itself flagged a value-equal re-encoding: {clean[0]['message']}")
        bump("TEST-6-01", "examples_re_encoded")
        mut = _check_canonical_encoding(raw, reordered, "byte-identity", subject)
        mutate("TEST-6-01", True, bool(mut), f"{subject} (byte-identity policy)")
    bump("TEST-6-01", "examples_matched_to_a_schema", matched)

    def need(rid: str, key: str, what: str) -> None:
        if corpus[rid].get(key, 0) == 0:
            failures[rid].append(f"corpus exercises no {what} ({key} = 0): the pass would be vacuous")

    need("TEST-6-01", "schema_documents", "real schema document under notes/plan/schemas/")
    need("TEST-6-01", "examples_matched_to_a_schema", "real example instance matched to its governing schema")

    # -- TEST-6-02: CIR validator -----------------------------------------
    if not CIR_SCHEMA_PATH.exists() or not CIR_EXAMPLE_PATH.exists():
        failures["TEST-6-02"].append("notes/plan/schemas/cir.schema.json or examples/minimal.cir.json is missing")
    else:
        golden = _load_json(CIR_EXAMPLE_PATH)
        clean = _check_cir_instance(golden)
        if clean:
            failures["TEST-6-02"].append(f"the real minimal.cir.json golden was itself flagged: {clean[0]['message']}")
        bump("TEST-6-02", "golden_checked")
        for kind in ("missing-roots", "bad-hash-pattern", "unknown-property", "bad-event-kind", "negative-epoch", "bad-event-id-pattern"):
            mutant = _mutate_cir(golden, kind)
            found = _check_cir_instance(mutant)
            mutate("TEST-6-02", True, bool(found), f"minimal.cir.json ({kind})")
            bump("TEST-6-02", "mutations")

    # -- TEST-6-03: certificate formats (routing) --------------------------
    try:
        magics = _extract_kernel_magics()
    except (OSError, ValueError) as exc:
        failures["TEST-6-03"].append(f"could not extract real kernel MAGIC constants: {exc}")
        magics = []
    if magics:
        for family, magic in magics:
            bump("TEST-6-03", "families_routed")
            for suffix in (b"", b"\x00\x00\x00", b"\xff" * 40):
                actual = _route(magic + suffix, magics)
                if actual != {"outcome": "routed", "family": family}:
                    failures["TEST-6-03"].append(f"{family}: real magic + trailing bytes {suffix!r} did not route to {family}: {actual}")
                mut = _check_certificate_routing({"bytes_hex": (magic + suffix).hex(), "claimed": {"outcome": "routed", "family": "Core" if family != "Core" else "Sat"}})
                mutate("TEST-6-03", True, bool(mut), f"{family} claimed as a different family")
        for n in (0, 1, 7):
            bump("TEST-6-03", "too_short_inputs")
            actual = _route(b"\x00" * n, magics)
            if actual != {"outcome": "too_short", "available": n}:
                failures["TEST-6-03"].append(f"{n}-byte input did not route to too_short: {actual}")
        bump("TEST-6-03", "unknown_magic_inputs")
        actual = _route(b"NOTAREAL" + b"\x00" * 4, magics)
        if actual.get("outcome") != "unknown":
            failures["TEST-6-03"].append(f"a magic none of the four kernels declare did not route to unknown: {actual}")

    need("TEST-6-03", "families_routed", "real kernel magic routed to its own family")
    need("TEST-6-03", "too_short_inputs", "input too short to carry a magic")
    need("TEST-6-03", "unknown_magic_inputs", "input carrying no kernel's magic")

    # -- TEST-6-04: domain-pack commands/faults -----------------------------
    if not DOMAIN_PACK_SCHEMA_PATH.exists() or not DOMAIN_PACK_EXAMPLE_PATH.exists():
        failures["TEST-6-04"].append("notes/plan/schemas/domain-pack.schema.json or examples/storage-pack.manifest.json is missing")
    else:
        golden = _load_json(DOMAIN_PACK_EXAMPLE_PATH)
        clean = _check_domain_pack_instance(golden)
        if clean:
            failures["TEST-6-04"].append(f"the real storage-pack.manifest.json golden was itself flagged: {clean[0]['message']}")
        bump("TEST-6-04", "golden_checked")
        for kind in (
            "fault-not-string",
            "bad-profile-class",
            "operation-missing-effect-phases",
            "unknown-top-level-field",
            "operation-command-type-not-string",
            "missing-pack-name",
        ):
            mutant = _mutate_domain_pack(golden, kind)
            found = _check_domain_pack_instance(mutant)
            mutate("TEST-6-04", True, bool(found), f"storage-pack.manifest.json ({kind})")
            bump("TEST-6-04", "mutations")
            if kind in ("fault-not-string", "bad-profile-class"):
                bump("TEST-6-04", "fault_or_profile_mutations")
            if "operation" in kind:
                bump("TEST-6-04", "operation_mutations")

    need("TEST-6-04", "fault_or_profile_mutations", "mutated fault/profile declaration")
    need("TEST-6-04", "operation_mutations", "mutated operation (command) declaration")

    # -- TEST-6-05: trace importers -----------------------------------------
    if not RFC_0006_PATH.exists():
        failures["TEST-6-05"].append("notes/plan/rfcs/0006-production-trace-conformance.md is missing")
    for seed in range(30):
        scenario = _gen_trace_scenario(seed)
        bump("TEST-6-05", "scenarios")
        if scenario["has_evidence"]:
            bump("TEST-6-05", "fully_evidenced_scenarios")
        else:
            bump("TEST-6-05", "missing_evidence_scenarios")
        reference = _reference_outcome(scenario["observations"], scenario["claimed_order"])
        clean = _check_trace_import({"observations": scenario["observations"], "claimed_order": scenario["claimed_order"], "outcome": reference})
        if clean:
            failures["TEST-6-05"].append(f"seed {seed}: the reference (honest) outcome {reference!r} was itself flagged: {clean[0]['message']}")
        if not scenario["has_evidence"]:
            # A buggy importer that claims Conforms despite missing evidence.
            mut = _check_trace_import({"observations": scenario["observations"], "claimed_order": scenario["claimed_order"], "outcome": "Conforms"})
            mutate("TEST-6-05", True, bool(mut), f"seed {seed} (fabricated Conforms over missing evidence)")
    mut_typed = _check_trace_import({"observations": {"a": {"clock": 0}, "b": {"clock": 1}}, "claimed_order": ["a", "b"], "outcome": "success"})
    if not mut_typed:
        failures["TEST-6-05"].append("an untyped outcome ('success') was not flagged by trace-outcome-not-typed-detected")

    need("TEST-6-05", "fully_evidenced_scenarios", "scenario with full clock evidence")
    need("TEST-6-05", "missing_evidence_scenarios", "scenario with no ordering evidence")

    # -- TEST-6-06: solver proof parsers (LRAT canonicity) -------------------
    for seed in range(40):
        body = _gen_lrat_body(seed)
        bump("TEST-6-06", "bodies")
        if len(body["steps"]) > 1:
            bump("TEST-6-06", "bodies_with_a_delete_step")
        clean = _check_lrat_body(body)
        if clean:
            failures["TEST-6-06"].append(f"seed {seed}: the generator's own clean body was flagged: {clean[0]['message']}")

        import copy as _copy

        # Mutant: reverse a multi-literal clause's ascending order (skip
        # unit clauses, which are trivially ascending).
        multi = [i for i, c in enumerate(body["formula"]) if len(c) > 1]
        if multi:
            m = _copy.deepcopy(body)
            i = multi[0]
            m["formula"][i] = list(reversed(m["formula"][i]))
            mutate("TEST-6-06", True, bool(_check_lrat_body(m)), f"seed {seed} (clause {i} literal order reversed)")
            bump("TEST-6-06", "multi_literal_clauses")

        # Mutant: the add step's id no longer exceeds the max id in use.
        m2 = _copy.deepcopy(body)
        m2["steps"][0]["id"] = 1
        mutate("TEST-6-06", True, bool(_check_lrat_body(m2)), f"seed {seed} (add id not increasing)")

        # Mutant: a hint referencing an undeclared clause id.
        m3 = _copy.deepcopy(body)
        m3["steps"][0]["hints"] = [9999]
        mutate("TEST-6-06", True, bool(_check_lrat_body(m3)), f"seed {seed} (hint references unknown clause)")

        # Mutant: deletion id list order (only when a delete step exists).
        if len(body["steps"]) > 1:
            m4 = _copy.deepcopy(body)
            ids = m4["steps"][1]["ids"]
            if len(ids) > 1:
                m4["steps"][1]["ids"] = list(reversed(ids))
            else:
                m4["steps"][1]["ids"] = ids + [ids[0] - 1] if ids and ids[0] > 1 else [2, 1]
            mutate("TEST-6-06", True, bool(_check_lrat_body(m4)), f"seed {seed} (deletion id order broken)")

    need("TEST-6-06", "bodies_with_a_delete_step", "generated body containing a delete step")
    need("TEST-6-06", "multi_literal_clauses", "generated clause with more than one literal")

    # -- Rust test evidence: for every ID this module claims `enforced`, the -----
    # named real Rust #[test] functions must still exist and not be #[ignore]d
    # (textual check, no cargo). A Python check of a real artifact is not, by
    # itself, evidence about a Rust implementation.
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

    for rid in STATUS:
        if STATUS[rid] == "enforced" and rid not in RUST_TESTS:
            failures[rid].append(f"{rid} is claimed enforced but names no real Rust test evidence")

    # -- TEST-6-06 drift tie: the LRAT grammar phrases this module's model is ---
    # built from must still be the real, committed wire.rs text.
    drift = _check_lrat_grammar_drift()
    if drift:
        failures["TEST-6-06"].extend(drift)

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
        "unclaimed": ["TEST-6-07", "TEST-6-08"],
    }
