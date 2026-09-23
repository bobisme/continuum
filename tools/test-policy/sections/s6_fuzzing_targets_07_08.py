"""docs/19 §6 "Fuzzing" — TEST-6-07 (replay state machine) and TEST-6-08 (schema
migrations), the two bullets `s6_fuzzing_targets.py` (bn-1zb9) leaves unclaimed —
see that module's own `unclaimed` list and README-test.md "Adding a section". This
file claims both without editing a line of that module, `tsys.py`, or the `Justfile`.

Per README-test.md step 5 and the coordinator's "delivered" bar: an ID is `enforced`
only where the check runs against a real, machine-normative artifact this revision
reads from disk or a real, committed, non-`#[ignore]`d Rust `#[test]` corroborates the
real implementation, checked for presence and non-`#[ignore]`d status by
`_rust_test_present` (textual, no cargo). Where this module's own check is a port of
Rust logic, a run-time drift check ties it back to the real source. An ID with no such
Rust test is `partial`, not enforced — the check still runs, with fixtures, and still
passes with zero failures.

| ID | Real surface enforced | Rust test evidence named | Status |
|---|---|---|---|
| TEST-6-07 | `crates/continuumd/src/daemon/recovery.rs::decode_campaign_record` — the byte-level replay decoder a daemon restart reads its task history back through (plan §4.5 O2, `RecordDefect`) | `recovery.rs`'s `a_record_that_does_not_decode_is_a_typed_defect_per_cause` (malformed-byte classification, the same seven defect classes this module models) and `the_decoder_is_the_inverse_of_the_record_writer` (round trip through the real writer, `budget::publication_record`) | enforced |
| TEST-6-08 | `notes/plan/schemas/README.md`'s schema-epoch-advance rules, modeled standalone and grounded against every real committed `notes/plan/schemas/*.schema.json`'s own `schema_epoch` | none: no crate or schema implements schema-epoch migration; see `ABSENCE` | partial |

# TEST-6-07: what is real, and what is a from-scratch model

`decode_campaign_record` (`crates/continuumd/src/daemon/recovery.rs`) is the function
the startup task-resolution pass (plan §4.5 O2) reads every `task_*` index entry's
committed bytes through before anything is "replayed" into live `TaskEntry` state — the
literal replay-state-machine entry point, and the one surface `docs/19` §6's "replay
state machine" bullet can mean that is not itself declared vocabulary with no producer
(`ErrorCode::ReplayDiverged`, the wire-level `program.replay` operation, has no
`daemon::family::Arguments` variant at all — `crates/continuumd/tests/
inv006_replay_stability_evidence.rs`'s own boundary section says so by name, and no
Rust test anywhere exercises a `program.replay` dispatch for this or any other module to
bind to). `decode_campaign_record` and its `Parts` cursor (both in the same file) are a
strict, length-prefixed, big-endian TLV grammar over exactly seven typed
[`RecordDefect`] classes this module models faithfully in `_decode_campaign_record`: a
Python port, not an execution of the real function (no cargo) — drift-tied by
`_check_replay_grammar_drift`, which re-reads `recovery.rs`'s own defect doc comments
and `protocol/scalar.rs`'s handle-grammar doc comment at run time and fails if the exact
phrases this model is built from are no longer present.

## Boundaries

- **Not modeled: continuation records.** `RecordDefect` also carries `Format`,
  `Vocabulary`, `NonCanonical`, `Ledger`, and `HandleMismatch` — `continuation.rs`'s own
  decoder for the *other* durable record a replay reads (a parked task's resume state).
  Porting that grammar (pins, budget, checkpoints, a canonical-encoding round trip) is
  materially more machinery than the seven campaign-record classes below; it is not
  attempted here, so `HandleMismatch`-class defects are outside this module's coverage.
- **Not modeled: `resolve_records`/`Resolution`.** The higher-level state machine that
  assembles many already-decoded records into one task's `Parked`/`Settled`/`Failed`
  verdict (`each_resolution_rule_fires_on_its_own_input_and_on_no_other`, `the_
  resolution_is_independent_of_input_order`, both real and passing) is a composition
  property over already-well-formed records, not itself a decoder over adversarial
  bytes — docs/19 §6 is the fuzzing section, and this module stays at the byte grain
  that section's other six IDs also occupy.
- **No cargo.** As every ID in the sibling module: a change to the real decoder's
  *shape* (not just its field grammar) would not be caught here, only by the drift
  check noticing its own quoted phrases have moved or gone.

# TEST-6-08: no real implementation exists to bind

`notes/plan/schemas/README.md` ("What advances a schema epoch", "Schema epochs and
compatibility") is real, committed, and normative (INV-003), and states the rules this
module's standalone reference checker enforces: an epoch advance is strictly
increasing and mints exactly the next integer, at most two schema epochs are held
concurrently during a migration, and an advance never mutates a published artifact in
place (ADR-0018) — so a migration chain can never revisit an epoch it already minted.
But — checked directly, mechanically, against the real corpus by
`_check_schema_migration`'s genesis rule — every one of the nineteen real committed
`notes/plan/schemas/*.schema.json` documents declares `"schema_epoch": 1` today; no
schema-epoch advance has ever happened in this repository. `crates/continuum-cml-elab/
src/lib.rs`'s own module doc names "the migration tool (PR 15b)" as future work, not
landed code. `crates/continuumd/tests/gate_g1_01_acceptance.rs` and `gate_g1_04_
acceptance.rs` both state the absence outright in their own words ("No epoch advance is
exercised" / "No daemon here migrates from one epoch to another"). `continuum-value::
epoch::EpochAdvance`/`EpochSet` are real, tested code, but they govern the six
*value* epochs (protocol/semantic/intent/evidence/proof/corpus) the README itself
distinguishes from `schema_epoch` in the same breath ("`schema_epoch` is not a seventh
epoch… MUST NOT be conflated with it"), so binding this ID to them would be exactly the
"analogous, not the real thing" move the coordinator's bar forbids. There is no real
Rust code that validates a schema-epoch migration step, so this ID is `partial`.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

import tsys

SECTION = 6
TITLE = "Fuzzing"

# tools/test-policy/sections/s6_fuzzing_targets_07_08.py -> repo root
ROOT = Path(__file__).resolve().parents[3]
RECOVERY_RS = ROOT / "crates/continuumd/src/daemon/recovery.rs"
SCALAR_RS = ROOT / "crates/continuumd/src/protocol/scalar.rs"
SCHEMAS_DIR = ROOT / "notes/plan/schemas"
SCHEMAS_README = SCHEMAS_DIR / "README.md"

OBLIGATIONS = {
    "TEST-6-07": "replay state machine",
    "TEST-6-08": "schema migrations.",
}

RULES: dict[str, str] = {
    "campaign-record-truncated-detected": "TEST-6-07",
    "campaign-record-trailing-bytes-detected": "TEST-6-07",
    "campaign-record-field-width-detected": "TEST-6-07",
    "campaign-record-task-handle-detected": "TEST-6-07",
    "campaign-record-snapshot-handle-detected": "TEST-6-07",
    "campaign-record-closure-token-detected": "TEST-6-07",
    "campaign-record-frontier-shape-detected": "TEST-6-07",
    "schema-epoch-advance-non-monotonic-detected": "TEST-6-08",
    "schema-epoch-advance-skips-detected": "TEST-6-08",
    "schema-epoch-concurrency-limit-exceeded-detected": "TEST-6-08",
    "schema-epoch-migration-cycle-detected": "TEST-6-08",
    "schema-epoch-migration-genesis-mismatch-detected": "TEST-6-08",
}

_BOUNDARY_07 = (
    "crates/continuumd/src/daemon/recovery.rs's decode_campaign_record is the real "
    "byte-level decoder a daemon restart reads its durable task history through (plan "
    "§4.5 O2) -- the replay-state-machine entry point. This module ports its Parts "
    "cursor and seven RecordDefect classes (Truncated, TrailingBytes, FieldWidth, "
    "TaskHandle, SnapshotHandle, ClosureToken, FrontierShape) faithfully in "
    "_decode_campaign_record, drift-tied to the real defect doc comments and the "
    "protocol_handle grammar doc comment (protocol/scalar.rs) by "
    "_check_replay_grammar_drift. Two real, non-#[ignore]d Rust tests in the same file "
    "exercise the real decoder over matching cases: a_record_that_does_not_decode_is_a_"
    "typed_defect_per_cause (malformed bytes -> typed defect, the seven classes this "
    "module also checks) and the_decoder_is_the_inverse_of_the_record_writer (round "
    "trip through the real writer, budget::publication_record) -- named and "
    "existence-checked below. What this module does NOT model: continuation-record "
    "defects (Format/Vocabulary/NonCanonical/Ledger/HandleMismatch, continuation.rs's "
    "own decoder for a parked task's resume state) and the higher-level "
    "resolve_records/Resolution assembly over already-decoded records (a composition "
    "property, not a byte-grain decoder -- real and tested, but out of this section's "
    "fuzzing scope). No cargo: a shape change to decode_campaign_record itself, not "
    "just its field grammar, is caught only if it moves the quoted doc-comment phrases "
    "the drift check re-reads."
)
_ABSENT_08 = (
    "notes/plan/schemas/README.md's schema-epoch-advance rules ('What advances a "
    "schema epoch', 'Schema epochs and compatibility') are real and normative "
    "(INV-003), and this module's standalone reference checker enforces them, with one "
    "rule (schema-epoch-migration-genesis-mismatch-detected) grounded directly against "
    "every real committed notes/plan/schemas/*.schema.json's own schema_epoch field. "
    "But every one of those nineteen real files declares schema_epoch 1 today -- no "
    "schema-epoch advance has ever happened in this repository -- and no Rust code "
    "validates a migration step: continuum-cml-elab/src/lib.rs names 'the migration "
    "tool (PR 15b)' as unlanded future work, and crates/continuumd/tests/"
    "gate_g1_01_acceptance.rs / gate_g1_04_acceptance.rs both state outright that no "
    "epoch advance is exercised by this daemon. continuum-value::epoch::EpochAdvance "
    "and EpochSet are real, tested Rust code, but they govern the six value epochs "
    "(protocol/semantic/intent/evidence/proof/corpus) the same README explicitly "
    "distinguishes from schema_epoch ('schema_epoch is not a seventh epoch... MUST NOT "
    "be conflated with it'); binding this ID to them would be the analogous-not-real "
    "substitution the coordinator's bar forbids. This ID is partial."
)
BOUNDARIES: dict[str, list[str]] = {
    "TEST-6-07": [_BOUNDARY_07],
    "TEST-6-08": [_ABSENT_08],
}
ABSENCE: dict[str, str] = {
    "TEST-6-08": (
        "no Rust code validates a schema-epoch migration step; continuum-cml-elab's "
        "migration tool (PR 15b) is unlanded and every real schema is still at "
        "schema_epoch 1; see BOUNDARIES"
    ),
}
STATUS: dict[str, str] = {
    "TEST-6-07": "enforced",
    "TEST-6-08": "partial",
}

# ---------------------------------------------------------------------------
# Real Rust #[test] evidence (textual, no cargo, no compiling -- README-test.md,
# coordinator directive on bn-1zb9: a Python port of a Rust predicate is not, by
# itself, evidence about the Rust code).
# ---------------------------------------------------------------------------

RUST_TESTS: dict[str, list[tuple[Path, str]]] = {
    "TEST-6-07": [
        (RECOVERY_RS, "a_record_that_does_not_decode_is_a_typed_defect_per_cause"),
        (RECOVERY_RS, "the_decoder_is_the_inverse_of_the_record_writer"),
    ],
}


def _rust_test_present(path: Path, fn_name: str) -> tuple[bool, str]:
    """Textual, no-cargo check: `fn <fn_name>(` exists in `path`, immediately preceded
    (skipping only blank lines and `//`/`///` comments) by an attribute run that
    includes `#[test]` and no `#[ignore]`. Identical contract to the sibling module's
    own `_rust_test_present` -- not imported from it, per README-test.md's "never edits
    a line another section owns" rule for a new file."""
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


