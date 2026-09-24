"""docs/19 §3 "Metamorphic relations" — TEST-3-07 … TEST-3-10 (bn-1rt2).

The remaining four "Expected preservation" bullets of docs/19 §3, left unclaimed
by `s3_metamorphic_relations.py` (bn-2sn5, TEST-3-01..06): independent-event swap,
equivalent guard normalization, serialization round trip, snapshot restore versus
root replay.

# The delivered bar, applied strictly (lead directive, bn-1rt2)

`s3_metamorphic_relations.py`'s own `_ENGINE_BOUNDARY` records that TEST-3-01..06
are checked only by this harness's own `tsys` oracle: "no optimized or Rust-side
engine is compared against it yet." Those six IDs are nonetheless annotated
`(delivered: bn-2sn5 — …)` in docs/19, because that annotation predates the
stricter bar `s4_protocol_corpus.py` (bn-2dt2) later established: killing a mutant
of a Python port shows the port catches the mutant class, not that the real Rust
code does. This module applies that later, stricter bar to all four of its own
IDs, regardless of what the sibling module did before it existed:

- **TEST-3-07** (independent-event swap) has real, load-bearing production code to
  bind to: `crates/continuum-engine-reference/src/semantic/oracle.rs`'s
  `check_independence` computes exactly this relation — firing a claimed-independent
  co-enabled pair in each order and requiring the two orders to reach one state (the
  diamond) — over every reachable state of every generated system, and
  `DefectClass::Dependence` (`semantic/defect.rs`) seeds the literal violating
  mutant ("declare conflicting events independent"). `enforced`.
- **TEST-3-09** (serialization round trip) also has real production code:
  `crates/continuum-value`'s CVNF-1 suite
  (`tests/phase_a_canonical_round_trip.rs`) states and tests exactly the two laws
  "serialization round trip" names — `decode(encode(v)) == v` and
  `encode(decode(b)) == b`. `enforced`.
- **TEST-3-08** (equivalent guard normalization) now has a real counterpart
  (bn-33g98, once bn-ybq landed the elaborator): `continuum-cml-elab`'s `norm`
  module normalizes a CML action's guard to `Action::guard`, a `Vec<Expr>` of
  conjuncts in source order, over which `NormModel::identity` is content-addressed
  (ADR-0013) — so two guards that normalize alike share one identity, and two that
  do not, do not. `elab.rs`'s `conjuncts`/`split_clauses` is the real code that
  does the splitting; `crates/continuum-cml-elab/tests/guard_equivalence.rs`
  exercises it directly: nested and separately-declared conjunctions share an
  identity, a dropped conjunct or a changed bound does not, and a hand-built
  mutant that disables the splitting rule (merges the split guard back into one
  unsplit `&&`) is shown to break the identity equality — the same discipline
  `s4_protocol_corpus.py` (bn-2dt2) established. `enforced`.
- **TEST-3-10** (snapshot restore versus root replay) also has no real
  counterpart. `continuum-engine-reference/src/bfs.rs`'s `Partial::frontier` is
  documented as "a resume point" and states that "a resumed walk agree[s] with an
  unbounded one" — but no `resume`/`restore` function exists (checked: `bfs.rs`
  declares only `explore`, never `resume`), and no test compares a resumed walk
  against a full one (checked: the only source hit for "resume" anywhere in
  `continuum-engine-reference`'s test suite is that one doc-comment line in
  `tests/bfs_diehard.rs`, not an assertion). `crates/continuumd`'s restart path is
  the opposite of a "root replay" comparison: `daemon/recovery.rs`'s own words are
  "MUST NOT reconstruct task state by inference" (docs/35) — a restart resumes
  only from a committed continuation record, and there is no replay-from-genesis
  path in the daemon for a restore to be checked against. `partial`.

# Method

Each ID still gets a full `tsys`-level transform-and-checker (or, for TEST-3-07, a
direct semantic check with no transform), a corpus over the same seeded generator
`s3_metamorphic_relations.py` uses (`tsys.generate`, seeds 0..255), a mutation that
the checker must detect, and fixtures — exactly the sibling module's methodology
(README-test.md, "Adding a section"). This module imports `tsys` and reuses its
existing functions (`compile_system`, `initial_state`, `enabled`, `fire`,
`explore`, `canonical`, `generate`, `SplitMix64`, `Finding`) without editing a line
of it or of any other section's module, per README-test.md.

Where a real Rust binding exists (TEST-3-07, TEST-3-08, TEST-3-09), `RUST_BINDING`
names the `crates/` test(s) that exercise the real code, and `real_run()`
verifies — textually, no cargo — that each named test exists, is `#[test]`, is not
`#[ignore]`d, and still contains the token that names the assertion this ID's
relation would break. TEST-3-07 additionally re-derives the breach-classification
tokens `check_independence` reports (`FirstDisablesSecond`, `SecondDisablesFirst`,
`DiamondOpen`) from the live source, so the Python port's own three-way
classification (`_swap_order`) cannot silently drift from the function it mirrors.
TEST-3-08 needs no such drift check: its tsys-level check (`_guard_equivalence`,
via `normalize_guard`'s De Morgan-plus-double-negation rewrite) is not a port of
`elab.rs`'s conjunct-splitting algorithm — it is an independent, oracle-preserving
rewrite over the harness's own system shape, the same relationship TEST-3-09's
tsys-level round trip has to CVNF-1 (`_BOUNDARY_09`). There is nothing for it to
drift from; only the Rust-binding check applies.
"""

