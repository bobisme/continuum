"""docs/19 §4 "Mutation testing", "Semantic engine" subsection — TEST-4-01 … TEST-4-06.

The subsection text lists nine mutation operators for the semantic engine; this
module claims the first six: drop causal edge; declare conflicting events
independent; skip obligation discharge; use hash equality only; ignore epoch;
map submitted to stable. (TEST-4-07..09 — accept unknown field as old meaning,
omit fairness edge, use timestamp total order — and the whole "Protocol
corpus" subsection, TEST-4-10..18, are left to a later module; see
`unclaimed` in the evidence file.)

docs/19 §4: "A mature suite has kill expectations per engine. Not every engine
must kill every mutant, but the assurance result cannot overclaim." This
module states, per ID, exactly which of the two substances this harness can
run against actually models the concept the mutant targets, and marks an ID
delivered only when one does:

- `tsys.py` (bn-1fqu's oracle, shared ground per README-test.md) already
  computes a genuine semantic-dependence/causal order (`tsys.trace_class`,
  used by TEST-2-04's `reduction-agrees`), a conflict/independence
  well-formedness check (`conflict-not-independent`, TEST-2-05), and an
  exhaustive exact-identity reachable-state set (`tsys.explore`'s `seen`,
  backing TEST-2's whole oracle). TEST-4-01, -02, -03 are the corresponding
  mutants of those three real mechanisms, so this module runs them directly
  against `tsys`'s own logic — no second system shape is defined
  (README-test.md "Adding a section").
- `continuum-engine-reference`'s `semantic::defect` module (bn-1zgs) already
  seeds exactly TEST-4-02's mutant (`DefectClass::Dependence`) and TEST-4-03's
  (`DefectClass::Obligation`) into the reference engine's own model, giving
  those two IDs independent Rust-side corroboration that the concept is real
  production surface, not just a harness convention. This Python, no-cargo
  driver (README-test.md) cannot execute that crate; `defect.rs`'s doc
  comment is cited, not run.
- TEST-4-01's concept (a causal/happens-before edge between dependent trace
  occurrences) has no counterpart in `defect.rs` — `continuum-cir`, where a
  real CIR causal order would eventually live, is still a documented
  scaffold (`crates/continuum-cir/src/lib.rs`; also noted for the RFC 0028
  causal-slicing row in `notes/plan/notes/G0_SPIKE_MATRIX.md`) — but `tsys`'s
  own dependence/trace-class machinery is real, already-relied-upon harness
  substance, the same bar TEST-2-04 met. TEST-4-01 is therefore claimed
  `enforced`.

TEST-4-04, -05, -06 have no counterpart in either substance:

- **TEST-4-04** ("use hash equality only") is ADR-0013's own subject
  ("Hash compaction and fingerprints are fast but collisions can suppress
  states... proof validity still derives from decoded structure"), and
  `tsys.explore` already IS an exact-identity oracle (it dedups reachable
  states with a plain Python `set` of exact state tuples, never a hash). But
  neither `tsys` nor `defect.rs` expose a *mutable* state-identity policy to
  seed this defect into — there is nothing in either substance whose hash-vs-
  exact comparison this module can flip and re-run. The check below is a
  freestanding, mathematically forced demonstration (a deliberately
  small-range digest over the oracle's own real reachable-state sets,
  guaranteeing a pigeonhole collision), not a mutation of running harness or
  engine logic.
- **TEST-4-05** ("ignore epoch") is real, seriously-typed production surface
  (`crates/continuum-value/src/epoch.rs`, ADR-0018's six independently
  versioned epochs, "MUST NOT be conflated" enforced by distinct newtypes)
  — but `tsys` systems carry no epoch field, `defect.rs` seeds no epoch
  mutant, and this Python harness cannot call the Rust epoch types. The
  check below is a standalone model of the single hazard common to every
  non-ordered epoch (a content-only comparison silently drops the epoch
  component), not a test of `epoch.rs` itself.
- **TEST-4-06** ("map submitted to stable") is grounded in a real declared
  artifact — `notes/plan/schemas/examples/storage-pack.manifest.json`'s
  `effect_phases` (`Requested, Reserved, Submitted, Stable, Acknowledged,
  Aborted`) and its `storage/append-log-v0` profile ("sync promotes a prefix
  to stable", fault `volatile-suffix-loss`) — but the crate that profile
  names (`continuum-pack-storage`) does not exist in `crates/` at all: there
  is no adapter, stub or otherwise, to enforce against. The check below
  models the phase lifecycle and durability predicate the manifest declares,
  in isolation.

Per README-test.md step 5, these three are therefore claimed with status
`partial` and are NOT annotated `(delivered: …)` on their docs/19 bullets,
even though their fixtures, corpus and mutation-kill checks below all pass
with zero failures (the harness's own positive-enforcement gate, which does
not distinguish `enforced` from `partial` — see `check_test_policy.py`
STATUSES). The absence is stated in each entry, not just in this docstring.
"""