_REPLAY_GRAMMAR_PHRASES = [
    "The bytes end inside a length-prefixed part, or a part is missing.",
    "Bytes remain after the last part the record format declares.",
    "A numeric part does not have its declared width.",
    "The task part is not a well-formed `task_*` handle.",
    "The snapshot part is neither `-` nor a well-formed `ws_*` handle.",
    "The closure part is neither `closed` nor `bounded`.",
    "The frontier component count is not a whole number of states.",
]
_HANDLE_GRAMMAR_PHRASES = [
    "or contains a character outside `[A-Za-z0-9_-]`.",
]
_MIGRATION_GRAMMAR_PHRASES = [
    "MUST advance `schema_epoch`",
    "At most two schema epochs are held",
    "concurrently during a migration",
]


def _check_replay_grammar_drift() -> list[str]:
    """Drift tie for TEST-6-07's Python model: the exact recovery.rs defect doc-comment
    phrases, and the exact scalar.rs handle-grammar phrase, this model is built from
    must still be the real, committed source text."""
    out: list[str] = []
    try:
        recovery_text = RECOVERY_RS.read_text(encoding="utf-8")
    except OSError as exc:
        return [f"could not read {RECOVERY_RS}: {exc}"]
    out.extend(
        f"crates/continuumd/src/daemon/recovery.rs no longer states {phrase!r}: "
        "this module's campaign-record grammar model may have drifted"
        for phrase in _REPLAY_GRAMMAR_PHRASES
        if phrase not in recovery_text
    )
    try:
        scalar_text = SCALAR_RS.read_text(encoding="utf-8")
    except OSError as exc:
        return out + [f"could not read {SCALAR_RS}: {exc}"]
    out.extend(
        f"crates/continuumd/src/protocol/scalar.rs no longer states {phrase!r}: "
        "this module's task_*/ws_* handle grammar model may have drifted"
        for phrase in _HANDLE_GRAMMAR_PHRASES
        if phrase not in scalar_text
    )
    return out