from __future__ import annotations

import copy
import json
import re
from pathlib import Path
from typing import Any

import tsys

SECTION = 3
TITLE = "Metamorphic relations"
OBLIGATIONS = {
    "TEST-3-07": "independent-event swap",
    "TEST-3-08": "equivalent guard normalization",
    "TEST-3-09": "serialization round trip",
    "TEST-3-10": "snapshot restore versus root replay.",
}
RULES: dict[str, str] = {
    "independent-swap-preserved": "TEST-3-07",
    "independent-swap-rust-binding-missing": "TEST-3-07",
    "independent-swap-diamond-drift": "TEST-3-07",
    "guard-normalize-preserved": "TEST-3-08",
    "guard-normalize-rust-binding-missing": "TEST-3-08",
    "serialization-round-trip-tsys": "TEST-3-09",
    "serialization-round-trip-rust-binding-missing": "TEST-3-09",
    "snapshot-restore-matches-replay": "TEST-3-10",
}
SEEDS = range(256)

_REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent  # tools/test-policy/sections/../../../..


def _read_source(rel_path: str) -> str | None:
    try:
        return (_REPO_ROOT / rel_path).read_text(encoding="utf-8")
    except OSError:
        return None


def _finding(rule: str, subject: str, message: str) -> tsys.Finding:
    return tsys.Finding(rule, RULES[rule], subject, message)


# ---------------------------------------------------------------------------
# Real-implementation binding (TEST-3-07, TEST-3-09): the same discipline
# s4_protocol_corpus.py established (bn-2dt2) — RUST_BINDING names the real test(s),
# _check_rust_binding verifies existence, #[test], not #[ignore], and the cited
# assertion token, against both a fixture's synthetic source and the real committed
# file.
# ---------------------------------------------------------------------------

RUST_BINDING: dict[str, dict[str, Any]] = {
    "TEST-3-07": {
        "path": "crates/continuum-engine-reference/tests/semantic_oracle.rs",
        "tests": {
            # a declared-independent pair has exactly two interleavings and one trace
            # class — the direct "swap order, same outcome" known answer.
            "an_independent_pair_has_two_interleavings_and_one_class": "(2, 0, 1)",
            # a genuinely dependent pair has two interleavings and two classes — the
            # negative control that shows the checker is not vacuous.
            "a_dependent_pair_has_two_interleavings_and_two_classes": "(2, 0, 2)",
            # a "declare conflicting events independent" mutant is caught by the
            # diamond, not only by the (also mutated) footprint.
            "a_dependence_mutant_is_caught_by_the_diamond_not_only_the_footprint": "IndependenceViolated",
        },
    },
    "TEST-3-09": {
        "path": "crates/continuum-value/tests/phase_a_canonical_round_trip.rs",
        "tests": {
            "positive_decoding_an_encoding_returns_the_value": '"decode(encode(v)) == v"',
            "positive_re_encoding_a_decoded_value_reproduces_its_bytes": '"encode(decode(b)) == b"',
        },
    },
    "TEST-3-08": {
        "path": "crates/continuum-cml-elab/tests/guard_equivalence.rs",
        "tests": {
            # nested and separately-declared conjunctions share one identity.
            "nested_conjunction_splits_the_same_as_three_separate_requires": (
                '"a nested && chain and three separate requires are one guard"'
            ),
            # the negative control: a dropped conjunct is a real change of meaning.
            "guards_that_differ_in_meaning_keep_different_identities": '"dropping a conjunct changes the guard"',
            # the mutant: disabling conjunct splitting must break the identity equality.
            "disabling_conjunct_splitting_breaks_the_relation": "disabling conjunct splitting must change the identity",
        },
    },
}
BINDING_RULE = {
    "TEST-3-07": "independent-swap-rust-binding-missing",
    "TEST-3-08": "guard-normalize-rust-binding-missing",
    "TEST-3-09": "serialization-round-trip-rust-binding-missing",
}


def _find_test_fn(source: str, name: str) -> tuple[str, str] | None:
    """`(prelude, body)` for a zero-argument `fn name() { ... }` in `source`:
    `prelude` is the two source lines immediately above it (where `#[test]`/
    `#[ignore]` live), `body` is the balanced, brace-matched function body. `None`
    if `name` is not declared this way."""
    m = re.search(r"(?m)^fn\s+" + re.escape(name) + r"\s*\(\s*\)\s*(?:->\s*[^{]+)?\{", source)
    if m is None:
        return None
    start = m.end() - 1
    depth = 0
    for i in range(start, len(source)):
        if source[i] == "{":
            depth += 1
        elif source[i] == "}":
            depth -= 1
            if depth == 0:
                prelude = "\n".join(source[: m.start()].splitlines()[-2:])
                return prelude, source[start : i + 1]
    return None