from __future__ import annotations

import copy
import hashlib
from typing import Any

import tsys

SECTION = 4
TITLE = "Mutation testing"
OBLIGATIONS = {
    "TEST-4-01": "drop causal edge",
    "TEST-4-02": "declare conflicting events independent",
    "TEST-4-03": "skip obligation discharge",
    "TEST-4-04": "use hash equality only",
    "TEST-4-05": "ignore epoch",
    "TEST-4-06": "map submitted to stable",
}
RULES: dict[str, str] = {
    "causal-edge-drop-detected": "TEST-4-01",
    "conflict-declared-independent-detected": "TEST-4-02",
    "obligation-discharge-skip-detected": "TEST-4-03",
    "hash-identity-collision-detected": "TEST-4-04",
    "epoch-ignored-detected": "TEST-4-05",
    "submitted-mapped-to-stable-detected": "TEST-4-06",
}
SEEDS = range(128)
HASH_BUCKETS = 4  # deliberately narrow: forces a pigeonhole collision whenever a
# system's exact reachable-state count exceeds it (see TEST-4-04 above)

_ENGINE_BOUNDARY_01 = (
    "continuum-cir, where a real CIR causal order would eventually live, is still a "
    "documented scaffold (crates/continuum-cir/src/lib.rs); no engine exists yet to "
    "compare against. This harness's own tsys.trace_class dependence/causal-order "
    "computation (already relied on by TEST-2-04's reduction-agrees rule) is the "
    "substance enforced here, the same bar TEST-2-04 met."
)
_ENGINE_BOUNDARY_02 = (
    "continuum-engine-reference's semantic::defect module independently seeds this "
    "exact mutant (DefectClass::Dependence, 'declare conflicting events independent') "
    "into the reference engine's own model (crates/continuum-engine-reference/src/"
    "semantic/defect.rs). This Python, no-cargo harness (README-test.md) does not "
    "execute that crate; it is cited as corroborating production surface, not run. "
    "Enforcement here is tsys.check_static's own conflict-not-independent rule."
)
_ENGINE_BOUNDARY_03 = (
    "continuum-engine-reference's semantic::defect module independently seeds this "
    "exact mutant (DefectClass::Obligation, 'skip obligation discharge') into the "
    "reference engine's own model (crates/continuum-engine-reference/src/semantic/"
    "defect.rs). This Python, no-cargo harness does not execute that crate; it is "
    "cited as corroborating production surface, not run. Enforcement here is "
    "tsys.explore's own obligation-leak rule, on a generator-guaranteed-clean system "
    "whose sole discharge site is made permanently unreachable."
)
_ABSENT_04 = (
    "Neither tsys nor continuum-engine-reference's semantic::defect exposes a "
    "mutable state-identity policy for this module to flip and re-run — tsys.explore "
    "is already exact-identity by construction (a plain Python set of exact state "
    "tuples), never mutated here. The check below is a freestanding, mathematically "
    "forced demonstration over tsys's own real reachable-state sets: a deliberately "
    "narrow (4-bucket) SHA-256 reduction, which the pigeonhole principle guarantees "
    "collides whenever a system's exact state count exceeds 4 (true of nearly every "
    "corpus system). It is not a mutation of any running harness or engine logic, "
    "which is why the ID is claimed `partial`, not `enforced` for delivery purposes "
    "(README-test.md step 5), despite the check itself passing with zero failures. "
    "ADR-0013 ('Use exact canonical state identity in exhaustive and certified "
    "modes') is the production decision this stands in for."
)
_ABSENT_05 = (
    "crates/continuum-value/src/epoch.rs is real, tested production surface (six "
    "independently versioned epoch newtypes, ADR-0018, 'MUST NOT be conflated' "
    "enforced by the type system), but tsys systems carry no epoch field, "
    "continuum-engine-reference's semantic::defect seeds no epoch mutant, and this "
    "Python harness cannot call the Rust epoch types (no cargo). The check below "
    "models only the single hazard common to every non-ordered epoch: a "
    "content-only comparison that silently drops the epoch component and reports "
    "two artifacts from different epochs equal. It is a standalone model, not a "
    "test of epoch.rs, hence `partial` rather than `enforced` for delivery."
)
_ABSENT_06 = (
    "notes/plan/schemas/examples/storage-pack.manifest.json declares the "
    "Requested/Reserved/Submitted/Stable/Acknowledged/Aborted effect-phase lifecycle "
    "and the storage/append-log-v0 profile's 'sync promotes a prefix to stable' "
    "assumption plus its volatile-suffix-loss fault, but the crate that manifest "
    "names as the adapter (continuum-pack-storage) does not exist anywhere under "
    "crates/ — not even as a stub. The check below models the declared phase "
    "lifecycle and durability predicate in isolation; there is no engine, stub or "
    "otherwise, to enforce it against, hence `partial` rather than `enforced`."
)