def _check_migration_grammar_drift() -> list[str]:
    """Drift tie for TEST-6-08's standalone reference model: the exact
    schemas/README.md phrases it is built from must still be present."""
    try:
        text = SCHEMAS_README.read_text(encoding="utf-8")
    except OSError as exc:
        return [f"could not read {SCHEMAS_README}: {exc}"]
    return [
        f"notes/plan/schemas/README.md no longer states {phrase!r}: this module's "
        "schema-epoch migration model may have drifted from the real rule"
        for phrase in _MIGRATION_GRAMMAR_PHRASES
        if phrase not in text
    ]


def _finding(rule: str, subject: str, message: str) -> dict[str, str]:
    return tsys.Finding(rule, RULES[rule], subject, message).as_json()


# ---------------------------------------------------------------------------
# TEST-6-07: replay state machine (campaign-record decoder, byte grain).
#
# A faithful port of crates/continuumd/src/daemon/recovery.rs's `Parts` cursor and
# `decode_campaign_record`: every field is an 8-byte-big-endian-length-prefixed part
# (task text, u32 sequence, snapshot text, u64 states, closure text, u64 frontier,
# then zero or more u64 "component" parts). Parsing is strict and left-to-right,
# exactly mirroring the real function's error ordering.
# ---------------------------------------------------------------------------

