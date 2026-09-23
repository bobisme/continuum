"""The differential corpus: one seeded generator, in the common sub-model both
`tsys.py` (this harness's TEST-2 oracle) and `continuum-engine-reference`'s
`semantic` oracle (bn-1zgs) can load, plus the Python-side oracle facts a Rust
integration test compares against (bn-1kgnz, under bn-3ly0).

This file imports `tsys` and adds functions; it edits no line `tsys.py` owns
(README-test.md, "Extending tsys is shared ground: add functions, do not
change the meaning of existing fields"). The corpus JSON it emits is a strict
*subset* of the shape `tsys.check_static`/`tsys.explore` already accept, so
both run over it completely unmodified.

# The common sub-model

A shared JSON system (`SCHEMA_VERSION`) is the intersection of what both
oracles can express, not the union:

| Feature | Included | Excluded, and why |
|---|---|---|
| Variable types | `bool`, bounded `int` | `enum` — `continuum-engine-reference::domain::Domain` is an integer range; it has no symbolic/enum domain, so an enum-typed corpus system has no Rust encoding. |
| Guard/update expressions | `const`, `var`, `not`/`and`/`or` (2-ary), `eq`/`ne`/`lt`/`le`/`gt`/`ge`, `add`/`sub` | `ite` (Rust's `expr.rs` has no conditional-value expression); `mul` (`tsys.py`'s `evaluate` has no `mul`); `min`/`max` (`tsys.py` has neither); n-ary `and`/`or` (Rust's `BoolExpr::And`/`Or` are binary; `tsys.py`'s are declared n-ary but the generator below only ever emits 2). |
| Transitions | named, one process, an explicit `guard`, `updates`, and exact `reads`/`writes` footprints | multi-outcome (nondeterministic) actions — `continuum-engine-reference`'s oracle refuses `NondeterministicAction`, and `tsys.py` has no outcome-choice construct either. |
| Independence | footprint-justified pairs (`[a, b]`) | dynamically-justified pairs (`{"pair": [a, b], "justification": "dynamic"}`) — `System::claims_independent` on the Rust side is always footprint-and-conflict derived; there is no declaration form that asks the oracle to trust the diamond alone. |
| Conflicts | declared pairs, same override semantics both sides (`{a, b} not in Cf` on both) | — |
| Obligations | one data variable per obligation, one owning process, no phase | `tsys.py`'s dedicated obligation mechanism (`"obligations"` list, `acquire`/`discharge` transition keys, the `obligation:<name>` pseudo-resource) is a *held-set overlay on the state*, checked by `obligation-linear`/`obligation-leak`. `continuum-engine-reference`'s is a *typed `VarKind::Obligation` variable* with domain `{0, 1}` (`0` discharged, non-zero open), checked by `QuiescenceLeak`/`CompletionLeak`/`OpenForever`. These are mechanically different declaration surfaces over the same intent and are not reconciled here. Instead: an obligation is an ordinary `int(0, 1)` **data** variable in the shared JSON (so `tsys.py` treats it as plain data — its `"obligations"` list stays `[]`, no `acquire`/`discharge` ever appears), named `<process>_ob` by convention. `tsys.py` ignores the convention; the Rust loader (`crates/continuum-engine-reference/tests/semantic_differential.rs`) uses it to build a native `VarKind::Obligation` + `ObligationDecl{owner_phase: None}`. The two oracles' obligation-*flavoured findings* are therefore each side's own extension and are not required to agree; only the structural artifacts (state count, dependence, interleavings, trace classes) and the footprint/independence findings are compared. |
| Cancellation phases | excluded entirely | `tsys.py` has no phase concept at all (README-test.md's own boundary: "TEST-2-07 (cancellation phases) … not claimed by this module"). |
| Fairness | excluded entirely | Same reason: `tsys.py` has no `Fairness` type and no fair-lasso search ("TEST-2-08 (fairness annotations) … not claimed"). The Rust side's `Fairness::Weak` accordingly never appears in a loaded system (every transition is `Fairness::Unfair`), which is also why `OpenForever` can never fire over this corpus — see `run_oracle`'s clean-by-construction argument below. |

# What is compared, and why the corpus is clean by construction

The generator below plants processes whose program counter is the *only*
same-process guard variable, so two transitions of one process are never
co-enabled (each guards a distinct `pc` value). Every declared-independent
pair therefore has fully disjoint footprints by construction, which makes the
diamond hold unconditionally (two actions touching disjoint variables from a
common state commute exactly, and neither can disable the other). Every
obligation is acquired then unconditionally released before its process can
go quiescent, so no obligation is ever open at a terminal state. No variable
is `Phase`-kind, so cancellation regression cannot fire, and no action is
`Weak`-fair, so the Rust side's fairness search is vacuous (an SCC needs an
internal edge to matter, and the reachable graph here is acyclic — `pc`
values only increase, so no state repeats). The corpus is therefore
"findings: []" on both oracles for every seed, *by construction, not by luck*
— `analyze` asserts this rather than assuming it. Comparison target 5
("finding kinds") is consequently exercised only by the negative control in
the Rust test, which fabricates a disagreement rather than mutating a real
system; extending the corpus with genuine seeded defects (mirroring
`continuum-engine-reference::semantic::defect`) is a follow-up, not this
bone's scope — see the module doc of the Rust test for the tracked absence.

# What is compared

For each seed: reachable state count, the *true* semantic dependence
relation (every co-enabled pair at every reachable state, diamond-checked
directly — not the corpus's own declared-independent/declared-conflict
labels, which the check above proves agree with the truth for this corpus,
but are not assumed to), the count of complete interleavings (maximal runs;
this corpus is small enough that neither oracle's depth/interleaving bound
is ever hit — `analyze` and the Rust test both assert their bound was not
hit, rather than silently accepting a partial count), the count of
Mazurkiewicz trace classes under that true dependence relation, and the
finding list (always empty, per the above).
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any

import tsys

SCHEMA_VERSION = "continuum-differential-system-v1"

# The full corpus is seeds 0..CORPUS_SIZE-1. Small enough to explore in well under a
# second per system (nproc=2, at most ~8 actions total, declared state-space product
# in the low hundreds) and large enough to plant both variable kinds, both
# obligation-or-not shapes, and 1-2 work steps per process across the range.
CORPUS_SIZE = 48

# The op vocabulary the loader on the Rust side accepts. A generator that emits
# anything outside this set is a generator defect (self-checked below).
_BOOL_OPS = {"not", "and", "or"}
_CMP_OPS = {"eq", "ne", "lt", "le", "gt", "ge"}
_ARITH_OPS = {"add", "sub"}

_OB_NAME_RE = re.compile(r"^p(\d+)_ob$")


def _c(v: Any) -> dict:
    return {"const": v}


def _v(name: str) -> dict:
    return {"var": name}


def _op(op: str, *args: dict) -> dict:
    assert op in _BOOL_OPS or op in _CMP_OPS or op in _ARITH_OPS, f"op {op!r} outside the common sub-model"
    return {"op": op, "args": list(args)}


def _domain_size(decl: dict) -> int:
    return 2 if decl["kind"] == "bool" else decl["max"] + 1


def _nth_value(decl: dict, index: int) -> Any:
    if decl["kind"] == "bool":
        return index != 0
    return index


def generate_common(seed: int) -> dict:
    """One system in the common sub-model, a pure function of `seed`.

    Shape: exactly 2 processes sharing one data variable `d0` (bool or bounded int).
    Each process has its own program counter `pc_p{p}` and, at 50%, one obligation
    variable `p{p}_ob` (acquired unconditionally first, released unconditionally
    last — see the module doc for why this keeps the corpus leak-free). 1-2 "work"
    steps per process read and optionally update `d0`. The first work step of each
    of the two processes is the *planted pair*: an unconditional write of `d0` to a
    literal value distinct between the two processes, so every generated system has
    a genuine, witnessed dependent pair (the corpus is never vacuous).
    """
    rng = tsys.SplitMix64(seed)
    variables: list[dict] = []
    init: dict[str, Any] = {}

    d0_kind = "int" if rng.below(2) == 0 else "bool"
    d0_decl: dict[str, Any] = (
        {"kind": "int", "min": 0, "max": 1 + rng.below(2)} if d0_kind == "int" else {"kind": "bool"}
    )
    variables.append({"name": "d0", "type": d0_decl})
    init["d0"] = False if d0_kind == "bool" else 0

    values = _domain_size(d0_decl)
    first_index = rng.below(values)
    offset = rng.below(values - 1) + 1
    second_index = (first_index + offset) % values
    planted = [_nth_value(d0_decl, first_index), _nth_value(d0_decl, second_index)]

    transitions: list[dict] = []
    for p in range(2):
        proc = f"p{p}"
        pc = f"pc_{proc}"
        obligate = rng.below(2) == 0
        work_steps = 1 + rng.below(2)
        total_steps = work_steps + (2 if obligate else 0)
        variables.append({"name": pc, "type": {"kind": "int", "min": 0, "max": total_steps}})
        init[pc] = 0
        ob = f"{proc}_ob"
        if obligate:
            variables.append({"name": ob, "type": {"kind": "int", "min": 0, "max": 1}})
            init[ob] = 0

        step = 0
        if obligate:
            guard = _op("eq", _v(pc), _c(0))
            updates = {pc: _op("add", _v(pc), _c(1)), ob: _c(1)}
            transitions.append({
                "name": f"{proc}.acquire", "process": proc, "guard": guard, "updates": updates,
                "reads": sorted({pc, ob}), "writes": sorted({pc, ob}),
            })
            step = 1

        # Once a process has acquired its obligation, every later step of that same
        # process must stay enabled unconditionally on program-counter progress alone
        # (mirrors `continuum-engine-reference::semantic::generate`'s own doc: "the
        # steps strictly after the acquire are guarded by the program counter alone,
        # so a process holding an obligation is never blocked"). A data-dependent
        # guard added here could make `release` permanently unreachable if the
        # required value of `d0` never arrives — a real quiescence leak, not a
        # generator convenience. `blockable` gates every guard/update form that could
        # make a step's enabledness depend on `d0`'s value.
        blockable = not obligate
        for w in range(work_steps):
            k = step + w
            is_planted = w == 0
            if is_planted:
                guard = _op("eq", _v(pc), _c(k))
                updates = {pc: _op("add", _v(pc), _c(1)), "d0": _c(planted[p])}
            else:
                conds = [_op("eq", _v(pc), _c(k))]
                if blockable and rng.below(2) == 0:
                    op = rng.choice(sorted(_CMP_OPS)) if d0_kind == "int" else "eq"
                    lit = _c(rng.below(2) == 0) if d0_kind == "bool" else _c(rng.below(values))
                    conds.append(_op(op, _v("d0"), lit))
                guard = conds[0] if len(conds) == 1 else _op("and", conds[0], conds[1])
                updates = {pc: _op("add", _v(pc), _c(1))}
                if rng.below(2) == 0:
                    if d0_kind == "bool":
                        # Not `not(d0)`: that is a bool-valued update expression, and the
                        # common sub-model has no bool-to-int coercion (Rust's `IntExpr`
                        # has no `ite`/cast; every update RHS must already be int-shaped).
                        # A literal assignment stays inside the intersection.
                        updates["d0"] = _c(rng.below(2) == 0)
                    elif blockable:
                        form = rng.below(3)
                        if form == 0 and d0_decl["max"] > 0:
                            guard = _op("and", guard, _op("lt", _v("d0"), _c(d0_decl["max"])))
                            updates["d0"] = _op("add", _v("d0"), _c(1))
                        elif form == 1:
                            guard = _op("and", guard, _op("gt", _v("d0"), _c(0)))
                            updates["d0"] = _op("sub", _v("d0"), _c(1))
                        else:
                            updates["d0"] = _c(rng.below(values))
                    else:
                        # Not blockable: only a literal, unconditional assignment.
                        updates["d0"] = _c(rng.below(values))
            reads = set(tsys.expr_vars(guard))
            for expr in updates.values():
                reads |= tsys.expr_vars(expr)
            writes = set(updates)
            transitions.append({
                "name": f"{proc}.w{w}", "process": proc, "guard": guard, "updates": updates,
                "reads": sorted(reads), "writes": sorted(writes),
            })

        if obligate:
            k = step + work_steps
            guard = _op("and", _op("eq", _v(pc), _c(k)), _op("eq", _v(ob), _c(1)))
            updates = {pc: _op("add", _v(pc), _c(1)), ob: _c(0)}
            transitions.append({
                "name": f"{proc}.release", "process": proc, "guard": guard, "updates": updates,
                "reads": sorted({pc, ob}), "writes": sorted({pc, ob}),
            })

    independent: list[list[str]] = []
    conflicts: list[list[str]] = []
    for i, a in enumerate(transitions):
        for b in transitions[i + 1:]:
            if a["process"] == b["process"]:
                continue
            ra, wa, rb, wb = set(a["reads"]), set(a["writes"]), set(b["reads"]), set(b["writes"])
            if not ((wa & wb) | (wa & rb) | (ra & wb)):
                independent.append([a["name"], b["name"]])
            elif (wa & tsys.expr_vars(b["guard"])) or (wb & tsys.expr_vars(a["guard"])):
                conflicts.append([a["name"], b["name"]])

    return {
        "name": f"diff-{seed}",
        "variables": variables,
        "init": init,
        "obligations": [],
        "transitions": transitions,
        "independent": independent,
        "conflicts": conflicts,
    }


# ---------------------------------------------------------------------------
# The true-dependence oracle (ground truth, independent of what the system
# declares — see the module doc for why the declared relation is checked
# against this rather than assumed to equal it).
# ---------------------------------------------------------------------------


def true_dependence(system: dict) -> tuple[set[tuple[str, str]], dict, tuple]:
    """Every co-enabled pair at every reachable state whose diamond fails, computed
    directly (fire both orders, compare), the same way
    `continuum-engine-reference::semantic::oracle::check_independence` does — not
    derived from the corpus's own `independent`/`conflicts` declarations."""
    c = tsys.compile_system(system)
    init = tsys.initial_state(c)
    seen = {init}
    frontier = [init]
    succ: dict[tuple, list[tuple[str, tuple]]] = {}
    dependent: set[tuple[str, str]] = set()
    while frontier:
        nxt = []
        for s in frontier:
            en = [t for t in c.transitions if tsys.enabled(c, s, t)]
            row = []
            for t in en:
                s2, fs = tsys.fire(c, s, t)
                if s2 is None or fs:
                    raise AssertionError(f"{system['name']}: transition {t['name']} mis-types in the corpus")
                row.append((t["name"], s2))
                if s2 not in seen:
                    seen.add(s2)
                    nxt.append(s2)
            succ[s] = row
            for i, a in enumerate(en):
                for b in en[i + 1:]:
                    key = tsys.pair_key(a["name"], b["name"])
                    sa, _ = tsys.fire(c, s, a)
                    sb, _ = tsys.fire(c, s, b)
                    a_disables_b = not tsys.enabled(c, sa, b)
                    b_disables_a = not tsys.enabled(c, sb, a)
                    if a_disables_b or b_disables_a:
                        dependent.add(key)
                        continue
                    sab, _ = tsys.fire(c, sa, b)
                    sba, _ = tsys.fire(c, sb, a)
                    if sab != sba:
                        dependent.add(key)
        frontier = nxt
    return dependent, succ, init