def _check_rust_binding(rid: str, source: Any) -> list[dict]:
    rule = BINDING_RULE[rid]
    spec = RUST_BINDING[rid]
    if not isinstance(source, str):
        return [_finding(rule, spec["path"], "the named source file could not be read").as_json()]
    findings = []
    for name, token in spec["tests"].items():
        found = _find_test_fn(source, name)
        if found is None:
            findings.append(
                _finding(rule, name, f"`{name}` (expected in {spec['path']}) does not exist — {rid}'s real-code binding is missing").as_json()
            )
            continue
        prelude, body = found
        if "#[test]" not in prelude:
            findings.append(_finding(rule, name, f"`{name}` exists but is not `#[test]`, so it never runs").as_json())
        if "#[ignore" in prelude:
            findings.append(_finding(rule, name, f"`{name}` is `#[ignore]`d, so it never runs").as_json())
        if token not in body:
            findings.append(
                _finding(rule, name, f"`{name}` no longer contains {token!r} — it may no longer exercise {rid}'s relation").as_json()
            )
    return findings


def _check_rust_binding_3_07(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"source"}):
        return [_finding("independent-swap-rust-binding-missing", "<payload>", "payload must have exactly source").as_json()]
    return _check_rust_binding("TEST-3-07", payload["source"])


def _check_rust_binding_3_09(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"source"}):
        return [_finding("serialization-round-trip-rust-binding-missing", "<payload>", "payload must have exactly source").as_json()]
    return _check_rust_binding("TEST-3-09", payload["source"])


def _check_rust_binding_3_08(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"source"}):
        return [_finding("guard-normalize-rust-binding-missing", "<payload>", "payload must have exactly source").as_json()]
    return _check_rust_binding("TEST-3-08", payload["source"])


# ---------------------------------------------------------------------------
# Port/source drift (TEST-3-07): `_swap_order`'s three-way classification must not
# silently diverge from `check_independence`'s own breach variants.
# ---------------------------------------------------------------------------

BREACH_TOKENS = ("FirstDisablesSecond", "SecondDisablesFirst", "DiamondOpen")


def _find_fn_body(source: str, name: str) -> str | None:
    """Balanced-brace body of the first `fn <name>(` declaration in `source`,
    whatever its parameter list (unlike `_find_test_fn`, which only matches
    zero-argument test functions)."""
    m = re.search(r"(?m)^(?:pub(?:\([^)]*\))?\s+)?fn\s+" + re.escape(name) + r"\s*\(", source)
    if m is None:
        return None
    brace = source.find("{", m.end())
    if brace == -1:
        return None
    depth = 0
    for i in range(brace, len(source)):
        if source[i] == "{":
            depth += 1
        elif source[i] == "}":
            depth -= 1
            if depth == 0:
                return source[brace : i + 1]
    return None


def _check_independence_drift(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"source"}) or not isinstance(payload["source"], str):
        return [_finding("independent-swap-diamond-drift", "<payload>", "payload must have exactly source (a string)").as_json()]
    body = _find_fn_body(payload["source"], "check_independence")
    if body is None:
        return [_finding("independent-swap-diamond-drift", "oracle.rs", "fn check_independence was not found in the given source").as_json()]
    missing = [t for t in BREACH_TOKENS if t not in body]
    if missing:
        return [
            _finding(
                "independent-swap-diamond-drift",
                "check_independence",
                f"oracle.rs's check_independence no longer reports {missing} — TEST-3-07's swap-order port "
                "(FirstDisablesSecond/SecondDisablesFirst/DiamondOpen) no longer mirrors real code",
            ).as_json()
        ]
    return []


# ---------------------------------------------------------------------------
# TEST-3-07: independent-event swap
# ---------------------------------------------------------------------------


def _find_transition(c: tsys.Compiled, name: str) -> dict:
    return next(t for t in c.transitions if t["name"] == name)


def _find_coenabled_state(c: tsys.Compiled, a_name: str, b_name: str) -> tsys.State | None:
    """The first reachable state (breadth-first from init) where both named
    transitions are enabled, found by an independent re-walk of the reachable set
    rather than by reading `tsys.explore`'s own `coenabled_*` bookkeeping — so this
    checker does not merely relabel TEST-2-04's `independence-diamond` rule."""
    ta, tb = _find_transition(c, a_name), _find_transition(c, b_name)
    init = tsys.initial_state(c)
    seen = {init}
    frontier = [init]
    while frontier:
        nxt = []
        for s in frontier:
            if tsys.enabled(c, s, ta) and tsys.enabled(c, s, tb):
                return s
            for t in c.transitions:
                if tsys.enabled(c, s, t):
                    s2, fs = tsys.fire(c, s, t)
                    if s2 is not None and s2 not in seen:
                        seen.add(s2)
                        nxt.append(s2)
        frontier = nxt
    return None