_OPAQUE_RE = re.compile(r"^[A-Za-z0-9_-]+$")


def _handle_ok(text: str, prefix: str) -> bool:
    if not text.startswith(prefix):
        return False
    rest = text[len(prefix) :]
    return len(rest) > 0 and bool(_OPAQUE_RE.match(rest))


class _Cursor:
    """Mirrors recovery.rs's `Parts<'a>`."""

    def __init__(self, data: bytes) -> None:
        self.data = data

    def is_empty(self) -> bool:
        return len(self.data) == 0

    def next_part(self) -> tuple[bytes | None, str | None]:
        if len(self.data) < 8:
            return None, "truncated"
        width = int.from_bytes(self.data[:8], "big")
        rest = self.data[8:]
        if len(rest) < width:
            return None, "truncated"
        part, rest2 = rest[:width], rest[width:]
        self.data = rest2
        return part, None

    def next_text(self) -> tuple[str | None, str | None]:
        part, err = self.next_part()
        if err is not None or part is None:
            return None, err
        try:
            return part.decode("utf-8"), None
        except UnicodeDecodeError:
            return None, "truncated"

    def next_u64(self) -> tuple[int | None, str | None]:
        part, err = self.next_part()
        if err is not None or part is None:
            return None, err
        if len(part) != 8:
            return None, "field-width"
        return int.from_bytes(part, "big"), None

    def next_u32(self) -> tuple[int | None, str | None]:
        part, err = self.next_part()
        if err is not None or part is None:
            return None, err
        if len(part) != 4:
            return None, "field-width"
        return int.from_bytes(part, "big"), None