BOUNDARIES: dict[str, list[str]] = {
    "TEST-4-01": [_ENGINE_BOUNDARY_01],
    "TEST-4-02": [_ENGINE_BOUNDARY_02],
    "TEST-4-03": [_ENGINE_BOUNDARY_03],
    "TEST-4-04": [_ABSENT_04],
    "TEST-4-05": [_ABSENT_05],
    "TEST-4-06": [_ABSENT_06],
}


def _finding(rule: str, subject: str, message: str) -> tsys.Finding:
    return tsys.Finding(rule, RULES[rule], subject, message)


# ---------------------------------------------------------------------------
# TEST-4-01: drop causal edge
# ---------------------------------------------------------------------------


def _run_greedy(base: dict) -> tuple[tsys.Compiled, list[str]]:
    """One concrete maximal interleaving word: at each reachable state, fire
    the lexicographically smallest enabled transition (`c.transitions` is
    already sorted by name) until quiescent. A pure function of `base`, no
    randomness — the fixture only needs *some* real fired trace, not every
    one."""
    c = tsys.compile_system(base)
    state = tsys.initial_state(c)
    word: list[str] = []
    for _ in range(10_000):  # generated systems are tiny; a guard, not a real bound
        enabled = [t for t in c.transitions if tsys.enabled(c, state, t)]
        if not enabled:
            break
        t = enabled[0]
        nxt, findings = tsys.fire(c, state, t)
        if nxt is None or findings:
            break
        word.append(t["name"])
        state = nxt
    return c, word


def _true_edges(word: list[str], independent: set[tuple[str, str]]) -> list[tuple]:
    return list(tsys.trace_class(word, independent)[1])


def _edge_to_json(edge: tuple) -> list[list]:
    return [list(edge[0]), list(edge[1])]


def _edge_from_json(raw: Any) -> tuple | None:
    if not (isinstance(raw, list) and len(raw) == 2):
        return None
    a, b = raw
    if not (isinstance(a, list) and len(a) == 2 and isinstance(b, list) and len(b) == 2):
        return None
    return ((a[0], a[1]), (b[0], b[1]))