def _swap_order(c: tsys.Compiled, state: tsys.State, a_name: str, b_name: str) -> tuple[bool, str]:
    """Fire `a_name` then `b_name` and `b_name` then `a_name` from `state`, and
    require the same outcome. Mirrors `check_independence`'s own breach
    classification (`oracle.rs`): `FirstDisablesSecond`, `SecondDisablesFirst`,
    `DiamondOpen`."""
    ta, tb = _find_transition(c, a_name), _find_transition(c, b_name)
    sa, _ = tsys.fire(c, state, ta)
    sb, _ = tsys.fire(c, state, tb)
    if sa is None or sb is None:
        return False, "one of the pair produced a dynamic finding instead of a successor"
    if not tsys.enabled(c, sa, tb):
        return False, f"firing {a_name} first disables {b_name} (FirstDisablesSecond)"
    if not tsys.enabled(c, sb, ta):
        return False, f"firing {b_name} first disables {a_name} (SecondDisablesFirst)"
    sab, _ = tsys.fire(c, sa, tb)
    sba, _ = tsys.fire(c, sb, ta)
    if sab != sba:
        return False, f"{a_name} then {b_name} reaches a different state than {b_name} then {a_name} (DiamondOpen)"
    return True, ""


def _check_independent_swap(payload: Any) -> list[dict]:
    need = {"system", "pair"}
    if not (isinstance(payload, dict) and set(payload) == need and isinstance(payload["pair"], list) and len(payload["pair"]) == 2):
        return [_finding("independent-swap-preserved", "<payload>", "payload must have exactly system and pair (a 2-element list)").as_json()]
    system = payload["system"]
    fs = tsys.check_static(system)
    if fs:
        return [_finding("independent-swap-preserved", "<system>", f"system is not well-formed: {len(fs)} static finding(s)").as_json()]
    c = tsys.compile_system(system)
    a_name, b_name = payload["pair"]
    state = _find_coenabled_state(c, a_name, b_name)
    if state is None:
        return [_finding("independent-swap-preserved", f"{a_name}|{b_name}", "no reachable state co-enables the named pair").as_json()]
    ok, detail = _swap_order(c, state, a_name, b_name)
    if ok:
        return []
    return [_finding("independent-swap-preserved", f"{a_name}|{b_name}", detail).as_json()]


# ---------------------------------------------------------------------------
# TEST-3-08: equivalent guard normalization
# ---------------------------------------------------------------------------


def _demorgan(expr: Any) -> Any:
    """Rewrite `and`/`or` nodes one level via De Morgan's law, leaving everything
    else structurally unchanged: `and(a, b, ...)` -> `not(or(not(a), not(b), ...))`
    and the dual. A semantics-preserving syntactic rewrite, not a simplification."""
    if not isinstance(expr, dict) or "op" not in expr:
        return expr
    op, args = expr["op"], expr.get("args", [])
    if op == "and":
        return {"op": "not", "args": [{"op": "or", "args": [{"op": "not", "args": [_demorgan(a)]} for a in args]}]}
    if op == "or":
        return {"op": "not", "args": [{"op": "and", "args": [{"op": "not", "args": [_demorgan(a)]} for a in args]}]}
    if op == "ite":
        return {"op": "ite", "args": [_demorgan(args[0]), _demorgan(args[1]), _demorgan(args[2])]}
    return {"op": op, "args": [_demorgan(a) for a in args]} if args else dict(expr)


def normalize_guard(system: dict) -> dict:
    """Rewrite every transition's guard into a semantically identical, syntactically
    different form: one level of De Morgan pushed through every and/or, the whole
    result wrapped in a double negation. Guardless transitions (none in `tsys.generate`'s
    output, but the shape allows them) are untouched."""
    m = copy.deepcopy(system)
    for t in m["transitions"]:
        if "guard" in t:
            t["guard"] = {"op": "not", "args": [{"op": "not", "args": [_demorgan(t["guard"])]}]}
    return m


def _guard_equivalence(base: dict, transformed: dict) -> tuple[bool, str]:
    """`transformed` must differ from `base` only in guard spelling: same reachable
    states, same oracle metrics."""
    fb = tsys.check_static(base)
    ft = tsys.check_static(transformed)
    if fb or ft:
        return False, f"static findings: base={len(fb)} transformed={len(ft)}"
    rb = tsys.explore(base)
    rt = tsys.explore(transformed)
    if set(rb.reachable) != set(rt.reachable):
        return False, "reachable state sets differ under the equivalent guard rewrite"
    metrics_b = (rb.states, rb.interleavings, rb.trace_classes)
    metrics_t = (rt.states, rt.interleavings, rt.trace_classes)
    if metrics_b != metrics_t:
        return False, f"aggregate oracle metrics differ: {metrics_b} != {metrics_t}"
    return True, ""


def _corrupt_guard_normalize(transformed: dict) -> dict:
    """Strip one `not` from the double-negation wrapper `normalize_guard` applies
    to the first guarded transition it finds, turning `not(not(demorgan(G)))` into
    `not(demorgan(G))` — the guard's real logical negation, not an equivalent
    rewrite."""
    m = copy.deepcopy(transformed)
    for t in m["transitions"]:
        g = t.get("guard")
        if isinstance(g, dict) and g.get("op") == "not":
            t["guard"] = g["args"][0]
            return m
    raise ValueError("no negated guard found to corrupt")