def _decode_campaign_record(data: bytes) -> tuple[dict[str, Any] | None, str | None]:
    """Returns `(record, None)` on a clean decode, or `(None, defect_token)` -- the
    first RecordDefect-equivalent token the real decoder would report."""
    cur = _Cursor(data)
    task, err = cur.next_text()
    if err is not None or task is None:
        return None, err
    if not _handle_ok(task, "task_"):
        return None, "task-handle"
    sequence, err = cur.next_u32()
    if err is not None or sequence is None:
        return None, err
    snapshot_text, err = cur.next_text()
    if err is not None or snapshot_text is None:
        return None, err
    if snapshot_text == "-":
        snapshot = None
    else:
        if not _handle_ok(snapshot_text, "ws_"):
            return None, "snapshot-handle"
        snapshot = snapshot_text
    states, err = cur.next_u64()
    if err is not None or states is None:
        return None, err
    closed_text, err = cur.next_text()
    if err is not None or closed_text is None:
        return None, err
    if closed_text == "closed":
        closed = True
    elif closed_text == "bounded":
        closed = False
    else:
        return None, "closure-token"
    frontier, err = cur.next_u64()
    if err is not None or frontier is None:
        return None, err
    components = 0
    while not cur.is_empty():
        _, err = cur.next_u64()
        if err is not None:
            return None, err
        components += 1
    shaped = components == 0 if frontier == 0 else (components >= frontier and components % frontier == 0)
    if not shaped:
        return None, "trailing-bytes" if frontier == 0 else "frontier-shape"
    return {
        "task": task,
        "sequence": sequence,
        "snapshot": snapshot,
        "states": states,
        "closed": closed,
        "frontier": frontier,
        "components": components,
    }, None


def _encode_part(data: bytes) -> bytes:
    return len(data).to_bytes(8, "big") + data


def _encode_text(s: str) -> bytes:
    return _encode_part(s.encode("utf-8"))


def _encode_u32(n: int) -> bytes:
    return _encode_part(n.to_bytes(4, "big"))


def _encode_u64(n: int) -> bytes:
    return _encode_part(n.to_bytes(8, "big"))


def _encode_record(
    task: str, sequence: int, snapshot: str | None, states: int, closed: bool, frontier: int, n_components: int
) -> bytes:
    out = _encode_text(task)
    out += _encode_u32(sequence)
    out += _encode_text(snapshot if snapshot is not None else "-")
    out += _encode_u64(states)
    out += _encode_text("closed" if closed else "bounded")
    out += _encode_u64(frontier)
    for i in range(n_components):
        out += _encode_u64(i)
    return out


_DEFECT_RULE = {
    "truncated": "campaign-record-truncated-detected",
    "trailing-bytes": "campaign-record-trailing-bytes-detected",
    "field-width": "campaign-record-field-width-detected",
    "task-handle": "campaign-record-task-handle-detected",
    "snapshot-handle": "campaign-record-snapshot-handle-detected",
    "closure-token": "campaign-record-closure-token-detected",
    "frontier-shape": "campaign-record-frontier-shape-detected",
}


def _check_campaign_record(payload: dict) -> list[dict[str, str]]:
    data = bytes.fromhex(payload["bytes_hex"])
    _record, defect = _decode_campaign_record(data)
    if defect is None:
        return []
    rule = _DEFECT_RULE.get(defect)
    if rule is None:
        return [_finding("campaign-record-truncated-detected", payload["bytes_hex"][:16], f"unknown defect token {defect!r}")]
    return [
        _finding(
            rule,
            payload["bytes_hex"][:16],
            f"malformed campaign-record bytes classified {defect!r} "
            "(crates/continuumd/src/daemon/recovery.rs RecordDefect, decode_campaign_record)",
        )
    ]