def _check_causal_edge(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"system", "word", "claimed_edges"}):
        return [_finding("causal-edge-drop-detected", "<payload>", "payload must have exactly system/word/claimed_edges").as_json()]
    system, word, raw_edges = payload["system"], payload["word"], payload["claimed_edges"]
    static = tsys.check_static(system)
    if static or not isinstance(word, list) or not isinstance(raw_edges, list):
        return [f.as_json() for f in static] or [
            _finding("causal-edge-drop-detected", "<payload>", "word/claimed_edges must be lists").as_json()
        ]
    c = tsys.compile_system(system)
    true_edges = set(_true_edges(word, c.independent))
    claimed = set()
    for raw in raw_edges:
        edge = _edge_from_json(raw)
        if edge is not None:
            claimed.add(edge)
    missing = sorted(true_edges - claimed)
    return [
        _finding(
            "causal-edge-drop-detected",
            f"{edge[0][0]}#{edge[0][1]}->{edge[1][0]}#{edge[1][1]}",
            "the recorded trace has a causal edge between two dependent occurrences that the claimed edge set omits",
        ).as_json()
        for edge in missing
    ]


# ---------------------------------------------------------------------------
# TEST-4-02: declare conflicting events independent
# ---------------------------------------------------------------------------


def _check_conflict_independent(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"system"}):
        return [_finding("conflict-declared-independent-detected", "<payload>", "payload must have exactly system").as_json()]
    static = tsys.check_static(payload["system"])
    hits = [f for f in static if f.rule == "conflict-not-independent"]
    return [
        _finding(
            "conflict-declared-independent-detected",
            f.subject,
            f"tsys.check_static's own conflict-not-independent rule confirms a declared conflict pair is also declared independent: {f.message}",
        ).as_json()
        for f in hits
    ]


def _declare_conflict_independent(base: dict, pair: tuple[str, str]) -> dict:
    m = copy.deepcopy(base)
    m["independent"] = m["independent"] + [[pair[0], pair[1]]]
    return m


# ---------------------------------------------------------------------------
# TEST-4-03: skip obligation discharge
# ---------------------------------------------------------------------------


def _pc_var(process: str) -> str:
    return f"pc_{process}"


def _find_discharge_site(base: dict) -> tuple[str, str, int] | None:
    """`(process, transition name, guard constant)` for one obligation whose
    sole discharge transition is the LAST step of its owning process, and
    whose guard is a plain `pc == k` equality — the shape `tsys.generate`'s
    acquire/discharge window always produces at the window's own end. Bumping
    that one constant by 1 makes the transition permanently unreachable
    (nothing else ever advances that process's own program counter past its
    last declared step), so the process gets stuck holding the obligation
    forever: a reachable, quiescent leak — exactly `defect.rs`'s Obligation
    class ('the owner can reach Cancelled while the obligation is open'),
    without touching the static acquire/discharge declaration TEST-2-06
    checks (the discharge is still declared; it just never fires)."""
    for obl in base["obligations"]:
        owner = obl["owner"]
        discharge_t = next((t for t in base["transitions"] if obl["name"] in t.get("discharge", [])), None)
        if discharge_t is None:
            continue
        proc_steps = [t for t in base["transitions"] if t["process"] == owner]
        last_step = max(int(t["name"].rsplit(".s", 1)[1]) for t in proc_steps)
        step = int(discharge_t["name"].rsplit(".s", 1)[1])
        if step != last_step:
            continue
        guard = discharge_t.get("guard")
        if not (isinstance(guard, dict) and guard.get("op") == "eq"):
            continue
        args = guard.get("args", [])
        if len(args) != 2 or args[0] != {"var": _pc_var(owner)} or "const" not in args[1]:
            continue
        return owner, discharge_t["name"], args[1]["const"]
    return None


def _skip_discharge(base: dict, tname: str, const: int) -> dict:
    m = copy.deepcopy(base)
    for t in m["transitions"]:
        if t["name"] == tname:
            t["guard"]["args"][1]["const"] = const + 1
    return m


def _check_obligation_discharge(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"system"}):
        return [_finding("obligation-discharge-skip-detected", "<payload>", "payload must have exactly system").as_json()]
    findings, _report = tsys.check(payload["system"])
    hits = [f for f in findings if f.rule == "obligation-leak"]
    return [
        _finding(
            "obligation-discharge-skip-detected",
            f.subject,
            f"tsys.explore's own obligation-leak rule confirms a reachable terminal state leaks an obligation: {f.message}",
        ).as_json()
        for f in hits
    ]


