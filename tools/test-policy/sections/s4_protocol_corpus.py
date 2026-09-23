"""docs/19 §4 "Protocol corpus" subsection — TEST-4-13 … TEST-4-18 (the last six of
its nine bullets: non-idempotent retry, ack-before-durability, forgotten loser drain,
timeout via silent drop, restart timer leak, recovery livelock). The first three
("all first-demo mutants", "stale term/epoch", "double counting", TEST-4-10..12) and
the "Semantic engine" subsection's TEST-4-07..09 are bn-3mo3's parallel module; see
`unclaimed` in the evidence file. `sections/s4_mutation_testing.py` (bn-1ccq) claims
TEST-4-01..06 and is untouched here.

# Real substance: the daemon protocol layer, not a fresh model

The "Semantic engine" mutants (bn-1ccq) had `tsys` (a real, already-relied-upon
harness oracle) and `continuum-engine-reference`'s `semantic::defect` module (a real
Rust crate seeding the exact same mutant classes) to bind against. Neither exists for
the *protocol* layer's six mutants this module claims. What does exist, and is real
and load-bearing rather than illustrative, is `crates/continuumd`: unlike the crates
plan §20/AGENTS.md call "documented stubs", `continuumd`'s daemon module
(`src/daemon/{state,task,continuation,recovery,obligation,errors}.rs`, several
thousand lines) already implements the idempotency ledger, cancel/drain/finalize,
restart resume, and epoch admissibility this section's mutants attack, each with its
own doc comment quoting the IDL `rule` it discharges. This Python, no-cargo harness
(README-test.md) cannot execute that crate. Where a check below runs a **faithful,
cited port** of one of its functions — not merely an analogous model — the docstring
says so explicitly and names the function; where it does not, and the concept is
still checked as a general, IDL-wide contract with no protocol-specific production
mechanism to bind the mutant to, the ID is `partial` and the absence is stated, the
same discipline `s4_mutation_testing.py` established for TEST-4-04..06.

- **TEST-4-13** (non-idempotent retry). `rule idempotency.replay` (the IDL) is a
  direct, named match, and `crates/continuumd/src/daemon/state.rs`'s `ReplayKey`
  type states — field by field, with its own citation of the rule's "byte-identical
  canonical request" — exactly which seven of `RequestEnvelope`'s thirteen fields
  the comparison is over. The check below is that same seven-field comparison.
  `enforced`.
- **TEST-4-14** (ack-before-durability). `ErrorCode::PublicationAborted`'s doc
  comment and INV-017 ("the artifact whose publication aborted is not published")
  are the IDL's atomicity contract; `crates/continuumd/src/daemon/task.rs`'s
  `reported()` states the production invariant this ID's mutant would break —
  "an *uncommitted* [publication] cannot reach [`ResultEnvelope.artifacts`],
  because `Publications::artifacts` reads the committed list and a staged
  publication is not in it" — and `cancel()`'s own doc comment (bn-20142) records
  the fix that made durability precede the status write for exactly this reason.
  `enforced`.

  # Lead review (this bone): killing a Python port's mutant is not enough

  Killing a mutant of a Python port shows the *port* catches the mutant class; it
  does not show the *daemon* does. For TEST-4-13, TEST-4-14, and TEST-4-17 — the
  three IDs this module marks `enforced` — `real_run()` now additionally (a) names
  and verifies, by reading the committed test file's text (no cargo), that the real
  `crates/continuumd` test(s) that exercise this mutant class exist, are `#[test]`,
  are not `#[ignore]`d, and still assert on the cited behaviour (`RUST_BINDING`,
  `_check_rust_binding`); and (c) re-derives each port's field list from the live
  Rust source and fails if it has diverged from the Python constant
  (`_check_replay_key_drift`, `_check_admissible_epochs_drift`,
  `_check_committed_only_drift`). Each obligation's `BOUNDARIES` entry below names
  the specific tests and drift anchors. Had no real test existed for an ID's mutant
  class, that ID would be downgraded to `partial` here — none needed it.
- **TEST-4-15** (forgotten loser drain). `rule task.cancel_correct` is real and its
  single-trigger discipline (`Residual::admit`'s per-publication Reserve-then-Commit
  accounting) is real, tested production code. But the *race* this ID's mutant name
  presupposes — two triggers competing to finalize one task, a winner and a loser —
  has no live counterpart today: `task.cancel`'s own doc comment states plainly that
  "when this operation runs there is no *live* execution of the task to interrupt"
  (a dispatch holds `&mut DaemonState` for its whole extent, so the campaign already
  finalized its own region before `task.cancel` is even called). Every plan-dossier
  mention of "race loser(s) drain" (`notes/plan/docs/06_RESEARCH_AGENDA.md` §E3,
  `notes/plan/docs/07_BENCHMARKS_AND_EVALUATION.md` Suite A, and
  `notes/plan/research/09-cancellation-and-obligation-calculus.md`'s "winner returns
  before loser drains") is in a research lane explicitly marked draft, pending
  ratification (docs/06 §"Promotion and kill criteria (draft)") — not production
  surface. The check below extends the real single-trigger discipline to a race the
  current architecture cannot yet run. `partial`.
- **TEST-4-16** (timeout via silent drop). INV-008, `rule envelope.epochs_named`
  ("An unsupported or empty task still returns a valid machine result … and carries
  no success flag") and `rule task.status_monotonic`'s "there is no silent failure"
  are real and general. But the specific mechanism this ID's mutant needs — a timer
  that elapses and is (mis)handled by dropping the response — has nothing to bind
  to: `verification.start`'s `timeout_ms` field is declared on the wire, but
  `crates/continuumd/src/daemon/verification.rs`'s own doc comment on
  `result`/`await` states "there is no interval for a timeout to elapse over and
  `timeout_ms` has nothing to bound" — the daemon does not implement timeouts yet.
  `partial`.
- **TEST-4-17** (restart timer leak). `ErrorCode::ContinuationEpochMismatch`'s doc
  comment is the IDL's own words for this exact mutant: "The daemon MUST NOT
  silently re-run." `crates/continuumd/src/daemon/task.rs::admissible_epochs` is the
  real, tested, two-predicate (P1 compatibility epochs, P2 engine identity) function
  that decides it on every `task.resume`. The check below is a direct line-by-line
  port of that function's match arms, cited by name, not an analogy. `enforced`.
- **TEST-4-18** (recovery livelock). `rule task.status_monotonic`'s "reported
  milestones and committed evidence only grow" is real and is what the check below
  enforces over a bounded resume window. But "livelock" itself — non-termination
  under repeated recovery attempts — is explicitly a *draft* mutation class of a
  liveness engine that does not exist yet: `notes/plan/research/
  04-liveness-progress-and-fairness.md`'s "Testing the liveness engine" section
  lists "crash/recovery livelock" among mutation classes for a not-yet-built
  differential-liveness harness, under "Promotion and kill criteria (draft)". No
  bounded-retry cap or progress-ranking mechanism exists in `crates/continuumd`'s
  `recovery.rs`/`task.rs` to bind this ID's mutant to. `partial`.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

import tsys

SECTION = 4
TITLE = "Mutation testing"
OBLIGATIONS = {
    "TEST-4-13": "non-idempotent retry",
    "TEST-4-14": "ack-before-durability",
    "TEST-4-15": "forgotten loser drain",
    "TEST-4-16": "timeout via silent drop",
    "TEST-4-17": "restart timer leak",
    "TEST-4-18": "recovery livelock.",
}
RULES: dict[str, str] = {
    "idempotent-replay-diverged": "TEST-4-13",
    "idempotent-replay-rust-binding-missing": "TEST-4-13",
    "idempotent-replay-key-drift": "TEST-4-13",
    "ack-before-durability-detected": "TEST-4-14",
    "ack-durability-rust-binding-missing": "TEST-4-14",
    "ack-durability-committed-drift": "TEST-4-14",
    "cancel-loser-drain-forgotten": "TEST-4-15",
    "timeout-silent-drop-detected": "TEST-4-16",
    "restart-timer-leak-detected": "TEST-4-17",
    "restart-timer-rust-binding-missing": "TEST-4-17",
    "restart-timer-epochs-drift": "TEST-4-17",
    "recovery-livelock-detected": "TEST-4-18",
}
SEEDS = range(64)
LIVELOCK_BOUND = 6  # a bounded resume window, docs/19 §2's own "for small sizes,
# enumerate" convention; an exceeded bound without progress is the livelock this
# checker can decide (INV-008: a bound, never an unbounded wait, decides "stuck").

# The seven fields state.rs's `ReplayKey` carries into the comparison, exactly as its
# doc-comment table spells them; `request_id`, `idempotency_key`, `actor`, `capability`,
# `protocol_version`, and `trace` are the six it deliberately omits. Also the expected
# struct-field order the (c) drift check below re-derives from the live source.
REPLAY_KEY_FIELDS = ("operation", "snapshot", "intent", "arguments", "budget", "output_policy", "page")

# The five PinnedEpochs compatibility kinds, in task.rs's own `compatibility()` order,
# plus `engine` (P2, checked separately — "provenance, not a seventh epoch"). Also the
# expected field order the (c) drift check below re-derives from the live source.
COMPAT_KINDS = ("semantic", "intent", "evidence", "proof", "corpus")

_REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent  # tools/test-policy/sections/../../../..


def _read_source(rel_path: str) -> str | None:
    try:
        return (_REPO_ROOT / rel_path).read_text(encoding="utf-8")
    except OSError:
        return None


# ---------------------------------------------------------------------------
# Real-implementation binding (lead review, bn-2dt2): killing a mutant of a Python
# port does not show the daemon's real code catches that mutant class. For each ID
# this module marks `enforced`, `RUST_BINDING` names the real `crates/continuumd`
# test(s) that exercise the real code path against this mutant class. `check_fixture`'s
# "rust-binding-*" kinds and `real_run()` both call `_check_rust_binding` — against,
# respectively, a fixture's synthetic source text and the real committed file — so the
# same logic both demonstrates the mutation detection and runs for real. It verifies,
# textually (no cargo): the named test exists, is `#[test]`, is not `#[ignore]`d, and
# its body still contains the token that names the assertion this ID's mutant class
# would break — not just any `assert!`.
# ---------------------------------------------------------------------------

RUST_BINDING: dict[str, dict[str, Any]] = {
    "TEST-4-13": {
        "path": "crates/continuumd/tests/gate_g1_03_acceptance.rs",
        "tests": {
            # a byte-identical replay of every reachable @mutation operation returns the
            # recorded outcome — the direct kill test for "non-idempotent retry".
            "sweep_every_reachable_mutation_is_idempotent_under_one_key": "outcome_equal",
            # the same key with a genuinely different request is IdempotencyKeyReused.
            "sweep_one_key_with_a_different_request_is_refused_and_changes_nothing": "IdempotencyKeyReused",
        },
    },
    "TEST-4-14": {
        "path": "crates/continuumd/tests/gate_g1_06_acceptance.rs",
        "tests": {
            # every abort reason at every phase leaves the published namespace untouched —
            # nothing becomes visible (ack-able) short of a full commit.
            "every_abort_reason_at_every_phase_leaves_the_published_namespace_byte_identical": "PublicationAborted",
            # the visible set only ever grows at a completed commit, never ahead of one.
            "the_visible_set_is_a_monotone_prefix_of_the_complete_publication": "growth_points",
        },
    },
    "TEST-4-17": {
        "path": "crates/continuumd/tests/gate_g1_04_acceptance.rs",
        "tests": {
            # one #[test] per P1 compatibility kind, named after COMPAT_KINDS itself so
            # this binding cannot silently drift from the five kinds the port iterates.
            **{f"class_p1_{kind}_epoch_disagreement": "ContinuationEpochMismatch" for kind in COMPAT_KINDS},
            # P2: engine identity.
            "class_p2_engine_identity_disagreement": "ContinuationEpochMismatch",
            # the literal restart scenario this ID's mutant names ("a timer from an old
            # epoch fires in a new process"): a continuation survives a real daemon crash
            # and rebuild with its pins unchanged, so a later resume re-checks them, not a
            # value the restart silently forgot.
            "regression_a_continuation_survives_a_restart_with_its_pins": "before.pinned",
        },
    },
}
BINDING_RULE = {
    "TEST-4-13": "idempotent-replay-rust-binding-missing",
    "TEST-4-14": "ack-durability-rust-binding-missing",
    "TEST-4-17": "restart-timer-rust-binding-missing",
}


def _find_fn(source: str, name: str) -> tuple[str, str] | None:
    """`(prelude, body)` for a zero-argument `fn name() { ... }` in `source`: `prelude` is
    the two source lines immediately above it (where `#[test]`/`#[ignore]` live on every
    named test — checked, not assumed: see the self-test fixtures), `body` is the
    balanced, brace-matched function body. `None` if `name` is not declared this way."""
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
        found = _find_fn(source, name)
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
                _finding(rule, name, f"`{name}` no longer asserts on {token!r} — it may no longer kill {rid}'s mutant class").as_json()
            )
    return findings


def _check_rust_binding_4_13(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"source"}):
        return [_finding("idempotent-replay-rust-binding-missing", "<payload>", "payload must have exactly source").as_json()]
    return _check_rust_binding("TEST-4-13", payload["source"])


def _check_rust_binding_4_14(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"source"}):
        return [_finding("ack-durability-rust-binding-missing", "<payload>", "payload must have exactly source").as_json()]
    return _check_rust_binding("TEST-4-14", payload["source"])


def _check_rust_binding_4_17(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"source"}):
        return [_finding("restart-timer-rust-binding-missing", "<payload>", "payload must have exactly source").as_json()]
    return _check_rust_binding("TEST-4-17", payload["source"])


# ---------------------------------------------------------------------------
# Port/source drift (lead review, bn-2dt2): each port above must not silently diverge
# from the real struct or function it claims to mirror.
# ---------------------------------------------------------------------------


def _check_replay_key_drift(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"source"}) or not isinstance(payload["source"], str):
        return [_finding("idempotent-replay-key-drift", "<payload>", "payload must have exactly source (a string)").as_json()]
    m = re.search(r"pub struct ReplayKey \{(.*?)\n\}", payload["source"], re.S)
    if m is None:
        return [_finding("idempotent-replay-key-drift", "state.rs", "struct ReplayKey was not found in the given source").as_json()]
    fields = tuple(re.findall(r"pub (\w+):", m.group(1)))
    if fields != REPLAY_KEY_FIELDS:
        return [
            _finding(
                "idempotent-replay-key-drift",
                "ReplayKey",
                f"state.rs's ReplayKey field order is now {fields}, but the Python port's REPLAY_KEY_FIELDS is "
                f"{REPLAY_KEY_FIELDS} — the two have diverged and TEST-4-13's port no longer mirrors real code",
            ).as_json()
        ]
    return []


def _check_admissible_epochs_drift(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"source"}) or not isinstance(payload["source"], str):
        return [_finding("restart-timer-epochs-drift", "<payload>", "payload must have exactly source (a string)").as_json()]
    m = re.search(r"fn compatibility\(&self\)[^{]*\{(.*?)\n    \}", payload["source"], re.S)
    if m is None:
        return [_finding("restart-timer-epochs-drift", "task.rs", "PinnedEpochs::compatibility() was not found in the given source").as_json()]
    kinds = tuple(re.findall(r"&self\.(\w+)", m.group(1)))
    if kinds != COMPAT_KINDS:
        return [
            _finding(
                "restart-timer-epochs-drift",
                "PinnedEpochs::compatibility",
                f"task.rs's compatibility() now returns {kinds}, but the Python port's COMPAT_KINDS is "
                f"{COMPAT_KINDS} — the two have diverged and TEST-4-17's port no longer mirrors real code",
            ).as_json()
        ]
    return []


def _check_committed_only_drift(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"source"}) or not isinstance(payload["source"], str):
        return [_finding("ack-durability-committed-drift", "<payload>", "payload must have exactly source (a string)").as_json()]
    m = re.search(r"pub fn artifacts\(&self, task: &TaskHandle\)[^{]*\{(.*?)\n    \}", payload["source"], re.S)
    if m is None:
        return [_finding("ack-durability-committed-drift", "budget.rs", "Publications::artifacts() was not found in the given source").as_json()]
    body = m.group(1)
    if "self.committed" not in body or "self.staged" in body:
        return [
            _finding(
                "ack-durability-committed-drift",
                "Publications::artifacts",
                "budget.rs's Publications::artifacts() no longer reads only `self.committed` (or now also reads "
                "`self.staged`) — TEST-4-14's committed-only-visible citation no longer mirrors real code",
            ).as_json()
        ]
    return []


_BOUNDARY_13 = (
    "rule idempotency.replay (the IDL) plus crates/continuumd/src/daemon/state.rs's "
    "ReplayKey doc comment, which states field-by-field which seven of "
    "RequestEnvelope's thirteen fields ('operation', 'snapshot', 'intent', "
    "'arguments', 'budget', 'output_policy', 'page') the byte-identical-request "
    "comparison covers, and which six it deliberately excludes ('request_id', "
    "'idempotency_key', 'actor', 'capability', 'protocol_version', 'trace'). The "
    "check below is that same seven-field comparison, not a smaller stand-in. Killing a "
    "mutant of this Python port does not by itself show the real daemon catches that "
    "mutant class, so real_run() also (a) verifies "
    "crates/continuumd/tests/gate_g1_03_acceptance.rs's "
    "sweep_every_reachable_mutation_is_idempotent_under_one_key and "
    "sweep_one_key_with_a_different_request_is_refused_and_changes_nothing exist, are "
    "#[test], are not #[ignore]d, and still assert on 'outcome_equal'/"
    "'IdempotencyKeyReused' respectively (RUST_BINDING, _check_rust_binding), and (c) "
    "re-derives ReplayKey's live field order from state.rs and fails on any divergence "
    "from REPLAY_KEY_FIELDS (_check_replay_key_drift)."
)
_BOUNDARY_14 = (
    "ErrorCode::PublicationAborted's doc comment and INV-017 (the IDL) state the "
    "atomicity contract; crates/continuumd/src/daemon/task.rs's reported() states "
    "the production invariant in these words: an uncommitted publication cannot "
    "reach ResultEnvelope.artifacts, because Publications::artifacts reads only the "
    "committed list. cancel()'s own doc comment (bn-20142) records publishing the "
    "durable continuation record before the status write moves, for exactly this "
    "reason. The check below enforces that same commit-before-ack ordering. Killing a "
    "mutant of this Python port does not by itself show the real daemon catches that "
    "mutant class, so real_run() also (a) verifies "
    "crates/continuumd/tests/gate_g1_06_acceptance.rs's "
    "every_abort_reason_at_every_phase_leaves_the_published_namespace_byte_identical "
    "and the_visible_set_is_a_monotone_prefix_of_the_complete_publication exist, are "
    "#[test], are not #[ignore]d, and still assert on 'PublicationAborted'/"
    "'growth_points' respectively (RUST_BINDING, _check_rust_binding), and (c) "
    "re-derives Publications::artifacts()'s live body from "
    "crates/continuumd/src/daemon/budget.rs and fails if it stops reading only "
    "self.committed (_check_committed_only_drift)."
)
_BOUNDARY_15 = (
    "rule task.cancel_correct (the IDL) and Residual::admit's real per-publication "
    "Reserve-then-Commit accounting (crates/continuumd/src/daemon/task.rs) are the "
    "single-trigger mechanism this extends. task.cancel's own doc comment states "
    "there is no live execution of a task to interrupt at this daemon grain (a "
    "dispatch holds &mut DaemonState for its whole extent and the campaign's region "
    "is already finalized before task.cancel runs), so a genuine two-trigger race "
    "has no live counterpart today. Every plan-dossier 'race loser(s) drain' "
    "mention (docs/06 §E3, docs/07 Suite A, research/09's 'winner returns before "
    "loser drains') is an explicitly draft, pending-ratification research-lane "
    "mutation class, not production surface. The check below models the invariant "
    "a future concurrent grain would have to uphold, extending Residual::admit's "
    "real discipline to a second, losing trigger."
)
_BOUNDARY_16 = (
    "INV-008, rule envelope.epochs_named ('An unsupported or empty task still "
    "returns a valid machine result … and carries no success flag'), and rule "
    "task.status_monotonic's 'there is no silent failure' are real and general. "
    "verification.start's timeout_ms field is declared on the wire, but "
    "crates/continuumd/src/daemon/verification.rs's own doc comment on the "
    "result/await handler states plainly: 'there is no interval for a timeout to "
    "elapse over and timeout_ms has nothing to bound' — no timeout mechanism is "
    "implemented yet for this check's mutant to be seeded into."
)
_BOUNDARY_17 = (
    "ErrorCode::ContinuationEpochMismatch's doc comment is the IDL's own words for "
    "this mutant: 'The daemon MUST NOT silently re-run.' The check below is a "
    "direct, cited port of crates/continuumd/src/daemon/task.rs::admissible_epochs "
    "— the real, tested two-predicate function (P1: the five PinnedEpochs "
    "compatibility kinds 'semantic', 'intent', 'evidence', 'proof', 'corpus'; P2: "
    "engine identity) that decides every task.resume's epoch admissibility — not an "
    "analogous model of it. Killing a mutant of this Python port does not by itself "
    "show the real daemon catches that mutant class, so real_run() also (a) verifies "
    "crates/continuumd/tests/gate_g1_04_acceptance.rs's five "
    "class_p1_<kind>_epoch_disagreement tests (one per COMPAT_KINDS member), "
    "class_p2_engine_identity_disagreement, and "
    "regression_a_continuation_survives_a_restart_with_its_pins — the literal restart "
    "scenario this ID's mutant names — all exist, are #[test], are not #[ignore]d, and "
    "still assert on 'ContinuationEpochMismatch'/'before.pinned' respectively "
    "(RUST_BINDING, _check_rust_binding), and (c) re-derives "
    "PinnedEpochs::compatibility()'s live field order from task.rs and fails on any "
    "divergence from COMPAT_KINDS (_check_admissible_epochs_drift)."
)
_BOUNDARY_18 = (
    "rule task.status_monotonic's 'reported milestones and committed evidence only "
    "grow' (the IDL) is real and is what the check below enforces over a bounded "
    "resume window. 'Livelock' itself is explicitly a draft mutation class "
    "('crash/recovery livelock') of a not-yet-built liveness engine "
    "(notes/plan/research/04-liveness-progress-and-fairness.md, 'Testing the "
    "liveness engine', under 'Promotion and kill criteria (draft)'). No "
    "bounded-retry cap or progress-ranking mechanism exists in "
    "crates/continuumd/src/daemon/{recovery,task}.rs to bind this mutant to."
)

BOUNDARIES: dict[str, list[str]] = {
    "TEST-4-13": [_BOUNDARY_13],
    "TEST-4-14": [_BOUNDARY_14],
    "TEST-4-15": [_BOUNDARY_15],
    "TEST-4-16": [_BOUNDARY_16],
    "TEST-4-17": [_BOUNDARY_17],
    "TEST-4-18": [_BOUNDARY_18],
}


def _finding(rule: str, subject: str, message: str) -> tsys.Finding:
    return tsys.Finding(rule, RULES[rule], subject, message)


# ---------------------------------------------------------------------------
# TEST-4-13: non-idempotent retry — rule idempotency.replay
# ---------------------------------------------------------------------------

# REPLAY_KEY_FIELDS is defined above, beside the RUST_BINDING/drift infrastructure it
# also grounds.


def _replay_key(request: Any) -> str:
    if not isinstance(request, dict):
        return tsys.canonical(None)
    return tsys.canonical({k: request.get(k) for k in REPLAY_KEY_FIELDS})


def _check_idempotent_replay(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"attempts"}) or not isinstance(payload["attempts"], list):
        return [_finding("idempotent-replay-diverged", "<payload>", "payload must have exactly attempts (a list)").as_json()]
    ledger: dict[tuple[str, str], tuple[str, Any]] = {}
    findings = []
    for i, attempt in enumerate(payload["attempts"]):
        if not (isinstance(attempt, dict) and {"actor", "idempotency_key", "request", "outcome_identity"} <= set(attempt)):
            findings.append(
                _finding("idempotent-replay-diverged", f"attempt {i}", "each attempt needs actor/idempotency_key/request/outcome_identity").as_json()
            )
            continue
        key = (attempt["actor"], attempt["idempotency_key"])
        rk = _replay_key(attempt["request"])
        outcome = attempt["outcome_identity"]
        if key not in ledger:
            ledger[key] = (rk, outcome)
            continue
        prev_rk, prev_outcome = ledger[key]
        if rk == prev_rk and outcome != prev_outcome:
            findings.append(
                _finding(
                    "idempotent-replay-diverged",
                    f"{attempt['actor']}/{attempt['idempotency_key']}#{i}",
                    f"a byte-identical replay (same ReplayKey: operation/snapshot/intent/arguments/budget/"
                    f"output_policy/page) returned outcome identity {outcome!r}, not the first attempt's "
                    f"{prev_outcome!r} — rule idempotency.replay requires the same task or artifact identity",
                ).as_json()
            )
    return findings


# ---------------------------------------------------------------------------
# TEST-4-14: ack-before-durability — INV-017, ErrorCode::PublicationAborted, reported()
# ---------------------------------------------------------------------------


def _check_ack_durability(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"events"}) or not isinstance(payload["events"], list):
        return [_finding("ack-before-durability-detected", "<payload>", "payload must have exactly events (a list)").as_json()]
    committed: set[str] = set()
    findings = []
    for i, ev in enumerate(payload["events"]):
        if not isinstance(ev, dict) or "type" not in ev:
            findings.append(_finding("ack-before-durability-detected", f"event {i}", "each event needs a type").as_json())
            continue
        kind = ev["type"]
        if kind == "commit":
            committed.add(ev.get("artifact"))
        elif kind == "reserve":
            pass  # reserved only: NOT in the committed set (Publications::artifacts reads only committed)
        elif kind == "ack":
            if ev.get("status") in ("ok", "task_started", "task_suspended"):
                for art in ev.get("artifacts", []):
                    if art not in committed:
                        findings.append(
                            _finding(
                                "ack-before-durability-detected",
                                f"event {i} artifact {art!r}",
                                "a ResultEnvelope named this artifact in `artifacts` before any `commit` event "
                                "recorded it durable — ack-before-durability (INV-017; reported()'s "
                                "committed-publications-only contract)",
                            ).as_json()
                        )
        else:
            findings.append(_finding("ack-before-durability-detected", f"event {i}", f"unknown event type {kind!r}").as_json())
    return findings


# ---------------------------------------------------------------------------
# TEST-4-15: forgotten loser drain — rule task.cancel_correct, Residual::admit (extended)
# ---------------------------------------------------------------------------


def _check_cancel_drain_race(payload: Any) -> list[dict]:
    need = {"publications_winner", "drained_winner", "publications_loser", "drained_loser"}
    if not (isinstance(payload, dict) and set(payload) == need):
        return [_finding("cancel-loser-drain-forgotten", "<payload>", f"payload must have exactly {sorted(need)}").as_json()]
    pl, dl = payload["publications_loser"], payload["drained_loser"]
    if not (isinstance(pl, int) and isinstance(dl, int) and pl >= 0 and 0 <= dl <= pl):
        return [_finding("cancel-loser-drain-forgotten", "<payload>", "publications_loser/drained_loser must be 0 <= drained <= publications").as_json()]
    if dl < pl:
        return [
            _finding(
                "cancel-loser-drain-forgotten",
                f"loser: {dl}/{pl} drained",
                f"the race loser reserved+committed {pl} publication(s) but only {dl} were drained/admitted "
                "before the task reached its terminal cancellation outcome — a forgotten loser drain "
                "(rule task.cancel_correct extended to a two-trigger race; Residual::admit's real "
                "per-publication Reserve-then-Commit discipline is the single-trigger mechanism this models)",
            ).as_json()
        ]
    return []


# ---------------------------------------------------------------------------
# TEST-4-16: timeout via silent drop — INV-008, envelope.epochs_named, status_monotonic
# ---------------------------------------------------------------------------


def _check_timeout_drop(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"requests"}) or not isinstance(payload["requests"], list):
        return [_finding("timeout-silent-drop-detected", "<payload>", "payload must have exactly requests (a list)").as_json()]
    findings = []
    for i, r in enumerate(payload["requests"]):
        if not (isinstance(r, dict) and {"request_id", "responded"} <= set(r)):
            findings.append(_finding("timeout-silent-drop-detected", f"request {i}", "each request needs request_id/responded").as_json())
            continue
        if not r["responded"] or r.get("response_status") is None:
            timeout_ms, elapsed_ms = r.get("timeout_ms"), r.get("elapsed_ms")
            timed_out = isinstance(timeout_ms, int) and isinstance(elapsed_ms, int) and elapsed_ms >= timeout_ms
            findings.append(
                _finding(
                    "timeout-silent-drop-detected",
                    str(r["request_id"]),
                    "no typed ResultEnvelope was produced for this request "
                    + ("after its timeout_ms elapsed" if timed_out else "(no response at all)")
                    + " — a silent drop, forbidden by INV-008 and rule envelope.epochs_named's "
                    "'a valid machine result … carries no success flag' (never nothing)",
                ).as_json()
            )
    return findings


# ---------------------------------------------------------------------------
# TEST-4-17: restart timer leak — ContinuationEpochMismatch, admissible_epochs (ported)
# ---------------------------------------------------------------------------

# COMPAT_KINDS is defined above, beside the RUST_BINDING/drift infrastructure it also
# grounds.


def _admissible_epochs(pinned: dict, current: dict) -> str:
    """A direct, line-by-line port of crates/continuumd/src/daemon/task.rs's
    `admissible_epochs(pinned: &PinnedEpochs, current: &EpochSet)`: P1 over the five
    compatibility kinds, then P2 over `engine`. Returns "ok",
    "EpochUnsupported", or "ContinuationEpochMismatch" — the same three outcomes
    the real function's `Result<(), Fault>` encodes."""
    for kind in COMPAT_KINDS:
        want = pinned.get(kind)
        if want is None:
            continue
        have = current.get(kind)
        if have is None:
            return "EpochUnsupported"
        if want != have:
            return "ContinuationEpochMismatch"
    want_engine = pinned.get("engine")
    if want_engine is None:
        return "ok"
    have_engine = current.get("engine")
    if want_engine == have_engine:
        return "ok"
    return "ContinuationEpochMismatch"