def _check_guard_normalize(payload: Any) -> list[dict]:
    need = {"base", "transformed"}
    if not (isinstance(payload, dict) and set(payload) == need):
        return [_finding("guard-normalize-preserved", "<payload>", "payload must have exactly base and transformed").as_json()]
    ok, detail = _guard_equivalence(payload["base"], payload["transformed"])
    if ok:
        return []
    return [_finding("guard-normalize-preserved", "guard rewrite", detail).as_json()]


# ---------------------------------------------------------------------------
# TEST-3-09: serialization round trip
# ---------------------------------------------------------------------------


def _round_trip(system: Any, blob: str | None = None) -> tuple[bool, str]:
    """`decode(encode(v)) == v` and `encode(decode(b)) == b`, over the harness's own
    canonical-JSON encoding of a system (`tsys.canonical`). `blob`, when given,
    substitutes for `canonical(system)` at the decode step — the "bytes arrived
    corrupted" case a fixture exercises."""
    honest = blob is None
    b = tsys.canonical(system) if honest else blob
    try:
        restored = json.loads(b)
    except json.JSONDecodeError as exc:
        return False, f"the encoding does not decode: {exc}"
    if restored != system:
        return False, "decoding the encoding did not reproduce the system"
    if honest:
        reencoded = tsys.canonical(restored)
        if reencoded != b:
            return False, "re-encoding the decoded system changed the bytes"
    return True, ""


def _check_round_trip(payload: Any) -> list[dict]:
    allowed = {"system", "blob"}
    if not (isinstance(payload, dict) and "system" in payload and set(payload) <= allowed):
        return [_finding("serialization-round-trip-tsys", "<payload>", "payload must have system and optionally blob").as_json()]
    ok, detail = _round_trip(payload["system"], payload.get("blob"))
    if ok:
        return []
    return [_finding("serialization-round-trip-tsys", "<system>", detail).as_json()]


# ---------------------------------------------------------------------------
# TEST-3-10: snapshot restore versus root replay
# ---------------------------------------------------------------------------


def _walk(c: tsys.Compiled, start: tsys.State, names: list[str]) -> tuple[tsys.State | None, str]:
    """Fire `names` in order from `start`. `None` with a reason the moment a named
    transition is not enabled or produces a dynamic finding, rather than firing it
    anyway — a trace that cannot be walked is itself evidence a restore diverged."""
    s = start
    for name in names:
        t = _find_transition(c, name)
        if not tsys.enabled(c, s, t):
            return None, f"{name} is not enabled at this point in the walk"
        s2, fs = tsys.fire(c, s, t)
        if s2 is None:
            return None, f"{name} produced a dynamic finding instead of a successor"
        s = s2
    return s, ""


def _serialize_state(state: tsys.State) -> str:
    values, held = state
    return tsys.canonical({"values": list(values), "held": list(held)})


def _deserialize_state(blob: str) -> tsys.State:
    obj = json.loads(blob)
    return (tuple(obj["values"]), tuple(obj["held"]))


def _snapshot_restore_equivalence(system: dict, names: list[str], checkpoint: int, blob_override: str | None = None) -> tuple[bool, str]:
    """`names` fired end to end from the root is "root replay". `names[:checkpoint]`
    fired from the root, then serialized, then restored and continued with
    `names[checkpoint:]`, is "snapshot restore". The two must reach the same state.
    `blob_override`, when given, replaces the honestly serialized snapshot — the
    "the persisted snapshot was corrupted" case a fixture exercises."""
    c = tsys.compile_system(system)
    prefix, suffix = names[:checkpoint], names[checkpoint:]
    replayed, reason = _walk(c, tsys.initial_state(c), names)
    if replayed is None:
        return False, f"the root-replay trace itself does not walk cleanly: {reason}"
    if blob_override is not None:
        blob = blob_override
    else:
        snap_state, reason0 = _walk(c, tsys.initial_state(c), prefix)
        if snap_state is None:
            return False, f"the snapshot prefix does not walk cleanly: {reason0}"
        blob = _serialize_state(snap_state)
    try:
        restored_start = _deserialize_state(blob)
    except (json.JSONDecodeError, KeyError, TypeError, ValueError) as exc:
        return False, f"the snapshot blob did not deserialize: {exc}"
    restored_final, reason2 = _walk(c, restored_start, suffix)
    if restored_final is None:
        return False, f"continuing from the restored snapshot: {reason2}"
    if restored_final != replayed:
        return False, "continuing from the restored snapshot reached a different state than replaying the whole trace from the root"
    return True, ""


def _check_snapshot_restore(payload: Any) -> list[dict]:
    need = {"system", "trace", "checkpoint"}
    if not (isinstance(payload, dict) and need <= set(payload) and set(payload) <= need | {"blob"}):
        return [_finding("snapshot-restore-matches-replay", "<payload>", f"payload must have {sorted(need)} and optionally blob").as_json()]
    system, trace, checkpoint = payload["system"], payload["trace"], payload["checkpoint"]
    if not (isinstance(trace, list) and isinstance(checkpoint, int) and 0 <= checkpoint <= len(trace)):
        return [_finding("snapshot-restore-matches-replay", "<payload>", "trace must be a list and 0 <= checkpoint <= len(trace)").as_json()]
    ok, detail = _snapshot_restore_equivalence(system, trace, checkpoint, payload.get("blob"))
    if ok:
        return []
    return [_finding("snapshot-restore-matches-replay", f"checkpoint {checkpoint}/{len(trace)}", detail).as_json()]