# ---------------------------------------------------------------------------
# TEST-4-04: use hash equality only
# ---------------------------------------------------------------------------


def _hash_bucket(state: Any, buckets: int) -> int:
    digest = hashlib.sha256(tsys.canonical(state).encode()).digest()
    return int.from_bytes(digest, "big") % buckets


def _check_hash_identity(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"states", "buckets"}):
        return [_finding("hash-identity-collision-detected", "<payload>", "payload must have exactly states/buckets").as_json()]
    states, buckets = payload["states"], payload["buckets"]
    if not isinstance(states, list):
        return [_finding("hash-identity-collision-detected", "<payload>", "states must be a list").as_json()]
    exact = {tsys.canonical(s) for s in states}
    if buckets is None:
        return []  # the canonical (exact) identity control: no reduction applied
    hashed = {_hash_bucket(s, buckets) for s in states}
    if len(hashed) < len(exact):
        return [
            _finding(
                "hash-identity-collision-detected",
                f"buckets={buckets}",
                f"{len(exact)} exactly-distinct states collapse to {len(hashed)} hash bucket(s): "
                "hash equality alone would silently merge distinct states (ADR-0013)",
            ).as_json()
        ]
    return []


# ---------------------------------------------------------------------------
# TEST-4-05: ignore epoch
# ---------------------------------------------------------------------------


def _epoch_identity(entry: dict) -> tuple:
    return (entry["epoch"], tsys.canonical(entry["body"]))


def _epoch_ignoring_identity(entry: dict) -> str:
    return tsys.canonical(entry["body"])


def _check_epoch(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"left", "right"}):
        return [_finding("epoch-ignored-detected", "<payload>", "payload must have exactly left/right").as_json()]
    left, right = payload["left"], payload["right"]
    if not (isinstance(left, dict) and set(left) == {"epoch", "body"} and isinstance(right, dict) and set(right) == {"epoch", "body"}):
        return [_finding("epoch-ignored-detected", "<payload>", "left/right must each have exactly epoch/body").as_json()]
    true_equal = _epoch_identity(left) == _epoch_identity(right)
    buggy_equal = _epoch_ignoring_identity(left) == _epoch_ignoring_identity(right)
    if buggy_equal and not true_equal:
        return [
            _finding(
                "epoch-ignored-detected",
                f"epoch {left['epoch']!r} vs {right['epoch']!r}",
                "a content-only comparison reports two artifacts equal across a differing epoch, "
                "which ADR-0018 requires kept distinct (crates/continuum-value/src/epoch.rs)",
            ).as_json()
        ]
    return []


# ---------------------------------------------------------------------------
# TEST-4-06: map submitted to stable
# ---------------------------------------------------------------------------

PHASES = ["Requested", "Reserved", "Submitted", "Stable", "Acknowledged"]
_DURABLE_PHASES = {"Stable", "Acknowledged"}
ENTRIES = range(32)


def _is_durable(phase: str, policy: str) -> bool:
    if policy == "correct":
        return phase in _DURABLE_PHASES
    if policy == "buggy":
        return phase in (_DURABLE_PHASES | {"Submitted"})
    raise ValueError(f"unknown policy {policy!r}")


def _check_submitted_stable(payload: Any) -> list[dict]:
    if not (isinstance(payload, dict) and set(payload) == {"phase", "policy"}):
        return [_finding("submitted-mapped-to-stable-detected", "<payload>", "payload must have exactly phase/policy").as_json()]
    phase, policy = payload["phase"], payload["policy"]
    if phase not in PHASES or policy not in ("correct", "buggy"):
        return [_finding("submitted-mapped-to-stable-detected", str(phase), "unknown phase or policy").as_json()]
    claimed = _is_durable(phase, policy)
    survives = phase in _DURABLE_PHASES
    if claimed and not survives:
        return [
            _finding(
                "submitted-mapped-to-stable-detected",
                phase,
                f"policy {policy!r} claims phase {phase!r} durable, but the "
                "storage/append-log-v0 profile's volatile-suffix-loss fault "
                "(schemas/examples/storage-pack.manifest.json) loses an entry before "
                "sync promotes its prefix to Stable",
            ).as_json()
        ]
    return []