def _trace_key(word: list[str], dependent: set[tuple[str, str]]) -> tuple:
    """The same canonical Mazurkiewicz-class key `tsys.trace_class` computes, but
    keyed on the *true* dependence relation rather than the corpus's declared
    independent set."""
    occ: dict[str, int] = {}
    letters = []
    for a in word:
        occ[a] = occ.get(a, 0) + 1
        letters.append((a, occ[a]))
    edges = []
    for i, (a, _) in enumerate(letters):
        for j in range(i + 1, len(letters)):
            b = letters[j][0]
            if a == b or tsys.pair_key(a, b) in dependent:
                edges.append((letters[i], letters[j]))
    return (tuple(sorted(letters)), tuple(sorted(edges)))


def _enumerate_interleavings(succ: dict, init: tuple, dependent: set[tuple[str, str]]) -> tuple[int, int]:
    count = 0
    classes: set[tuple] = set()
    stack: list[tuple[tuple, list[str]]] = [(init, [])]
    while stack:
        s, word = stack.pop()
        row = succ.get(s, [])
        if not row:
            count += 1
            if count > tsys.INTERLEAVING_BOUND:
                raise AssertionError("differential corpus exceeded the interleaving bound")
            classes.add(_trace_key(word, dependent))
            continue
        for name, s2 in reversed(row):
            stack.append((s2, word + [name]))
    return count, len(classes)