def _random_trace(c: tsys.Compiled, rng: tsys.SplitMix64, length: int) -> list[str]:
    names: list[str] = []
    s = tsys.initial_state(c)
    for _ in range(length):
        en = [t for t in c.transitions if tsys.enabled(c, s, t)]
        if not en:
            break
        t = rng.choice(en)
        s2, fs = tsys.fire(c, s, t)
        if s2 is None:
            break
        names.append(t["name"])
        s = s2
    return names


def _corrupt_value(v: Any) -> Any:
    if isinstance(v, bool):
        return not v
    if isinstance(v, int):
        return v + 1
    if isinstance(v, str):
        return v + "_x"
    return v


def _corrupt_blob(state: tsys.State) -> str:
    values, held = state
    if values:
        cv = list(values)
        cv[0] = _corrupt_value(cv[0])
        values = tuple(cv)
    else:
        held = ()
    return _serialize_state((values, held))


# ---------------------------------------------------------------------------
# Fixtures dispatch
# ---------------------------------------------------------------------------

_KIND_CHECKERS = {
    "independent-swap": _check_independent_swap,
    "independent-swap-rust-binding": _check_rust_binding_3_07,
    "independent-swap-drift": _check_independence_drift,
    "guard-normalize": _check_guard_normalize,
    "guard-normalize-rust-binding": _check_rust_binding_3_08,
    "round-trip": _check_round_trip,
    "round-trip-rust-binding": _check_rust_binding_3_09,
    "snapshot-restore": _check_snapshot_restore,
}


def check_fixture(system: Any) -> list[dict[str, str]]:
    if not isinstance(system, dict) or system.get("kind") not in _KIND_CHECKERS:
        return [_finding("independent-swap-preserved", "<payload>", "payload must have a known 'kind'").as_json()]
    payload = {k: v for k, v in system.items() if k != "kind"}
    return _KIND_CHECKERS[system["kind"]](payload)


# ---------------------------------------------------------------------------
# The real run
# ---------------------------------------------------------------------------