# ---------------------------------------------------------------------------
# Fixtures dispatch
# ---------------------------------------------------------------------------

_KIND_CHECKERS = {
    "causal-edge": _check_causal_edge,
    "conflict-independent": _check_conflict_independent,
    "obligation-discharge": _check_obligation_discharge,
    "hash-identity": _check_hash_identity,
    "epoch": _check_epoch,
    "submitted-stable": _check_submitted_stable,
}


def check_fixture(system: Any) -> list[dict[str, str]]:
    if not isinstance(system, dict) or system.get("kind") not in _KIND_CHECKERS:
        return [_finding("causal-edge-drop-detected", "<payload>", "payload must have a known 'kind'").as_json()]
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
        base = tsys.generate(seed)
        static = tsys.check_static(base)
        if static:
            failures["TEST-4-01"].append(f"gen-{seed}: unexpected static finding on the generator's own output: {static[0].message}")
            continue
        report = tsys.explore(base)

        # TEST-4-01: drop causal edge.
        c, word = _run_greedy(base)
        true_edges = _true_edges(word, c.independent)
        bump("TEST-4-01", "systems")
        clean01 = _check_causal_edge({"system": base, "word": word, "claimed_edges": [_edge_to_json(e) for e in true_edges]})
        if clean01:
            failures["TEST-4-01"].append(f"gen-{seed}: the correct edge set was itself flagged: {clean01[0]['message']}")
        if true_edges:
            bump("TEST-4-01", "words_with_causal_edges")
            dropped = true_edges[1:]  # drop the lexicographically first edge
            mut01 = _check_causal_edge({"system": base, "word": word, "claimed_edges": [_edge_to_json(e) for e in dropped]})
            mutate("TEST-4-01", True, bool(mut01), f"gen-{seed} (dropped {true_edges[0]})")

        # TEST-4-02: declare conflicting events independent.
        bump("TEST-4-02", "systems")
        clean02 = _check_conflict_independent({"system": base})
        if clean02:
            failures["TEST-4-02"].append(f"gen-{seed}: the unmutated system was itself flagged: {clean02[0]['message']}")
        if base["conflicts"]:
            bump("TEST-4-02", "systems_with_conflicts")
            pair = tuple(sorted(base["conflicts"])[0])
            mut02 = _check_conflict_independent({"system": _declare_conflict_independent(base, pair)})
            mutate("TEST-4-02", True, bool(mut02), f"gen-{seed} (conflict {pair} declared independent)")

        # TEST-4-03: skip obligation discharge.
        bump("TEST-4-03", "systems")
        clean03 = _check_obligation_discharge({"system": base})
        if clean03:
            failures["TEST-4-03"].append(f"gen-{seed}: the unmutated (generator-guaranteed leak-free) system was itself flagged: {clean03[0]['message']}")
        site = _find_discharge_site(base)
        if site is not None:
            owner, tname, const = site
            if tname in report.fired:
                # Only a site the unmutated system actually exercises: the
                # process must genuinely reach pc == const before the guard
                # is bumped, or the mutation would "break" an already-dead
                # transition and demonstrate nothing (a generated system can
                # deadlock immediately on an unrelated initial guard).
                bump("TEST-4-03", "systems_with_a_last_step_discharge_site")
                mut03 = _check_obligation_discharge({"system": _skip_discharge(base, tname, const)})
                mutate("TEST-4-03", True, bool(mut03), f"gen-{seed} (discharge {tname} made unreachable)")

        # TEST-4-04: use hash equality only.
        bump("TEST-4-04", "systems")
        bump("TEST-4-04", "exact_reachable_states", report.states)
        states = [list(s[0]) + list(s[1]) for s in report.reachable]
        clean04 = _check_hash_identity({"states": states, "buckets": None})
        if clean04:
            failures["TEST-4-04"].append(f"gen-{seed}: the exact-identity control was itself flagged: {clean04[0]['message']}")
        if report.states > HASH_BUCKETS:
            bump("TEST-4-04", "systems_forcing_a_pigeonhole_collision")
            mut04 = _check_hash_identity({"states": states, "buckets": HASH_BUCKETS})
            mutate("TEST-4-04", True, bool(mut04), f"gen-{seed} ({report.states} states into {HASH_BUCKETS} buckets)")

        # TEST-4-05: ignore epoch.
        bump("TEST-4-05", "systems")
        same_epoch = {"left": {"epoch": 1, "body": base}, "right": {"epoch": 1, "body": base}}
        clean05 = _check_epoch(same_epoch)
        if clean05:
            failures["TEST-4-05"].append(f"gen-{seed}: a same-epoch same-body pair was itself flagged: {clean05[0]['message']}")
        diff_epoch = {"left": {"epoch": 1, "body": base}, "right": {"epoch": 2, "body": base}}
        mut05 = _check_epoch(diff_epoch)
        mutate("TEST-4-05", True, bool(mut05), f"gen-{seed} (same body, epoch 1 vs 2)")

    # TEST-4-06: map submitted to stable. A small closed protocol
    # (Requested/Reserved/Submitted/Stable/Acknowledged) enumerated
    # exhaustively per entry, docs/19 §2's own "for small sizes, enumerate
    # all interleavings and configurations" convention; ENTRIES is index
    # volume for the evidence record, not a source of variation.
    bump("TEST-4-06", "entries", len(ENTRIES))
    for entry in ENTRIES:
        for phase in PHASES:
            clean06 = _check_submitted_stable({"phase": phase, "policy": "correct"})
            if clean06:
                failures["TEST-4-06"].append(f"entry {entry}/{phase}: the correct policy was itself flagged: {clean06[0]['message']}")
            mut06 = _check_submitted_stable({"phase": phase, "policy": "buggy"})
            applied = phase == "Submitted"
            mutate("TEST-4-06", applied, bool(mut06), f"entry {entry}/{phase}")
            if not applied and mut06:
                failures["TEST-4-06"].append(f"entry {entry}/{phase}: the buggy policy was flagged outside the Submitted phase")

    def need(rid: str, key: str, what: str) -> None:
        if corpus[rid].get(key, 0) == 0:
            failures[rid].append(f"corpus exercises no {what} ({key} = 0): the pass would be vacuous")

    need("TEST-4-01", "words_with_causal_edges", "fired trace containing a real causal (dependent-occurrence) edge")
    need("TEST-4-02", "systems_with_conflicts", "generated system with a declared conflict pair")
    need("TEST-4-03", "systems_with_a_last_step_discharge_site", "generated system with a mutable last-step discharge site")
    need("TEST-4-04", "systems_forcing_a_pigeonhole_collision", "system whose exact state count exceeds the hash bucket count")

    for rid in OBLIGATIONS:
        if mutants[rid]["applied"] and mutants[rid]["detected"] == 0:
            failures[rid].append(f"{rid}: the mutant was never detected across the corpus (the checker would be vacuous)")

    status = {
        "TEST-4-01": "enforced",
        "TEST-4-02": "enforced",
        "TEST-4-03": "enforced",
        "TEST-4-04": "partial",
        "TEST-4-05": "partial",
        "TEST-4-06": "partial",
    }
    absence = {
        "TEST-4-04": "no mutable state-identity policy exists in tsys or defect.rs to enforce against; see BOUNDARIES",
        "TEST-4-05": "no epoch field/mutant exists in tsys or defect.rs, and the real epoch.rs cannot be called from this Python harness; see BOUNDARIES",
        "TEST-4-06": "continuum-pack-storage (the adapter the manifest names) does not exist under crates/; see BOUNDARIES",
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
        results[rid] = entry
    return {
        "seeds": f"{SEEDS.start}..{SEEDS.stop - 1}",
        "entries": f"{ENTRIES.start}..{ENTRIES.stop - 1}",
        "requirements": results,
        "unclaimed": ["TEST-4-07", "TEST-4-08", "TEST-4-09"],
        "out_of_scope": "TEST-4-10..18 (Protocol corpus subsection) is a later module's job",
    }