def analyze(system: dict) -> dict:
    """The facts compared against the Rust oracle: `states`, `dependent`,
    `interleavings_complete`, `trace_classes`, `findings`. Every corpus system must
    be well-formed and clean on both axes — see the module doc for why that is
    guaranteed by construction, not assumed."""
    static = tsys.check_static(system)
    if static:
        raise AssertionError(f"{system['name']}: not well-formed: {[f.as_json() for f in static]}")
    report = tsys.explore(system)
    if report.inconclusive:
        raise AssertionError(f"{system['name']}: inconclusive: {report.inconclusive}")
    if report.findings:
        raise AssertionError(f"{system['name']}: unexpectedly has findings: {[f.as_json() for f in report.findings]}")

    dependent, succ, init = true_dependence(system)
    if len(succ) != report.states or {*succ} != set(report.reachable):
        raise AssertionError(f"{system['name']}: true_dependence and tsys.explore disagree on the reachable set")
    if tsys._cyclic(init, succ):  # noqa: SLF001 -- shared-ground helper, see module doc
        raise AssertionError(f"{system['name']}: corpus system is cyclic; interleaving enumeration is undefined")
    complete, classes = _enumerate_interleavings(succ, init, dependent)

    return {
        "states": report.states,
        "dependent": sorted(sorted(pair) for pair in dependent),
        "interleavings_complete": complete,
        "trace_classes": classes,
        "findings": [],
    }