def _check_restart_epoch(payload: Any) -> list[dict]:
    need = {"pinned", "current", "executed"}
    if not (isinstance(payload, dict) and set(payload) == need and isinstance(payload["pinned"], dict) and isinstance(payload["current"], dict)):
        return [_finding("restart-timer-leak-detected", "<payload>", f"payload must have exactly {sorted(need)}").as_json()]
    verdict = _admissible_epochs(payload["pinned"], payload["current"])
    if verdict != "ok" and payload["executed"]:
        return [
            _finding(
                "restart-timer-leak-detected",
                f"pinned={payload['pinned']} current={payload['current']}",
                f"admissible_epochs found {verdict} but the continuation/timer effect ran anyway — a "
                "silent re-run across a restart's epoch advance (rule: 'The daemon MUST NOT silently "
                "re-run', ErrorCode::ContinuationEpochMismatch/EpochUnsupported)",
            ).as_json()
        ]
    return []


# ---------------------------------------------------------------------------
# TEST-4-18: recovery livelock — rule task.status_monotonic (bounded window)
# ---------------------------------------------------------------------------

TERMINAL_STATUSES = frozenset({"completed", "failed", "cancelled"})


def _check_recovery_resume(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"attempts"}) or not isinstance(payload["attempts"], list) or not payload["attempts"]:
        return [_finding("recovery-livelock-detected", "<payload>", "payload must have exactly a non-empty attempts list").as_json()]
    attempts = payload["attempts"]
    for i, a in enumerate(attempts):
        if not (isinstance(a, dict) and {"milestones", "evidence", "status"} <= set(a)):
            return [_finding("recovery-livelock-detected", f"attempt {i}", "each attempt needs milestones/evidence/status").as_json()]
    progressed = any(
        attempts[i]["milestones"] > attempts[i - 1]["milestones"] or attempts[i]["evidence"] > attempts[i - 1]["evidence"]
        for i in range(1, len(attempts))
    )
    terminal_reached = attempts[-1]["status"] in TERMINAL_STATUSES
    if not progressed and not terminal_reached and len(attempts) >= LIVELOCK_BOUND:
        return [
            _finding(
                "recovery-livelock-detected",
                f"{len(attempts)} attempts",
                f"{len(attempts)} resume attempts (>= the {LIVELOCK_BOUND}-attempt bound) made no monotonic "
                "progress (milestones/evidence never grew) and never reached a terminal status — recovery "
                "livelock (rule task.status_monotonic: 'reported milestones and committed evidence only grow')",
            ).as_json()
        ]
    return []


