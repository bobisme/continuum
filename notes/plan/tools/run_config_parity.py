#!/usr/bin/env python3
"""Reader/schema parity for the run configuration (bn-3a9sr, cr-1sxoia round 4).

`notes/plan/schemas/run-config.schema.json` is normative for the run configuration
(INV-003, RFC 0003 correction 3); `continuum_cml_elab::config::RunConfig::parse` must
accept exactly the documents it admits. This module is the schema half of a
differential:

- `cases()` generates a fixed corpus: the committed fixtures, and edge names and
  shapes for every field the schema constrains;
- `verdict(text)` decides a document by Draft 2020-12 validation of the schema,
  under the document profile the schema states beside it (below);
- `--write` records every case with its verdict in
  `crates/continuum-cml-elab/tests/configs/parity.json`;
- `check()` (run by `validate_dossier.py`, which provides `jsonschema`) regenerates
  the corpus and fails if the committed file differs, so the recorded verdicts are
  the schema's own.

The Rust half, `crates/continuum-cml-elab/tests/configured.rs::
the_reader_agrees_with_the_schema_on_every_parity_case`, runs every recorded document
through the reader and requires the same verdict.

The document profile — what the schema's description and schemas/README.md state
beside the JSON Schema, and what a JSON Schema validator cannot see because it
validates the parsed data model, not the text:

1. lexical (the strict ID5 reader): no duplicate object key, no floating-point
   number (so `1.0` is not `1`), no `NaN`/`Infinity`, no lone surrogate escape;
2. value-level: no duplicate `set` member and no duplicate `map` key, by value
   (a set's members and a map's entries compared after the same normalization the
   identity uses);
3. bound-level (RFC 0003 correction 4): an `Int` bound has `min <= max` and holds at
   most 65536 values (JSON Schema cannot relate two fields).

Two reader caps are resource bounds outside the schema and outside this corpus: the
16 MiB document size and the JSON nesting depth of 64.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
PROJECT_ROOT = ROOT.parents[1]
SCHEMA = ROOT / "schemas/run-config.schema.json"
CORPUS = PROJECT_ROOT / "crates/continuum-cml-elab/tests/configs/parity.json"
FIXTURES = (
    ROOT / "schemas/examples/replicated-register.run-config.json",
    PROJECT_ROOT / "crates/continuum-cml-elab/tests/configs/ring.run-config.json",
)

SCHEMA_ID = "https://continuum.dev/schema/run-config.json"
MAX_BOUND_VALUES = 1 << 16
I64_MIN, I64_MAX = -(2**63), 2**63 - 1


class Refused(Exception):
    """A document outside the profile."""


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    out: dict[str, Any] = {}
    for key, value in pairs:
        if key in out:
            raise Refused(f"duplicate key {key!r}")
        out[key] = value
    return out


def _no_float(text: str) -> Any:
    raise Refused(f"floating-point number {text}")


def _no_constant(text: str) -> Any:
    raise Refused(f"non-finite number {text}")


def _strings_are_utf8(value: Any) -> None:
    stack = [value]
    while stack:
        v = stack.pop()
        if isinstance(v, str):
            try:
                v.encode("utf-8")
            except UnicodeEncodeError as exc:
                raise Refused("lone surrogate") from exc
        elif isinstance(v, dict):
            for k, x in v.items():
                stack.append(k)
                stack.append(x)
        elif isinstance(v, list):
            stack.extend(v)


def _canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def _normal(value: Any) -> Any:
    """A tagged value with `set` members and `map` entries in canonical order; raises
    `Refused` on a duplicate member or key. Only called on schema-valid values."""
    (tag, body), = value.items()
    if tag in ("tuple", "seq"):
        return {tag: [_normal(x) for x in body]}
    if tag == "set":
        members = sorted((_normal(x) for x in body), key=_canonical)
        keys = [_canonical(m) for m in members]
        if len(keys) != len(set(keys)):
            raise Refused("duplicate set member")
        return {tag: members}
    if tag == "map":
        entries = sorted(([_normal(k), _normal(v)] for k, v in body), key=lambda e: _canonical(e[0]))
        keys = [_canonical(e[0]) for e in entries]
        if len(keys) != len(set(keys)):
            raise Refused("duplicate map key")
        return {tag: entries}
    if tag == "record":
        return {tag: {k: _normal(x) for k, x in body.items()}}
    if tag == "some":
        return {tag: _normal(body)}
    return value


def verdict(text: str) -> bool:
    """Whether the schema, under the document profile, admits `text`."""
    from jsonschema import Draft202012Validator

    try:
        doc = json.loads(
            text,
            object_pairs_hook=_pairs,
            parse_float=_no_float,
            parse_constant=_no_constant,
        )
        _strings_are_utf8(doc)
    except (Refused, ValueError):
        return False
    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    if not Draft202012Validator(schema).is_valid(doc):
        return False
    try:
        for value in doc["constants"].values():
            _normal(value)
    except Refused:
        return False
    int_bound = doc.get("bounds", {}).get("Int")
    if int_bound is not None:
        lo, hi = int_bound["min"], int_bound["max"]
        if lo > hi or hi - lo + 1 > MAX_BOUND_VALUES:
            return False
    return True


def _doc(model: Any = "M", sorts: Any = None, constants: Any = None, **extra: Any) -> str:
    body = {
        "schema_id": SCHEMA_ID,
        "schema_epoch": 1,
        "model": model,
        "sorts": {} if sorts is None else sorts,
        "constants": {} if constants is None else constants,
    }
    body.update(extra)
    return json.dumps(body, ensure_ascii=False)


def _const(value: Any) -> str:
    return _doc(constants={"C": value})


NAMES = [
    "a", "Ring", "!", "~", "a-b", "a.b", "1", "x" * 128,
    "x" * 129, "", "a b", "a\n", "a\t", "é", "\n",
]
IDENTIFIERS = [
    "a", "_", "A1", "a_b", "Busy", "y" * 128,
    "y" * 129, "1a", "a-b", "bad-name", "1", "", "a\n", "a b", "é",
]


def cases() -> list[tuple[str, str]]:
    """The corpus, in a fixed order: (case name, document text)."""
    out: list[tuple[str, str]] = []
    for path in FIXTURES:
        out.append((f"fixture {path.name}", path.read_text(encoding="utf-8")))

    for n in NAMES:
        out.append((f"model name {n!r}", _doc(model=n)))
        out.append((f"element name {n!r}", _doc(sorts={"S": {"elements": [n]}})))
        out.append((f"elem.name {n!r}", _const({"elem": {"sort": "S", "name": n}})))
    for n in IDENTIFIERS:
        out.append((f"sort name {n!r}", _doc(sorts={n: {"elements": ["a"]}})))
        out.append((f"constant name {n!r}", _doc(constants={n: {"int": 0}})))
        out.append((f"elem.sort {n!r}", _const({"elem": {"sort": n, "name": "a"}})))
        out.append((f"variant.enum {n!r}", _const({"variant": {"enum": n, "name": "V"}})))
        out.append((f"variant.name {n!r}", _const({"variant": {"enum": "E", "name": n}})))
        out.append((f"record field {n!r}", _const({"record": {n: {"int": 0}}})))

    shapes: list[tuple[str, str]] = [
        ("elements empty", _doc(sorts={"S": {"elements": []}})),
        ("elements duplicate", _doc(sorts={"S": {"elements": ["a", "a"]}})),
        ("elements two", _doc(sorts={"S": {"elements": ["a", "b"]}})),
        ("elements not strings", _doc(sorts={"S": {"elements": [1]}})),
        ("sort extra key", _doc(sorts={"S": {"elements": ["a"], "size": 1}})),
        ("sort missing elements", _doc(sorts={"S": {}})),
        ("int zero", _const({"int": 0})),
        ("int min", _const({"int": -(2**63)})),
        ("int max", _const({"int": 2**63 - 1})),
        ("int over", _const({"int": 2**63})),
        ("int under", _const({"int": -(2**63) - 1})),
        ("int as string", _const({"int": "1"})),
        ("int as bool", _const({"int": True})),
        ("bool", _const({"bool": False})),
        ("bool as int", _const({"bool": 0})),
        ("str empty", _const({"str": ""})),
        ("str any", _const({"str": "a b\né"})),
        ("str as int", _const({"str": 1})),
        ("elem ok", _const({"elem": {"sort": "S", "name": "a"}})),
        ("elem missing name", _const({"elem": {"sort": "S"}})),
        ("elem extra key", _const({"elem": {"sort": "S", "name": "a", "x": 1}})),
        ("variant ok", _const({"variant": {"enum": "E", "name": "V"}})),
        ("tuple empty", _const({"tuple": []})),
        ("tuple one", _const({"tuple": [{"int": 1}]})),
        ("tuple two", _const({"tuple": [{"int": 1}, {"bool": True}]})),
        ("record empty", _const({"record": {}})),
        ("record nested", _const({"record": {"a": {"record": {"b": {"int": 1}}}}})),
        ("set empty", _const({"set": []})),
        ("set", _const({"set": [{"int": 1}, {"int": 2}]})),
        ("set duplicate", _const({"set": [{"int": 1}, {"int": 1}]})),
        (
            "set duplicate by value",
            _const({"set": [
                {"set": [{"int": 1}, {"int": 2}]},
                {"set": [{"int": 2}, {"int": 1}]},
            ]}),
        ),
        (
            "set duplicate records",
            _const({"set": [
                {"record": {"a": {"int": 1}, "b": {"int": 2}}},
                {"record": {"b": {"int": 2}, "a": {"int": 1}}},
            ]}),
        ),
        ("seq duplicate", _const({"seq": [{"int": 1}, {"int": 1}]})),
        ("map", _const({"map": [[{"int": 1}, {"int": 2}]]})),
        ("map duplicate key", _const({"map": [[{"int": 1}, {"int": 2}], [{"int": 1}, {"int": 3}]]})),
        ("map pair of three", _const({"map": [[{"int": 1}, {"int": 2}, {"int": 3}]]})),
        ("map pair of one", _const({"map": [[{"int": 1}]]})),
        ("none", _const({"none": None})),
        ("none not null", _const({"none": 0})),
        ("some", _const({"some": {"some": {"int": 1}}})),
        ("two tags", _const({"int": 1, "bool": True})),
        ("no tag", _const({})),
        ("unknown tag", _const({"float": 1})),
        ("value not an object", _const([{"int": 1}])),
        ("missing constants", json.dumps({"schema_id": SCHEMA_ID, "schema_epoch": 1, "model": "M", "sorts": {}})),
        ("extra top-level", _doc(faults={})),
        ("wrong schema_id", _doc(schema_id="https://continuum.dev/schema/other.json")),
        ("epoch 2", _doc(schema_epoch=2)),
        ("epoch true", _doc(schema_epoch=True)),
        ("sorts not an object", _doc(sorts=[])),
        ("top level array", "[]"),
        ("epoch 1.0", _doc().replace('"schema_epoch": 1', '"schema_epoch": 1.0')),
        ("int 2.0", _const({"int": 2}).replace('{"int": 2}', '{"int": 2.0}')),
        ("int exponent", _const({"int": 2}).replace('{"int": 2}', '{"int": 2e0}')),
        ("duplicate key", _doc().replace('"model": "M"', '"model": "M", "model": "M"')),
        ("trailing bytes", _doc() + " {}"),
        ("NaN", _const({"int": 2}).replace('{"int": 2}', '{"int": NaN}')),
        ("lone surrogate", _const({"str": "x"}).replace('"x"', '"\\ud800"')),
        ("escaped name", _doc(model="M").replace('"model": "M"', '"model": "\\u0041"')),
        ("whitespace around", "  \n" + _doc() + "\n  "),
        # bounds (RFC 0003 correction 4)
        ("bounds Nat", _doc(bounds={"Nat": {"max": 3}})),
        ("bounds Nat zero", _doc(bounds={"Nat": {"max": 0}})),
        ("bounds Nat at the value limit", _doc(bounds={"Nat": {"max": 65535}})),
        ("bounds Nat over the value limit", _doc(bounds={"Nat": {"max": 65536}})),
        ("bounds Nat negative", _doc(bounds={"Nat": {"max": -1}})),
        ("bounds Nat as string", _doc(bounds={"Nat": {"max": "3"}})),
        ("bounds Nat as bool", _doc(bounds={"Nat": {"max": True}})),
        ("bounds Nat missing max", _doc(bounds={"Nat": {}})),
        ("bounds Nat extra key", _doc(bounds={"Nat": {"max": 3, "min": 0}})),
        ("bounds Nat not an object", _doc(bounds={"Nat": 3})),
        ("bounds Int", _doc(bounds={"Int": {"min": -2, "max": 2}})),
        ("bounds Int one value", _doc(bounds={"Int": {"min": 5, "max": 5}})),
        ("bounds Int crossing", _doc(bounds={"Int": {"min": 3, "max": 2}})),
        ("bounds Int at the value limit", _doc(bounds={"Int": {"min": -32768, "max": 32767}})),
        ("bounds Int over the value limit", _doc(bounds={"Int": {"min": -32768, "max": 32768}})),
        ("bounds Int at the i64 top", _doc(bounds={"Int": {"min": I64_MAX - 1, "max": I64_MAX}})),
        ("bounds Int at the i64 bottom", _doc(bounds={"Int": {"min": I64_MIN, "max": I64_MIN + 1}})),
        ("bounds Int whole i64", _doc(bounds={"Int": {"min": I64_MIN, "max": I64_MAX}})),
        ("bounds Int over i64", _doc(bounds={"Int": {"min": 0, "max": I64_MAX + 1}})),
        ("bounds Int missing min", _doc(bounds={"Int": {"max": 2}})),
        ("bounds Int extra key", _doc(bounds={"Int": {"min": 0, "max": 2, "step": 1}})),
        ("bounds both", _doc(bounds={"Nat": {"max": 1}, "Int": {"min": -1, "max": 1}})),
        ("bounds empty", _doc(bounds={})),
        ("bounds unknown type", _doc(bounds={"Real": {"max": 1}})),
        ("bounds lowercase nat", _doc(bounds={"nat": {"max": 1}})),
        ("bounds not an object", _doc(bounds=[])),
        ("bounds null", _doc(bounds=None)),
        ("bounds Nat 1.0", _doc(bounds={"Nat": {"max": 1}}).replace('"max": 1', '"max": 1.0')),
    ]
    out.extend(shapes)
    return out


def corpus() -> dict[str, Any]:
    return {
        "note": "Generated by notes/plan/tools/run_config_parity.py --write; verdicts are the "
        "run-config schema's under its document profile. Do not edit by hand.",
        "cases": [
            {"name": name, "document": text, "accept": verdict(text)}
            for name, text in cases()
        ],
    }


def render() -> str:
    return json.dumps(corpus(), indent=2, ensure_ascii=True) + "\n"


def check() -> dict[str, Any]:
    """Fail if the committed corpus differs from a fresh generation."""
    want = render()
    have = CORPUS.read_text(encoding="utf-8") if CORPUS.exists() else ""
    assert have == want, (
        f"{CORPUS.relative_to(PROJECT_ROOT)} is stale: regenerate with "
        "`uv run --with jsonschema python3 notes/plan/tools/run_config_parity.py --write`"
    )
    data = json.loads(want)
    accepted = sum(1 for c in data["cases"] if c["accept"])
    return {"cases": len(data["cases"]), "accepted": accepted}


def main() -> int:
    if "--write" in sys.argv[1:]:
        CORPUS.write_text(render(), encoding="utf-8")
        return 0
    print(json.dumps(check(), indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