_BOUNDARY_07 = (
    "crates/continuum-engine-reference/src/semantic/oracle.rs's check_independence "
    "computes, for every co-enabled pair at every reachable state: firing first-then-"
    "second vs second-then-first and comparing the outcomes (the diamond); a claimed-"
    "independent pair that fails it is Finding::IndependenceViolated with a breach "
    "(FirstDisablesSecond, SecondDisablesFirst, or DiamondOpen). DefectClass::Dependence "
    "(semantic/defect.rs) seeds exactly the mutant this ID names: 'declare conflicting "
    "events independent'. The check below (_swap_order) is a direct port of that same "
    "three-way classification. Killing a mutant of this Python port does not by itself "
    "show the real oracle catches that mutant class, so real_run() also (a) verifies "
    "crates/continuum-engine-reference/tests/semantic_oracle.rs's "
    "an_independent_pair_has_two_interleavings_and_one_class, "
    "a_dependent_pair_has_two_interleavings_and_two_classes, and "
    "a_dependence_mutant_is_caught_by_the_diamond_not_only_the_footprint exist, are "
    "#[test], are not #[ignore]d, and still assert the cited known-answer/finding tokens "
    "(RUST_BINDING, _check_rust_binding), and (b) re-derives check_independence's live "
    "breach-variant names from oracle.rs and fails on any divergence from BREACH_TOKENS "
    "(_check_independence_drift). continuum-engine-explicit and continuum-engine-dpor, "
    "where an optimized reduction engine would also exercise this relation, remain "
    "documented stubs (the same boundary s3_metamorphic_relations.py's TEST-3-01..06 and "
    "TEST-2-04 record)."
)
_BOUNDARY_08 = (
    "continuum-cml-elab is real now (bn-ybq): norm.rs documents Action::guard as a "
    "Vec<Expr> of conjuncts in source order, over which NormModel::identity is "
    "content-addressed (ADR-0013); elab.rs's conjuncts/split_clauses is the code that "
    "splits require a && b, or two require clauses, into that one list. "
    "crates/continuum-cml-elab/tests/guard_equivalence.rs exercises it directly: "
    "nested_conjunction_splits_the_same_as_three_separate_requires shows two && "
    "associations and three separate requires share one NormModel identity and one "
    "lowered Model identity; guards_that_differ_in_meaning_keep_different_identities is "
    "the negative control (a dropped conjunct or a changed bound keeps a different "
    "identity); disabling_conjunct_splitting_breaks_the_relation is the mutant — a "
    "hand-built NormModel with the split guard merged back into one unsplit && no "
    "longer shares the elaborator's identity, showing the splitting rule is load-"
    "bearing. continuum-intent/src/cpnf.rs's CPNF-1 remains a different denotation "
    "(top-level intent-contract properties under RFC 0037's N1-N7, not a transition's "
    "guard) and is still not cited as this ID's binding. The tsys-level check below "
    "(_guard_equivalence) also still runs for real, over the harness's own oracle: it "
    "applies a De Morgan push plus a double-negation wrap (normalize_guard) to every "
    "generated system's guards and requires the reachable state set and oracle metrics "
    "to be unchanged, and a corrupted rewrite (_corrupt_guard_normalize, which leaves "
    "the guard's real logical negation) is required to be caught. This tsys-level "
    "rewrite is independent of elab.rs's algorithm (it is not a port of conjunct "
    "splitting), so no drift check applies to it, the same relationship TEST-3-09's "
    "tsys round trip has to CVNF-1."
)
_BOUNDARY_09 = (
    "crates/continuum-value/tests/phase_a_canonical_round_trip.rs states and tests "
    "exactly the two laws 'serialization round trip' names: decode(encode(v)) == v "
    "(positive_decoding_an_encoding_returns_the_value) and encode(decode(b)) == b "
    "(positive_re_encoding_a_decoded_value_reproduces_its_bytes), over CVNF-1, the "
    "canonical value encoding ADR-0013 makes identity. Killing a mutant of the tsys-"
    "level check below does not by itself show CVNF-1 catches a round-trip defect, so "
    "real_run() also verifies both named tests exist, are #[test], are not #[ignore]d, "
    "and still assert their own law by name (RUST_BINDING, _check_rust_binding). No "
    "drift check applies here: the tsys-level check (_round_trip) is not a port of "
    "CVNF-1's algorithm — it is the harness's own canonical-JSON round trip over a "
    "tsys system value (json.dumps/json.loads with sort_keys=True), a different, "
    "independently-real encoding already relied on throughout this harness's evidence "
    "files, not a mirror of CVNF-1's binary kind-tagged format. There is nothing to "
    "compare it against for divergence, so it is checked directly (corpus, mutation, "
    "fixtures) rather than ported."
)
_BOUNDARY_10 = (
    "No production or test code compares a restored snapshot against a full root "
    "replay. continuum-engine-reference/src/bfs.rs's Partial::frontier documents "
    "itself as 'a resume point' and states 'a resumed walk agree[s] with an unbounded "
    "one' — but bfs.rs declares only explore, never resume/restore (checked directly), "
    "and no test in continuum-engine-reference's suite exercises a resumed walk against "
    "a full one (checked: the only 'resume' hit in its tests is that one doc-comment "
    "line in tests/bfs_diehard.rs, not an assertion). crates/continuumd's restart path "
    "is the opposite of a root-replay comparison: daemon/recovery.rs's own words, "
    "quoting docs/35, are 'MUST NOT reconstruct task state by inference' — a restart "
    "resumes only from a committed continuation record, and there is no "
    "replay-from-genesis path in the daemon for a restore to be checked against. The "
    "check below (_snapshot_restore_equivalence) still runs for real over tsys systems: "
    "it fires a trace from the root, separately serializes the state reached partway "
    "through (the 'snapshot'), restores it, continues with the remaining trace, and "
    "requires the same final state as the full root replay; a corrupted restored blob "
    "(_corrupt_blob) is required to be caught."
)
BOUNDARIES: dict[str, list[str]] = {
    "TEST-3-07": [_BOUNDARY_07],
    "TEST-3-08": [_BOUNDARY_08],
    "TEST-3-09": [_BOUNDARY_09],
    "TEST-3-10": [_BOUNDARY_10],
}