def _gen_campaign_record(seed: int) -> dict[str, Any]:
    frontier = seed % 4
    arity = 1 + (seed // 4) % 3
    n_components = frontier * arity if frontier else 0
    return {
        "task": f"task_t{1 + seed % 5}",
        "sequence": seed % 50,
        "snapshot": None if seed % 3 == 0 else f"ws_s{seed % 7}",
        "states": (seed * 11) % 100000,
        "closed": seed % 2 == 0,
        "frontier": frontier,
        "n_components": n_components,
    }


def _good_bytes(rec: dict[str, Any]) -> bytes:
    return _encode_record(rec["task"], rec["sequence"], rec["snapshot"], rec["states"], rec["closed"], rec["frontier"], rec["n_components"])


def _mutate_campaign_bytes(rec: dict[str, Any], kind: str) -> bytes:
    if kind == "truncated":
        good = _good_bytes(rec)
        return good[: max(0, len(good) - 1)]
    if kind == "trailing-bytes":
        base = _encode_record(rec["task"], rec["sequence"], rec["snapshot"], rec["states"], rec["closed"], 0, 0)
        return base + _encode_u64(1)
    if kind == "field-width":
        # The "sequence" field written at width 8 instead of the declared u32 width 4.
        return _encode_text(rec["task"]) + _encode_part((0).to_bytes(8, "big"))
    if kind == "task-handle":
        return _encode_record("not-a-task", rec["sequence"], rec["snapshot"], rec["states"], rec["closed"], rec["frontier"], rec["n_components"])
    if kind == "snapshot-handle":
        return _encode_record(rec["task"], rec["sequence"], "nope", rec["states"], rec["closed"], rec["frontier"], rec["n_components"])
    if kind == "closure-token":
        out = _encode_text(rec["task"]) + _encode_u32(rec["sequence"])
        out += _encode_text(rec["snapshot"] if rec["snapshot"] is not None else "-")
        out += _encode_u64(rec["states"])
        out += _encode_text("open")  # neither "closed" nor "bounded"
        out += _encode_u64(rec["frontier"])
        for i in range(rec["n_components"]):
            out += _encode_u64(i)
        return out
    if kind == "frontier-shape":
        # frontier=3 but a component count (4) that is not a multiple of it.
        return _encode_record(rec["task"], rec["sequence"], rec["snapshot"], rec["states"], rec["closed"], 3, 4)
    raise ValueError(kind)


# ---------------------------------------------------------------------------
# TEST-6-08: schema migrations (standalone reference model over
# notes/plan/schemas/README.md's schema-epoch-advance rules).
# ---------------------------------------------------------------------------


def _load_json(path: Path) -> Any:
    import json

    return json.loads(path.read_text(encoding="utf-8"))


def _real_schema_epoch(cls: str) -> int | None:
    path = SCHEMAS_DIR / f"{cls}.schema.json"
    if not path.exists():
        return None
    doc = _load_json(path)
    epoch = doc.get("schema_epoch")
    return epoch if isinstance(epoch, int) else None


def _real_schema_classes() -> list[str]:
    return sorted(p.name.removesuffix(".schema.json") for p in SCHEMAS_DIR.glob("*.schema.json"))


def _check_schema_migration(payload: dict) -> list[dict[str, str]]:
    cls = payload["class"]
    advances: list[int] = payload["advances"]
    held: list[int] = payload.get("concurrently_held", [])
    out: list[dict[str, str]] = []

    if len(set(advances)) != len(advances):
        out.append(
            _finding(
                "schema-epoch-migration-cycle-detected",
                cls,
                f"migration chain {advances} for {cls!r} revisits an epoch already minted "
                "(ADR-0018: an epoch advance never mutates a published artifact; re-derived "
                "artifacts get new identities)",
            )
        )
    for i in range(len(advances) - 1):
        f, t = advances[i], advances[i + 1]
        if t <= f:
            out.append(
                _finding(
                    "schema-epoch-advance-non-monotonic-detected",
                    f"{cls}[{i}]",
                    f"advance {f} -> {t} is not strictly increasing (schemas/README.md 'What "
                    "advances a schema epoch': a change to a schema document MUST advance "
                    "schema_epoch)",
                )
            )
        elif t - f > 1:
            out.append(
                _finding(
                    "schema-epoch-advance-skips-detected",
                    f"{cls}[{i}]",
                    f"advance {f} -> {t} skips an epoch with no intermediate document minted",
                )
            )
    if len(set(held)) > 2:
        out.append(
            _finding(
                "schema-epoch-concurrency-limit-exceeded-detected",
                cls,
                f"{len(set(held))} distinct epochs {sorted(set(held))} claimed concurrently "
                "held (schemas/README.md 'Schema epochs and compatibility': at most two "
                "schema epochs are held concurrently during a migration)",
            )
        )
    real = _real_schema_epoch(cls)
    if real is not None and advances and advances[0] != real:
        out.append(
            _finding(
                "schema-epoch-migration-genesis-mismatch-detected",
                cls,
                f"migration chain claims to start at schema_epoch {advances[0]}, but the real "
                f"committed notes/plan/schemas/{cls}.schema.json declares schema_epoch {real} "
                "(INV-003: schemas decide, prose does not)",
            )
        )
    return out


def _gen_migration_chain(seed: int, cls: str, genesis: int) -> dict[str, Any]:
    length = 1 + seed % 3
    advances = [genesis + i for i in range(length)]
    held = advances[-2:] if len(advances) >= 2 else advances[:]
    return {"class": cls, "advances": advances, "concurrently_held": held}


# ---------------------------------------------------------------------------
# Fixtures dispatch.
# ---------------------------------------------------------------------------


def check_fixture(system: Any) -> list[dict[str, str]]:
    if not isinstance(system, dict) or "kind" not in system:
        return [_finding("campaign-record-truncated-detected", "<payload>", "payload must be an object with a 'kind'")]
    kind = system["kind"]
    payload = {k: v for k, v in system.items() if k != "kind"}
    if kind == "campaign-record":
        return _check_campaign_record(payload)
    if kind == "schema-migration":
        return _check_schema_migration(payload)
    return [_finding("campaign-record-truncated-detected", "<payload>", f"unknown fixture kind {kind!r}")]


# ---------------------------------------------------------------------------
# The real run.
# ---------------------------------------------------------------------------


def real_run() -> dict[str, Any]:
    failures: dict[str, list[str]] = {rid: [] for rid in OBLIGATIONS}
    corpus: dict[str, dict[str, int]] = {rid: {} for rid in OBLIGATIONS}
    mutants: dict[str, dict[str, int]] = {rid: {"applied": 0, "detected": 0} for rid in OBLIGATIONS}

    def bump(rid: str, key: str, n: int = 1) -> None:
        corpus[rid][key] = corpus[rid].get(key, 0) + n

    def need(rid: str, key: str, what: str) -> None:
        if corpus[rid].get(key, 0) == 0:
            failures[rid].append(f"corpus exercises no {what} ({key} = 0): the pass would be vacuous")

    def mutate(rid: str, detected: bool, subject: str) -> None:
        mutants[rid]["applied"] += 1
        if detected:
            mutants[rid]["detected"] += 1
        else:
            failures[rid].append(f"mutant on {subject} was not detected: the checker reported it clean")

    # -- TEST-6-07: replay state machine (campaign-record decoder) --------------------
    for seed in range(60):
        rec = _gen_campaign_record(seed)
        bump("TEST-6-07", "generated_records")
        if rec["frontier"] > 0:
            bump("TEST-6-07", "records_with_a_frontier")
        good = _good_bytes(rec)
        record, defect = _decode_campaign_record(good)
        if defect is not None or record is None:
            failures["TEST-6-07"].append(f"seed {seed}: the generator's own well-formed bytes were flagged: {defect}")
        else:
            if record["task"] != rec["task"] or record["sequence"] != rec["sequence"] or record["frontier"] != rec["frontier"]:
                failures["TEST-6-07"].append(f"seed {seed}: decode did not round-trip the generator's fields: {record} != {rec}")
        bump("TEST-6-07", "clean_decodes")
        clean = _check_campaign_record({"bytes_hex": good.hex()})
        if clean:
            failures["TEST-6-07"].append(f"seed {seed}: well-formed bytes were flagged by the checker: {clean[0]['message']}")

        for kind in _DEFECT_RULE:
            malformed = _mutate_campaign_bytes(rec, kind)
            found = _check_campaign_record({"bytes_hex": malformed.hex()})
            got_rules = {f["rule"] for f in found}
            expected_rule = _DEFECT_RULE[kind]
            mutate("TEST-6-07", expected_rule in got_rules, f"seed {seed} ({kind})")
            bump("TEST-6-07", f"mutations_{kind.replace('-', '_')}")

    need("TEST-6-07", "records_with_a_frontier", "generated record with a nonzero frontier")
    for kind in _DEFECT_RULE:
        need("TEST-6-07", f"mutations_{kind.replace('-', '_')}", f"mutation of class {kind!r}")

    # -- Rust test evidence: TEST-6-07 -------------------------------------------------
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

    drift = _check_replay_grammar_drift()
    if drift:
        failures["TEST-6-07"].extend(drift)

    # -- TEST-6-08: schema migrations (standalone reference model) --------------------
    real_classes = _real_schema_classes()
    bump("TEST-6-08", "real_schema_classes", len(real_classes))
    for cls in real_classes:
        genesis = _real_schema_epoch(cls)
        if genesis is None:
            failures["TEST-6-08"].append(f"{cls}: real schema document has no integer schema_epoch")
            continue
        clean = _check_schema_migration({"class": cls, "advances": [genesis], "concurrently_held": [genesis]})
        if clean:
            failures["TEST-6-08"].append(f"{cls}: the real genesis epoch was itself flagged: {clean[0]['message']}")
        bump("TEST-6-08", "genesis_checked")

    for seed in range(20):
        cls = real_classes[seed % len(real_classes)] if real_classes else "unknown"
        genesis = _real_schema_epoch(cls) or 1
        scenario = _gen_migration_chain(seed, cls, genesis)
        bump("TEST-6-08", "generated_chains")
        if len(scenario["advances"]) >= 2:
            bump("TEST-6-08", "multi_step_chains")
        clean = _check_schema_migration(scenario)
        if clean:
            failures["TEST-6-08"].append(f"seed {seed}: the generator's own honest chain {scenario} was flagged: {clean[0]['message']}")

        # Mutant: non-monotonic advance (repeat the genesis epoch).
        bad_monotonic = {"class": cls, "advances": scenario["advances"] + [scenario["advances"][-1]], "concurrently_held": [genesis]}
        found = _check_schema_migration(bad_monotonic)
        mutate("TEST-6-08", any(f["rule"] in ("schema-epoch-advance-non-monotonic-detected", "schema-epoch-migration-cycle-detected") for f in found), f"seed {seed} (non-monotonic/repeat)")
        bump("TEST-6-08", "mutations_monotonic")

        # Mutant: skip an epoch.
        bad_skip = {"class": cls, "advances": [genesis, genesis + 2], "concurrently_held": [genesis]}
        found = _check_schema_migration(bad_skip)
        mutate("TEST-6-08", any(f["rule"] == "schema-epoch-advance-skips-detected" for f in found), f"seed {seed} (skip)")
        bump("TEST-6-08", "mutations_skip")

        # Mutant: three epochs concurrently held.
        bad_concurrency = {"class": cls, "advances": [genesis, genesis + 1, genesis + 2], "concurrently_held": [genesis, genesis + 1, genesis + 2]}
        found = _check_schema_migration(bad_concurrency)
        mutate("TEST-6-08", any(f["rule"] == "schema-epoch-concurrency-limit-exceeded-detected" for f in found), f"seed {seed} (concurrency)")
        bump("TEST-6-08", "mutations_concurrency")

        # Mutant: genesis mismatch against the real committed schema.
        bad_genesis = {"class": cls, "advances": [genesis + 5], "concurrently_held": [genesis + 5]}
        found = _check_schema_migration(bad_genesis)
        mutate("TEST-6-08", any(f["rule"] == "schema-epoch-migration-genesis-mismatch-detected" for f in found), f"seed {seed} (genesis)")
        bump("TEST-6-08", "mutations_genesis")

    need("TEST-6-08", "genesis_checked", "real committed schema class checked at its own declared genesis epoch")
    need("TEST-6-08", "multi_step_chains", "generated migration chain with more than one advance")
    for key in ("mutations_monotonic", "mutations_skip", "mutations_concurrency", "mutations_genesis"):
        need("TEST-6-08", key, f"mutation of class {key!r}")

    drift = _check_migration_grammar_drift()
    if drift:
        failures["TEST-6-08"].extend(drift)

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
        "unclaimed": [],
    }