def corpus() -> list[dict]:
    """The full committed golden: schema, seeds, and each system with its oracle
    facts."""
    entries = []
    for seed in range(CORPUS_SIZE):
        system = generate_common(seed)
        entries.append({"seed": seed, "system": system, "oracle": analyze(system)})
    return entries


def canonical_bytes() -> bytes:
    payload = {
        "schema": SCHEMA_VERSION,
        "corpus_size": CORPUS_SIZE,
        "systems": corpus(),
    }
    text = json.dumps(payload, sort_keys=True, indent=2, separators=(",", ": "))
    return (text + "\n").encode("ascii")


GOLDEN_PATH = Path(__file__).resolve().parent / "evidence" / "differential_corpus.json"


def main(argv: list[str]) -> int:
    write = "--write" in argv
    data = canonical_bytes()
    if write:
        GOLDEN_PATH.write_bytes(data)
        print(f"wrote {GOLDEN_PATH} ({len(data)} bytes, sha256 {hashlib.sha256(data).hexdigest()[:16]}…)")
        return 0
    if not GOLDEN_PATH.exists():
        print(f"{GOLDEN_PATH} does not exist; run with --write", file=sys.stderr)
        return 1
    committed = GOLDEN_PATH.read_bytes()
    if committed != data:
        print(
            f"{GOLDEN_PATH} is stale: committed {len(committed)} bytes, "
            f"regenerated {len(data)} bytes; run with --write and commit the result",
            file=sys.stderr,
        )
        return 1
    print(f"{GOLDEN_PATH} is up to date ({len(data)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