def real_run() -> dict[str, Any]:
    failures: dict[str, list[str]] = {rid: [] for rid in OBLIGATIONS}
    corpus: dict[str, dict[str, int]] = {rid: {} for rid in OBLIGATIONS}
    mutants: dict[str, dict[str, int]] = {rid: {"applied": 0, "detected": 0} for rid in OBLIGATIONS}

    def bump(rid: str, key: str, n: int = 1) -> None:
        corpus[rid][key] = corpus[rid].get(key, 0) + n

    def mutate(rid: str, detected: bool, subject: str) -> None:
        mutants[rid]["applied"] += 1
        if detected:
            mutants[rid]["detected"] += 1
        else:
            failures[rid].append(f"mutant on {subject} was not detected: the checker reported the relation preserved")

    def mutate_soft(rid: str, detected: bool) -> None:
        """Like `mutate`, but one miss is not itself a failure: over a tiny
        reachable state space a corrupted value or restored state can coincide with
        the honest one by chance. Only `detected == 0` across the whole corpus
        (checked after the loop) means the checker is vacuous."""
        mutants[rid]["applied"] += 1
        if detected:
            mutants[rid]["detected"] += 1

    for seed in SEEDS:
        base = tsys.generate(seed)
        rb = tsys.explore(base)
        c = tsys.compile_system(base)

        # TEST-3-07: independent-event swap.
        bump("TEST-3-07", "systems")
        for pair in sorted(rb.coenabled_independent):
            bump("TEST-3-07", "independent_pairs_tested")
            state = _find_coenabled_state(c, *pair)
            if state is None:
                continue
            ok, detail = _swap_order(c, state, *pair)
            if not ok:
                failures["TEST-3-07"].append(f"gen-{seed} {pair}: declared-independent pair failed to commute: {detail}")
        for pair in sorted(rb.coenabled_dependent):
            bump("TEST-3-07", "dependent_pairs_tested")
            state = _find_coenabled_state(c, *pair)
            if state is None:
                continue
            ok, _ = _swap_order(c, state, *pair)
            # `coenabled_dependent` means only "not declared independent" (a static
            # under-approximation, docs/02 §3), not "provably order-dependent" — many
            # such pairs still happen to commute. So this is a soft check: only
            # `detected == 0` across the whole corpus (below) means _swap_order never
            # catches a real order-dependent pair, which would make it vacuous.
            mutate_soft("TEST-3-07", not ok)

        # TEST-3-08: equivalent guard normalization.
        normalized = normalize_guard(base)
        ok8, detail8 = _guard_equivalence(base, normalized)
        bump("TEST-3-08", "systems")
        if not ok8:
            failures["TEST-3-08"].append(f"gen-{seed}: {detail8}")
        broken8 = _corrupt_guard_normalize(normalized)
        bok8, _ = _guard_equivalence(base, broken8)
        mutate_soft("TEST-3-08", not bok8)

        # TEST-3-09: serialization round trip.
        bump("TEST-3-09", "systems")
        ok9, detail9 = _round_trip(base)
        if not ok9:
            failures["TEST-3-09"].append(f"gen-{seed}: a correct round trip was itself flagged: {detail9}")
        truncated = tsys.canonical(base)[:-1]
        bok9, _ = _round_trip(base, truncated)
        mutate("TEST-3-09", not bok9, f"gen-{seed} (truncated encoding)")

        # TEST-3-10: snapshot restore versus root replay.
        rng10 = tsys.SplitMix64(seed ^ 0x5310_5310)
        trace = _random_trace(c, rng10, 6)
        bump("TEST-3-10", "systems")
        if len(trace) >= 2:
            checkpoint = 1 + rng10.below(len(trace) - 1)
            bump("TEST-3-10", "checkpointed_traces")
            ok10, detail10 = _snapshot_restore_equivalence(base, trace, checkpoint)
            if not ok10:
                failures["TEST-3-10"].append(f"gen-{seed}: an honest restore was itself flagged: {detail10}")
            snap_state, reason = _walk(c, tsys.initial_state(c), trace[:checkpoint])
            if snap_state is not None:
                corrupted = _corrupt_blob(snap_state)
                bok10, _ = _snapshot_restore_equivalence(base, trace, checkpoint, corrupted)
                mutate_soft("TEST-3-10", not bok10)

    # Real-implementation binding and port/source drift (TEST-3-07, TEST-3-09). These
    # run once against the actual committed files, not per seed: a future edit to
    # continuum-engine-reference or continuum-value is caught the same way a stale
    # evidence file is (README-test.md "retained output").
    for rid, spec in RUST_BINDING.items():
        bump(rid, "rust_tests_bound", len(spec["tests"]))
        for f in _check_rust_binding(rid, _read_source(spec["path"])):
            failures[rid].append(f"rust binding ({spec['path']}): {f['message']}")

    bump("TEST-3-07", "drift_checks", 1)
    for f in _check_independence_drift({"source": _read_source("crates/continuum-engine-reference/src/semantic/oracle.rs")}):
        failures["TEST-3-07"].append(f"drift (crates/continuum-engine-reference/src/semantic/oracle.rs): {f['message']}")

    def need(rid: str, key: str, what: str) -> None:
        if corpus[rid].get(key, 0) == 0:
            failures[rid].append(f"corpus exercises no {what} ({key} = 0): the pass would be vacuous")

    need("TEST-3-07", "independent_pairs_tested", "declared-independent co-enabled pair")
    need("TEST-3-07", "dependent_pairs_tested", "genuinely dependent co-enabled pair")
    need("TEST-3-09", "systems", "system whose canonical encoding was round-tripped")
    need("TEST-3-10", "checkpointed_traces", "trace long enough to take a mid-run snapshot")

    for rid in OBLIGATIONS:
        if mutants[rid]["applied"] and mutants[rid]["detected"] == 0:
            failures[rid].append(f"{rid}: the mutant was never detected across the corpus (the checker would be vacuous)")

    status = {
        "TEST-3-07": "enforced",
        "TEST-3-08": "enforced",
        "TEST-3-09": "enforced",
        "TEST-3-10": "partial",
    }
    absence = {
        "TEST-3-10": "no resume/restore function exists anywhere in continuum-engine-reference (bfs.rs's "
        "Partial::frontier only documents itself as 'a resume point'), and continuumd's restart path is "
        "architecturally the opposite of a root-replay comparison (docs/35: 'MUST NOT reconstruct task state "
        "by inference'); see BOUNDARIES",
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
        if rid in RUST_BINDING:
            entry["rust_binding"] = RUST_BINDING[rid]
        results[rid] = entry
    return {
        "seeds": f"{SEEDS.start}..{SEEDS.stop - 1}",
        "requirements": results,
        "unclaimed": [],
        "out_of_scope": "TEST-3-01..06 are s3_metamorphic_relations.py's (bn-2sn5) module",
    }