# ---------------------------------------------------------------------------
# Fixtures dispatch
# ---------------------------------------------------------------------------

_KIND_CHECKERS = {
    "idempotent-replay": _check_idempotent_replay,
    "ack-durability": _check_ack_durability,
    "cancel-drain-race": _check_cancel_drain_race,
    "timeout-drop": _check_timeout_drop,
    "restart-epoch": _check_restart_epoch,
    "recovery-resume": _check_recovery_resume,
    "rust-binding-4-13": _check_rust_binding_4_13,
    "rust-binding-4-14": _check_rust_binding_4_14,
    "rust-binding-4-17": _check_rust_binding_4_17,
    "replay-key-drift": _check_replay_key_drift,
    "admissible-epochs-drift": _check_admissible_epochs_drift,
    "committed-only-drift": _check_committed_only_drift,
}


def check_fixture(system: Any) -> list[dict[str, str]]:
    if not isinstance(system, dict) or system.get("kind") not in _KIND_CHECKERS:
        return [_finding("idempotent-replay-diverged", "<payload>", "payload must have a known 'kind'").as_json()]
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

    for seed in SEEDS:
        rng = tsys.SplitMix64(seed)

        # TEST-4-13: non-idempotent retry.
        op = rng.choice(["task.cancel", "verification.start", "workspace.create", "evidence.link"])
        base_request = {
            "operation": op,
            "snapshot": rng.choice([None, "ws_1", "ws_2"]),
            "intent": rng.choice([None, "intent_1"]),
            "arguments": {"n": rng.below(10)},
            "budget": rng.choice([None, {"states": rng.below(100)}]),
            "output_policy": rng.choice([None, {"max_bytes": rng.below(1000)}]),
            "page": rng.choice([None, {"token": "p1"}]),
        }
        n_attempts = 1 + rng.below(3)
        actor, key, outcome = f"actor-{rng.below(4)}", f"key-{rng.below(1000)}", f"outcome-{rng.below(1000)}"
        attempts = [
            {"actor": actor, "idempotency_key": key, "request": dict(base_request, **{"request_id": f"req-{i}"}), "outcome_identity": outcome}
            for i in range(n_attempts)
        ]
        bump("TEST-4-13", "systems")
        clean13 = _check_idempotent_replay({"attempts": attempts})
        if clean13:
            failures["TEST-4-13"].append(f"gen-{seed}: a correct replay ledger was itself flagged: {clean13[0]['message']}")
        if n_attempts > 1:
            bump("TEST-4-13", "multi_attempt_groups")
            mutated = [dict(a) for a in attempts]
            mutated[-1] = dict(mutated[-1], outcome_identity=f"outcome-mutant-{seed}")
            mut13 = _check_idempotent_replay({"attempts": mutated})
            mutate("TEST-4-13", True, bool(mut13), f"gen-{seed} (retry #{n_attempts - 1} returned a different outcome identity)")

        # TEST-4-14: ack-before-durability.
        n_artifacts = 1 + rng.below(3)
        artifacts = [f"art-{seed}-{i}" for i in range(n_artifacts)]
        events = []
        for art in artifacts:
            events.append({"type": "reserve", "artifact": art})
            events.append({"type": "commit", "artifact": art})
        events.append({"type": "ack", "status": "ok", "artifacts": artifacts})
        bump("TEST-4-14", "systems")
        bump("TEST-4-14", "artifacts", n_artifacts)
        clean14 = _check_ack_durability({"events": events})
        if clean14:
            failures["TEST-4-14"].append(f"gen-{seed}: commit-before-ack was itself flagged: {clean14[0]['message']}")
        early_ack = events[:-2] + [events[-1], events[-2]]  # ack moved before the last artifact's commit
        mut14 = _check_ack_durability({"events": early_ack})
        mutate("TEST-4-14", True, bool(mut14), f"gen-{seed} (ack moved before {artifacts[-1]}'s commit)")

        # TEST-4-15: forgotten loser drain.
        pw, pl = rng.below(6), rng.below(6)
        bump("TEST-4-15", "systems")
        bump("TEST-4-15", "loser_publications", pl)
        clean15 = _check_cancel_drain_race({"publications_winner": pw, "drained_winner": pw, "publications_loser": pl, "drained_loser": pl})
        if clean15:
            failures["TEST-4-15"].append(f"gen-{seed}: fully-drained winner and loser were themselves flagged: {clean15[0]['message']}")
        if pl > 0:
            bump("TEST-4-15", "systems_with_a_loser_to_forget")
            mut15 = _check_cancel_drain_race({"publications_winner": pw, "drained_winner": pw, "publications_loser": pl, "drained_loser": pl - 1})
            mutate("TEST-4-15", True, bool(mut15), f"gen-{seed} (loser drained {pl - 1}/{pl})")

        # TEST-4-16: timeout via silent drop.
        n_requests = 1 + rng.below(4)
        requests = []
        for i in range(n_requests):
            timeout_ms = 100 + rng.below(900)
            elapsed_ms = rng.below(1200)
            status = rng.choice(["ok", "error", "task_started"])
            requests.append(
                {"request_id": f"req-{seed}-{i}", "timeout_ms": timeout_ms, "elapsed_ms": elapsed_ms, "responded": True, "response_status": status}
            )
        bump("TEST-4-16", "systems")
        bump("TEST-4-16", "requests", n_requests)
        clean16 = _check_timeout_drop({"requests": requests})
        if clean16:
            failures["TEST-4-16"].append(f"gen-{seed}: fully-answered requests were themselves flagged: {clean16[0]['message']}")
        timed_out = [i for i, r in enumerate(requests) if r["elapsed_ms"] >= r["timeout_ms"]]
        if timed_out:
            bump("TEST-4-16", "systems_with_a_timed_out_request")
            victim = timed_out[0]
            dropped = [dict(r) for r in requests]
            dropped[victim] = dict(dropped[victim], responded=False, response_status=None)
            mut16 = _check_timeout_drop({"requests": dropped})
            mutate("TEST-4-16", True, bool(mut16), f"gen-{seed} (request {victim} silently dropped after timeout)")

        # TEST-4-17: restart timer leak.
        pinned = {k: rng.choice([None, f"e{rng.below(3)}"]) for k in COMPAT_KINDS}
        pinned["engine"] = rng.choice([None, f"eng{rng.below(2)}"])
        current = {k: rng.choice([None, f"e{rng.below(3)}"]) for k in COMPAT_KINDS}
        current["engine"] = rng.choice([None, f"eng{rng.below(2)}"])
        verdict = _admissible_epochs(pinned, current)
        bump("TEST-4-17", "systems")
        clean17 = _check_restart_epoch({"pinned": pinned, "current": current, "executed": verdict == "ok"})
        if clean17:
            failures["TEST-4-17"].append(f"gen-{seed}: a daemon that only executes when admissible was itself flagged: {clean17[0]['message']}")
        if verdict != "ok":
            bump("TEST-4-17", "systems_forcing_a_mismatch")
            mut17 = _check_restart_epoch({"pinned": pinned, "current": current, "executed": True})
            mutate("TEST-4-17", True, bool(mut17), f"gen-{seed} (verdict {verdict}, executed anyway)")

        # TEST-4-18: recovery livelock.
        length = 1 + rng.below(6)
        milestones, evidence = rng.below(3), rng.below(3)
        progressing = []
        for i in range(length):
            if i > 0 and rng.below(2):
                milestones += 1 + rng.below(2)
                evidence += rng.below(2)
            status = "completed" if i == length - 1 else rng.choice(["running", "suspended"])
            progressing.append({"milestones": milestones, "evidence": evidence, "status": status})
        bump("TEST-4-18", "systems")
        clean18 = _check_recovery_resume({"attempts": progressing})
        if clean18:
            failures["TEST-4-18"].append(f"gen-{seed}: a progressing-then-terminal resume sequence was itself flagged: {clean18[0]['message']}")
        frozen = [dict(progressing[0], status="suspended") for _ in range(LIVELOCK_BOUND)]
        bump("TEST-4-18", "frozen_windows_at_bound", 1)
        mut18 = _check_recovery_resume({"attempts": frozen})
        mutate("TEST-4-18", True, bool(mut18), f"gen-{seed} ({LIVELOCK_BOUND} frozen, non-terminal resume attempts)")

    # Real-implementation binding and port/source drift (lead review, bn-2dt2). These run
    # once against the actual committed files, not per-seed: they check the repository's
    # current state, not a generated payload, so a future edit to continuumd or to the
    # cited test files is caught the same way a stale evidence file is (README-test.md
    # "retained output").
    for rid, drift_check, drift_path in (
        ("TEST-4-13", _check_replay_key_drift, "crates/continuumd/src/daemon/state.rs"),
        ("TEST-4-14", _check_committed_only_drift, "crates/continuumd/src/daemon/budget.rs"),
        ("TEST-4-17", _check_admissible_epochs_drift, "crates/continuumd/src/daemon/task.rs"),
    ):
        bump(rid, "drift_checks", 1)
        for f in drift_check({"source": _read_source(drift_path)}):
            failures[rid].append(f"drift ({drift_path}): {f['message']}")

    for rid, spec in RUST_BINDING.items():
        bump(rid, "rust_tests_bound", len(spec["tests"]))
        for f in _check_rust_binding(rid, _read_source(spec["path"])):
            failures[rid].append(f"rust binding ({spec['path']}): {f['message']}")

    def need(rid: str, key: str, what: str) -> None:
        if corpus[rid].get(key, 0) == 0:
            failures[rid].append(f"corpus exercises no {what} ({key} = 0): the pass would be vacuous")

    need("TEST-4-13", "multi_attempt_groups", "idempotency key replayed more than once")
    need("TEST-4-15", "systems_with_a_loser_to_forget", "cancel race with residual loser publications")
    need("TEST-4-16", "systems_with_a_timed_out_request", "request whose elapsed_ms reached its timeout_ms")
    need("TEST-4-17", "systems_forcing_a_mismatch", "pinned/current epoch pair that disagrees")

    for rid in OBLIGATIONS:
        if mutants[rid]["applied"] and mutants[rid]["detected"] == 0:
            failures[rid].append(f"{rid}: the mutant was never detected across the corpus (the checker would be vacuous)")

    status = {
        "TEST-4-13": "enforced",
        "TEST-4-14": "enforced",
        "TEST-4-15": "partial",
        "TEST-4-16": "partial",
        "TEST-4-17": "enforced",
        "TEST-4-18": "partial",
    }
    absence = {
        "TEST-4-15": "no live concurrent execution exists at the daemon dispatch grain to race a second "
        "cancel trigger against (task.cancel's own doc comment); the race-loser mutation classes in the "
        "plan dossier are all draft, pending-ratification research lanes; see BOUNDARIES",
        "TEST-4-16": "verification.rs documents that timeout_ms is not wired to any interval yet ('nothing "
        "to bound'); no timeout mechanism exists in continuumd for this mutant to be seeded into; see "
        "BOUNDARIES",
        "TEST-4-18": "'crash/recovery livelock' is a draft, not-yet-ratified mutation class of a liveness "
        "engine that does not exist; no bounded-retry or progress-ranking mechanism exists in "
        "recovery.rs/task.rs; see BOUNDARIES",
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
        "livelock_bound": LIVELOCK_BOUND,
        "requirements": results,
        "unclaimed": [],
        "out_of_scope": "TEST-4-07..09 (Semantic engine remainder) and TEST-4-10..12 (Protocol corpus, "
        "first three) are bn-3mo3's parallel module",
    }
