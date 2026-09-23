"""Finite transition systems for the docs/19 test-policy harness.

This module is the shared substrate of `check_test_policy.py`. It holds four
things, each a pure function of its inputs (no clock, no ambient randomness,
no hash-ordered iteration that reaches an output):

1. The system shape. A system is a JSON object with typed state variables,
   explicitly guarded transitions, explicit read/write footprints, a declared
   independence relation, declared conflicts, and linear obligations — the
   first six features docs/19 §2 lists. The shape is canonical JSON so a later
   consumer (an optimized Rust engine, a §3 metamorphic relation, a §4 mutant)
   can read the same bytes.
2. Well-formedness: the static rules a declaration must satisfy.
3. The oracle: exhaustive exploration of every reachable state, every maximal
   interleaving, and every Mazurkiewicz trace class ("configuration") for small
   sizes, with the dynamic rules (typing, footprints, the independence diamond,
   conflict completeness, obligation linearity and leaks) evaluated on every
   reachable state.
4. A reduction under test (sleep-set exploration driven by the *declared*
   independence relation) and its comparison against the oracle.

And a seeded generator (`generate`) that produces the corpus. Its PRNG is an
explicit SplitMix64 over a caller-supplied seed, so a corpus is reproducible
from the seed alone (INV-005).

Every finding is a `Finding(rule, requirement, subject, message)`. Outcomes the
oracle cannot decide (a cyclic system for interleaving enumeration, an
enumeration bound exceeded) are typed as `Inconclusive`, never folded into a
pass (INV-008).
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Any

# Rule -> requirement id. The single place a rule is bound to the docs/19 §2
# obligation it serves; the evidence file is keyed through it.
RULES: dict[str, str] = {
    # TEST-2-01 typed state variables
    "type-declared": "TEST-2-01",
    "init-typed": "TEST-2-01",
    "expr-typed": "TEST-2-01",
    "value-in-type": "TEST-2-01",
    # TEST-2-02 guarded transitions
    "guard-declared": "TEST-2-02",
    "guard-boolean": "TEST-2-02",
    "transition-wellformed": "TEST-2-02",
    # TEST-2-03 explicit read/write footprints
    "footprint-declared": "TEST-2-03",
    "footprint-covers-reads": "TEST-2-03",
    "footprint-covers-writes": "TEST-2-03",
    # TEST-2-04 independent/dependent pairs
    "independence-wellformed": "TEST-2-04",
    "independence-static": "TEST-2-04",
    "independence-diamond": "TEST-2-04",
    "reduction-agrees": "TEST-2-04",
    # TEST-2-05 conflicts
    "conflict-wellformed": "TEST-2-05",
    "conflict-not-independent": "TEST-2-05",
    "conflict-complete": "TEST-2-05",
    # TEST-2-06 obligations
    "obligation-declared": "TEST-2-06",
    "obligation-linear": "TEST-2-06",
    "obligation-leak": "TEST-2-06",
}

INTERLEAVING_BOUND = 50_000
STATE_BOUND = 200_000


@dataclass(frozen=True, order=True)
class Finding:
    rule: str
    requirement: str
    subject: str
    message: str

    def as_json(self) -> dict[str, str]:
        return {
            "rule": self.rule,
            "requirement": self.requirement,
            "subject": self.subject,
            "message": self.message,
        }


def finding(rule: str, subject: str, message: str) -> Finding:
    return Finding(rule, RULES[rule], subject, message)


@dataclass(frozen=True)
class Inconclusive:
    """A typed undecided outcome (INV-008): the reason is one of a closed set."""

    reason: str  # "cyclic-state-graph" | "interleaving-bound-exceeded" | "state-bound-exceeded" | "ill-formed"
    detail: str


def canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


# ---------------------------------------------------------------------------
# Types and expressions
# ---------------------------------------------------------------------------

BOOL = ("bool",)
INT = ("int",)


def _type_tag(decl: Any) -> tuple | None:
    """The expression-level type of a variable type declaration, or None."""
    if not isinstance(decl, dict):
        return None
    kind = decl.get("kind")
    if kind == "bool" and set(decl) == {"kind"}:
        return BOOL
    if kind == "int" and set(decl) == {"kind", "min", "max"}:
        lo, hi = decl["min"], decl["max"]
        if type(lo) is int and type(hi) is int and lo <= hi:
            return INT
        return None
    if kind == "enum" and set(decl) == {"kind", "values"}:
        values = decl["values"]
        if (
            isinstance(values, list)
            and values
            and all(isinstance(v, str) and v for v in values)
            and len(set(values)) == len(values)
        ):
            return ("enum", tuple(values))
        return None
    return None


def value_has_type(value: Any, decl: dict) -> bool:
    kind = decl["kind"]
    if kind == "bool":
        return type(value) is bool
    if kind == "int":
        return type(value) is int and decl["min"] <= value <= decl["max"]
    return type(value) is str and value in decl["values"]


def domain(decl: dict) -> list[Any]:
    kind = decl["kind"]
    if kind == "bool":
        return [False, True]
    if kind == "int":
        return list(range(decl["min"], decl["max"] + 1))
    return list(decl["values"])


_BOOL_OPS = {"not": 1, "and": None, "or": None}
_CMP_OPS = {"lt", "le", "gt", "ge"}
_ARITH_OPS = {"add", "sub"}
_EQ_OPS = {"eq", "ne"}


class _TypeError(Exception):
    pass


def expr_vars(expr: Any) -> set[str]:
    out: set[str] = set()

    def walk(e: Any) -> None:
        if isinstance(e, dict):
            if "var" in e and isinstance(e["var"], str):
                out.add(e["var"])
            for arg in e.get("args", []) if isinstance(e.get("args"), list) else []:
                walk(arg)

    walk(expr)
    return out


def type_of(expr: Any, env: dict[str, tuple]) -> tuple:
    """Static type of `expr` under `env` (variable -> type tag). Raises _TypeError."""
    if not isinstance(expr, dict):
        raise _TypeError(f"expression is not an object: {canonical(expr)}")
    if set(expr) == {"const"}:
        value = expr["const"]
        if type(value) is bool:
            return BOOL
        if type(value) is int:
            return INT
        if isinstance(value, str) and value:
            return ("sym", value)
        raise _TypeError(f"constant of no type: {canonical(value)}")
    if set(expr) == {"var"}:
        name = expr["var"]
        if name not in env:
            raise _TypeError(f"unknown variable {name!r}")
        return env[name]
    if set(expr) == {"op", "args"} and isinstance(expr["args"], list):
        op, args = expr["op"], expr["args"]
        types = [type_of(a, env) for a in args]
        if op in _BOOL_OPS:
            arity = _BOOL_OPS[op]
            if (arity is not None and len(args) != arity) or (arity is None and len(args) < 2):
                raise _TypeError(f"{op} has wrong arity {len(args)}")
            if any(t != BOOL for t in types):
                raise _TypeError(f"{op} applied to non-bool operand")
            return BOOL
        if op in _CMP_OPS or op in _ARITH_OPS:
            if len(args) != 2 or any(t != INT for t in types):
                raise _TypeError(f"{op} needs two int operands")
            return BOOL if op in _CMP_OPS else INT
        if op in _EQ_OPS:
            if len(args) != 2:
                raise _TypeError(f"{op} needs two operands")
            if not compatible(types[0], types[1]):
                raise _TypeError(f"{op} compares incompatible types {types[0]} and {types[1]}")
            return BOOL
        if op == "ite":
            if len(args) != 3 or types[0] != BOOL:
                raise _TypeError("ite needs a bool condition and two branches")
            if not compatible(types[1], types[2]):
                raise _TypeError("ite branches have incompatible types")
            return types[1] if types[1][0] != "sym" else types[2]
        raise _TypeError(f"unknown operator {op!r}")
    raise _TypeError(f"malformed expression: {canonical(expr)}")


def compatible(a: tuple, b: tuple) -> bool:
    if a == b:
        return True
    if a[0] == "sym" and b[0] == "enum":
        return a[1] in b[1]
    if b[0] == "sym" and a[0] == "enum":
        return b[1] in a[1]
    return False


def assignable(target: tuple, source: tuple) -> bool:
    if target[0] == "enum" and source[0] == "sym":
        return source[1] in target[1]
    return target == source


def evaluate(expr: dict, env: dict[str, Any]) -> Any:
    if "const" in expr:
        return expr["const"]
    if "var" in expr:
        return env[expr["var"]]
    op, args = expr["op"], expr["args"]
    if op == "ite":
        return evaluate(args[1], env) if evaluate(args[0], env) else evaluate(args[2], env)
    vals = [evaluate(a, env) for a in args]
    if op == "not":
        return not vals[0]
    if op == "and":
        return all(vals)
    if op == "or":
        return any(vals)
    if op == "eq":
        return vals[0] == vals[1] and type(vals[0]) is type(vals[1])
    if op == "ne":
        return not (vals[0] == vals[1] and type(vals[0]) is type(vals[1]))
    if op == "lt":
        return vals[0] < vals[1]
    if op == "le":
        return vals[0] <= vals[1]
    if op == "gt":
        return vals[0] > vals[1]
    if op == "ge":
        return vals[0] >= vals[1]
    if op == "add":
        return vals[0] + vals[1]
    if op == "sub":
        return vals[0] - vals[1]
    raise ValueError(op)


# ---------------------------------------------------------------------------
# Well-formedness (static rules)
# ---------------------------------------------------------------------------

_TRANSITION_KEYS = {"name", "process", "guard", "updates", "reads", "writes", "acquire", "discharge"}
_REQUIRED_TRANSITION_KEYS = {"name", "process", "updates", "reads", "writes"}
_SYSTEM_KEYS = {"name", "variables", "init", "obligations", "transitions", "independent", "conflicts"}


def obligation_resource(name: str) -> str:
    return f"obligation:{name}"


def _pairs(raw: Any) -> list[tuple[str, str]] | None:
    if not isinstance(raw, list):
        return None
    out = []
    for item in raw:
        if not (isinstance(item, list) and len(item) == 2 and all(isinstance(x, str) for x in item)):
            return None
        out.append((item[0], item[1]))
    return out


def independence_entries(raw: Any) -> list[tuple[str, str, str]] | None:
    """Declared independence: `[a, b]` is justified by disjoint footprints and
    checked statically; `{"pair": [a, b], "justification": "dynamic"}` claims
    commutation despite overlapping footprints and is checked only by the
    oracle's diamond (docs/02 §3: "proven statically", "checked dynamically")."""
    if not isinstance(raw, list):
        return None
    out = []
    for item in raw:
        if isinstance(item, dict):
            if set(item) != {"pair", "justification"} or item["justification"] != "dynamic":
                return None
            pair = _pairs([item["pair"]])
            if pair is None:
                return None
            out.append((pair[0][0], pair[0][1], "dynamic"))
        else:
            pair = _pairs([item])
            if pair is None:
                return None
            out.append((pair[0][0], pair[0][1], "footprint"))
    return out


def pair_key(a: str, b: str) -> tuple[str, str]:
    return (a, b) if a <= b else (b, a)


def check_static(system: Any) -> list[Finding]:
    """Every static rule. An empty list means the declaration is well formed."""
    out: list[Finding] = []
    if not isinstance(system, dict) or set(system) != _SYSTEM_KEYS:
        return [finding("transition-wellformed", "<system>", f"system keys must be exactly {sorted(_SYSTEM_KEYS)}")]
    name = system["name"]

    # TEST-2-01: typed state variables.
    env: dict[str, tuple] = {}
    decls: dict[str, dict] = {}
    variables = system["variables"]
    if not isinstance(variables, list) or not variables:
        out.append(finding("type-declared", name, "variables must be a non-empty list"))
        variables = []
    for var in variables:
        vname = var.get("name") if isinstance(var, dict) else None
        if not isinstance(vname, str) or not vname or set(var) != {"name", "type"}:
            out.append(finding("type-declared", name, f"variable declaration malformed: {canonical(var)}"))
            continue
        if vname in env:
            out.append(finding("type-declared", vname, "variable declared twice"))
            continue
        tag = _type_tag(var["type"])
        if tag is None:
            out.append(finding("type-declared", vname, f"no valid type: {canonical(var['type'])}"))
            continue
        env[vname] = tag
        decls[vname] = var["type"]
    init = system["init"]
    if not isinstance(init, dict):
        out.append(finding("init-typed", name, "init must be an object"))
        init = {}
    for vname in sorted(set(init) - set(env)):
        out.append(finding("init-typed", vname, "init assigns an undeclared variable"))
    for vname in sorted(env):
        if vname not in init:
            out.append(finding("init-typed", vname, "variable has no initial value"))
        elif not value_has_type(init[vname], decls[vname]):
            out.append(finding("init-typed", vname, f"initial value {canonical(init[vname])} is not of its type"))

    # TEST-2-06 (declarations): obligations.
    obligations: dict[str, str] = {}
    raw_obl = system["obligations"]
    if not isinstance(raw_obl, list):
        out.append(finding("obligation-declared", name, "obligations must be a list"))
        raw_obl = []
    for obl in raw_obl:
        if not (isinstance(obl, dict) and set(obl) == {"name", "owner"} and all(isinstance(obl[k], str) and obl[k] for k in obl)):
            out.append(finding("obligation-declared", name, f"obligation declaration malformed: {canonical(obl)}"))
            continue
        if obl["name"] in obligations:
            out.append(finding("obligation-declared", obl["name"], "obligation declared twice"))
            continue
        obligations[obl["name"]] = obl["owner"]

    # TEST-2-02 / TEST-2-03: transitions, guards, footprints.
    transitions = system["transitions"]
    names: set[str] = set()
    processes: set[str] = set()
    acquired: dict[str, list[str]] = {}
    discharged: dict[str, list[str]] = {}
    if not isinstance(transitions, list) or not transitions:
        out.append(finding("transition-wellformed", name, "transitions must be a non-empty list"))
        transitions = []
    resources = set(env) | {obligation_resource(o) for o in obligations}
    for t in transitions:
        tname = t.get("name") if isinstance(t, dict) else None
        if not isinstance(tname, str) or not tname:
            out.append(finding("transition-wellformed", name, f"transition has no name: {canonical(t)}"))
            continue
        if tname in names:
            out.append(finding("transition-wellformed", tname, "transition name declared twice"))
            continue
        names.add(tname)
        extra = set(t) - _TRANSITION_KEYS
        if extra:
            out.append(finding("transition-wellformed", tname, f"unknown keys {sorted(extra)}"))
        if "guard" not in t:
            out.append(finding("guard-declared", tname, "transition has no explicit guard"))
        missing = _REQUIRED_TRANSITION_KEYS - set(t)
        for key in sorted(missing & {"reads", "writes"}):
            out.append(finding("footprint-declared", tname, f"transition declares no {key} footprint"))
        if missing - {"reads", "writes"}:
            out.append(finding("transition-wellformed", tname, f"missing keys {sorted(missing - {'reads', 'writes'})}"))
            continue
        if not isinstance(t["process"], str) or not t["process"]:
            out.append(finding("transition-wellformed", tname, "process must be a non-empty string"))
            continue
        processes.add(t["process"])
        reads = t.get("reads", [])
        writes = t.get("writes", [])
        fp_ok = True
        for key, fp in (("reads", reads), ("writes", writes)):
            if not isinstance(fp, list) or not all(isinstance(r, str) for r in fp) or len(set(fp)) != len(fp):
                out.append(finding("footprint-declared", tname, f"{key} must be a list of distinct names"))
                fp_ok = False
                continue
            for r in fp:
                if r not in resources:
                    out.append(finding("footprint-declared", tname, f"{key} names undeclared resource {r!r}"))
        if not fp_ok:
            reads, writes = [], []
        guard = t.get("guard")
        if "guard" in t:
            try:
                if type_of(guard, env) != BOOL:
                    out.append(finding("guard-boolean", tname, "guard is not of type bool"))
            except _TypeError as exc:
                out.append(finding("expr-typed", tname, f"guard: {exc}"))
            for v in sorted(expr_vars(guard) - set(reads)):
                out.append(finding("footprint-covers-reads", tname, f"guard reads {v!r} outside the declared reads"))
        updates = t["updates"]
        if not isinstance(updates, dict):
            out.append(finding("transition-wellformed", tname, "updates must be an object"))
            updates = {}
        for target, expr in sorted(updates.items()):
            if target not in env:
                out.append(finding("expr-typed", tname, f"update targets undeclared variable {target!r}"))
                continue
            try:
                source = type_of(expr, env)
                if not assignable(env[target], source):
                    out.append(finding("expr-typed", tname, f"update of {target!r} assigns {source}, variable is {env[target]}"))
            except _TypeError as exc:
                out.append(finding("expr-typed", tname, f"update of {target!r}: {exc}"))
            for v in sorted(expr_vars(expr) - set(reads)):
                out.append(finding("footprint-covers-reads", tname, f"update reads {v!r} outside the declared reads"))
            if target not in writes:
                out.append(finding("footprint-covers-writes", tname, f"writes {target!r} outside the declared writes"))
        for key, sink in (("acquire", acquired), ("discharge", discharged)):
            items = t.get(key, [])
            if not isinstance(items, list) or not all(isinstance(o, str) for o in items):
                out.append(finding("obligation-declared", tname, f"{key} must be a list of obligation names"))
                continue
            for o in items:
                if o not in obligations:
                    out.append(finding("obligation-declared", tname, f"{key}s undeclared obligation {o!r}"))
                    continue
                if obligations[o] != t["process"]:
                    out.append(finding("obligation-declared", tname, f"{key}s {o!r} owned by {obligations[o]!r}, not {t['process']!r}"))
                sink.setdefault(o, []).append(tname)
                res = obligation_resource(o)
                if res not in writes:
                    out.append(finding("footprint-covers-writes", tname, f"{key}s {o!r} outside the declared writes"))
                if res not in reads:
                    out.append(finding("footprint-covers-reads", tname, f"{key}s {o!r} outside the declared reads"))
    for o, owner in sorted(obligations.items()):
        if owner not in processes:
            out.append(finding("obligation-declared", o, f"owner {owner!r} is not a process"))
        if o not in acquired:
            out.append(finding("obligation-declared", o, "obligation is never acquired"))
        if o not in discharged:
            out.append(finding("obligation-declared", o, "obligation is never discharged"))

    # TEST-2-04 / TEST-2-05: relations.
    by_name = {t["name"]: t for t in transitions if isinstance(t, dict) and isinstance(t.get("name"), str)}
    entries = independence_entries(system["independent"])
    conflicts = _pairs(system["conflicts"])
    if entries is None:
        out.append(finding("independence-wellformed", name, "independent must be a list of name pairs or dynamic-justified pairs"))
        entries = []
    independent = [(a, b) for a, b, _ in entries]
    justification = {pair_key(a, b): j for a, b, j in entries}
    if conflicts is None:
        out.append(finding("conflict-wellformed", name, "conflicts must be a list of name pairs"))
        conflicts = []
    for rel, rule, pairs in (("independent", "independence-wellformed", independent), ("conflicts", "conflict-wellformed", conflicts)):
        seen: set[tuple[str, str]] = set()
        for a, b in pairs:
            subject = f"{a}|{b}"
            if a == b:
                out.append(finding(rule, subject, f"{rel} pair relates a transition to itself"))
            if a not in names or b not in names:
                out.append(finding(rule, subject, f"{rel} pair names an undeclared transition"))
            if pair_key(a, b) in seen:
                out.append(finding(rule, subject, f"{rel} pair declared twice"))
            seen.add(pair_key(a, b))
    conflict_keys = {pair_key(a, b) for a, b in conflicts}
    for a, b in independent:
        if a not in by_name or b not in by_name or a == b:
            continue
        subject = f"{a}|{b}"
        if pair_key(a, b) in conflict_keys:
            out.append(finding("conflict-not-independent", subject, "a declared conflict is declared independent"))
        if justification.get(pair_key(a, b)) == "dynamic":
            continue
        ta, tb = by_name[a], by_name[b]
        ra, wa = set(ta.get("reads", [])), set(ta.get("writes", []))
        rb, wb = set(tb.get("reads", [])), set(tb.get("writes", []))
        overlap = sorted((wa & wb) | (wa & rb) | (ra & wb))
        if overlap:
            out.append(finding("independence-static", subject, f"declared independent but footprints overlap on {overlap}"))
    return out


# ---------------------------------------------------------------------------
# The oracle
# ---------------------------------------------------------------------------


@dataclass
class Compiled:
    system: dict
    order: list[str]  # variable order in a state vector
    decls: dict[str, dict]
    transitions: list[dict]  # sorted by name
    index: dict[str, int]
    independent: set[tuple[str, str]]
    conflicts: set[tuple[str, str]]


def compile_system(system: dict) -> Compiled:
    order = sorted(v["name"] for v in system["variables"])
    decls = {v["name"]: v["type"] for v in system["variables"]}
    transitions = sorted(system["transitions"], key=lambda t: t["name"])
    return Compiled(
        system=system,
        order=order,
        decls=decls,
        transitions=transitions,
        index={t["name"]: i for i, t in enumerate(transitions)},
        independent={pair_key(a, b) for a, b, _ in independence_entries(system["independent"]) or []},
        conflicts={pair_key(a, b) for a, b in system["conflicts"]},
    )


# A state is (values tuple in `order`, sorted tuple of held obligations).
State = tuple


def initial_state(c: Compiled) -> State:
    return (tuple(c.system["init"][v] for v in c.order), ())


def _env(c: Compiled, state: State) -> dict[str, Any]:
    return dict(zip(c.order, state[0]))


def enabled(c: Compiled, state: State, t: dict) -> bool:
    return bool(evaluate(t.get("guard", {"const": True}), _env(c, state)))


def fire(c: Compiled, state: State, t: dict) -> tuple[State | None, list[Finding]]:
    """Successor of `state` under enabled `t`, or None with the dynamic findings."""
    env = _env(c, state)
    new = dict(env)
    out: list[Finding] = []
    for target, expr in sorted(t["updates"].items()):
        new[target] = evaluate(expr, env)
    for target, value in sorted(new.items()):
        if not value_has_type(value, c.decls[target]):
            out.append(finding("value-in-type", t["name"], f"assigns {target} := {canonical(value)} outside {canonical(c.decls[target])}"))
    held = set(state[1])
    for o in t.get("acquire", []):
        if o in held:
            out.append(finding("obligation-linear", t["name"], f"acquires {o!r} while it is already held"))
        held.add(o)
    for o in t.get("discharge", []):
        if o not in held:
            out.append(finding("obligation-linear", t["name"], f"discharges {o!r} while it is not held"))
        held.discard(o)
    if out:
        return None, out
    return (tuple(new[v] for v in c.order), tuple(sorted(held))), out


@dataclass
class OracleReport:
    findings: list[Finding] = field(default_factory=list)
    inconclusive: list[Inconclusive] = field(default_factory=list)
    states: int = 0
    reachable: list[State] = field(default_factory=list)
    terminal_states: list[State] = field(default_factory=list)
    interleavings: int = 0
    trace_classes: int = 0
    reduced_runs: int = 0
    reduced_classes: int = 0
    # Coverage facts the corpus-level non-vacuity rules read.
    guard_true: set[str] = field(default_factory=set)
    guard_false: set[str] = field(default_factory=set)
    coenabled_independent: set[tuple[str, str]] = field(default_factory=set)
    coenabled_dependent: set[tuple[str, str]] = field(default_factory=set)
    witnessed_conflicts: set[tuple[str, str]] = field(default_factory=set)
    fired: set[str] = field(default_factory=set)
    leaks: int = 0


def _dedup(findings: list[Finding]) -> list[Finding]:
    return sorted(set(findings))


def explore(system: dict) -> OracleReport:
    """Run the oracle. Call only on a system `check_static` accepts."""
    c = compile_system(system)
    report = OracleReport()
    init = initial_state(c)
    seen = {init}
    frontier = [init]
    succ: dict[State, list[tuple[str, State]]] = {}
    while frontier:
        nxt = []
        for s in frontier:
            row = []
            en = [t for t in c.transitions if enabled(c, s, t)]
            for t in c.transitions:
                (report.guard_true if t in en else report.guard_false).add(t["name"])
            for t in en:
                s2, fs = fire(c, s, t)
                report.findings.extend(fs)
                if s2 is None:
                    continue
                report.fired.add(t["name"])
                row.append((t["name"], s2))
                if s2 not in seen:
                    if len(seen) >= STATE_BOUND:
                        report.inconclusive.append(Inconclusive("state-bound-exceeded", f"more than {STATE_BOUND} states"))
                        report.findings = _dedup(report.findings)
                        return report
                    seen.add(s2)
                    nxt.append(s2)
            succ[s] = row
            # Pairwise: diamond for declared-independent pairs, disabling for conflicts.
            for i, a in enumerate(en):
                for b in en[i + 1 :]:
                    key = pair_key(a["name"], b["name"])
                    sa, _ = fire(c, s, a)
                    sb, _ = fire(c, s, b)
                    if sa is None or sb is None:
                        continue
                    a_disables_b = not enabled(c, sa, b)
                    b_disables_a = not enabled(c, sb, a)
                    if a_disables_b or b_disables_a:
                        report.witnessed_conflicts.add(key)
                        if key not in c.conflicts:
                            report.findings.append(
                                finding("conflict-complete", f"{key[0]}|{key[1]}", "one transition disables the other in a reachable state, and no conflict is declared")
                            )
                    if key in c.independent:
                        report.coenabled_independent.add(key)
                        if a_disables_b or b_disables_a:
                            report.findings.append(finding("independence-diamond", f"{key[0]}|{key[1]}", "declared independent, but one disables the other"))
                            continue
                        sab, _ = fire(c, sa, b)
                        sba, _ = fire(c, sb, a)
                        if sab != sba:
                            report.findings.append(finding("independence-diamond", f"{key[0]}|{key[1]}", "declared independent, but the two orders reach different states"))
                    else:
                        report.coenabled_dependent.add(key)
        frontier = nxt
    report.states = len(seen)
    report.reachable = sorted(seen, key=canonical)
    terminals = sorted((s for s in seen if not succ.get(s)), key=canonical)
    report.terminal_states = terminals
    for s in terminals:
        if s[1]:
            report.leaks += 1
            report.findings.append(finding("obligation-leak", system["name"], f"terminal state {canonical(list(s[0]))} holds {list(s[1])}"))

    # Interleavings and trace classes: only on an acyclic state graph.
    if _cyclic(init, succ):
        report.inconclusive.append(Inconclusive("cyclic-state-graph", "interleaving enumeration needs an acyclic state graph"))
    else:
        _interleavings(c, init, succ, report)
    report.findings = _dedup(report.findings)
    return report


def _cyclic(init: State, succ: dict) -> bool:
    WHITE, GREY, BLACK = 0, 1, 2
    colour: dict[State, int] = {}
    stack = [(init, iter(succ.get(init, [])))]
    colour[init] = GREY
    while stack:
        s, it = stack[-1]
        step = next(it, None)
        if step is None:
            colour[s] = BLACK
            stack.pop()
            continue
        s2 = step[1]
        col = colour.get(s2, WHITE)
        if col == GREY:
            return True
        if col == WHITE:
            colour[s2] = GREY
            stack.append((s2, iter(succ.get(s2, []))))
    return False


def trace_class(word: list[str], independent: set[tuple[str, str]]) -> tuple:
    """The Mazurkiewicz trace of `word`: its occurrence-indexed letters plus the
    order between dependent occurrences. Two words have equal keys exactly when
    one is reachable from the other by swapping adjacent independent letters."""
    occ: dict[str, int] = {}
    letters = []
    for a in word:
        occ[a] = occ.get(a, 0) + 1
        letters.append((a, occ[a]))
    edges = []
    for i, (a, _) in enumerate(letters):
        for j in range(i + 1, len(letters)):
            b = letters[j][0]
            if a == b or pair_key(a, b) not in independent:
                edges.append((letters[i], letters[j]))
    return (tuple(sorted(letters)), tuple(sorted(edges)))


def _interleavings(c: Compiled, init: State, succ: dict, report: OracleReport) -> None:
    # Full enumeration: every maximal interleaving from the initial state.
    full_terminals: set[State] = set()
    classes: set[tuple] = set()
    count = 0
    stack: list[tuple[State, list[str]]] = [(init, [])]
    while stack:
        s, word = stack.pop()
        row = succ.get(s, [])
        if not row:
            count += 1
            if count > INTERLEAVING_BOUND:
                report.inconclusive.append(Inconclusive("interleaving-bound-exceeded", f"more than {INTERLEAVING_BOUND} maximal interleavings"))
                return
            full_terminals.add(s)
            classes.add(trace_class(word, c.independent))
            continue
        for name, s2 in reversed(row):
            stack.append((s2, word + [name]))
    report.interleavings = count
    report.trace_classes = len(classes)

    # The reduction under test: sleep sets over the declared independence.
    reduced_terminals: set[State] = set()
    reduced_classes: set[tuple] = set()
    runs = 0
    rstack: list[tuple[State, list[str], frozenset[str]]] = [(init, [], frozenset())]
    while rstack:
        s, word, sleep = rstack.pop()
        row = succ.get(s, [])
        if not row:
            runs += 1
            reduced_terminals.add(s)
            reduced_classes.add(trace_class(word, c.independent))
            continue
        done: list[str] = []
        children = []
        for name, s2 in row:
            if name in sleep:
                continue
            child_sleep = frozenset(u for u in list(sleep) + done if pair_key(u, name) in c.independent)
            children.append((s2, word + [name], child_sleep))
            done.append(name)
        rstack.extend(reversed(children))
    report.reduced_runs = runs
    report.reduced_classes = len(reduced_classes)
    if reduced_terminals != full_terminals:
        missing = sorted((canonical([list(s[0]), list(s[1])]) for s in full_terminals - reduced_terminals))
        report.findings.append(
            finding("reduction-agrees", c.system["name"], f"sleep-set reduction misses {len(missing)} terminal state(s) the oracle reaches, first {missing[0] if missing else '-'}")
        )
    elif reduced_classes != classes:
        report.findings.append(
            finding("reduction-agrees", c.system["name"], f"sleep-set reduction covers {len(reduced_classes)} of {len(classes)} trace classes")
        )


def check(system: Any) -> tuple[list[Finding], OracleReport | None]:
    """Static rules, then (only for a well-formed system) the oracle."""
    static = check_static(system)
    if static:
        return sorted(set(static)), None
    report = explore(system)
    return report.findings, report


# ---------------------------------------------------------------------------
# The generator
# ---------------------------------------------------------------------------


class SplitMix64:
    """Explicitly seeded PRNG. No ambient entropy (INV-005)."""

    MASK = (1 << 64) - 1

    def __init__(self, seed: int) -> None:
        self.state = seed & self.MASK

    def next(self) -> int:
        self.state = (self.state + 0x9E3779B97F4A7C15) & self.MASK
        z = self.state
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & self.MASK
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & self.MASK
        return z ^ (z >> 31)

    def below(self, n: int) -> int:
        return self.next() % n

    def choice(self, items: list) -> Any:
        return items[self.below(len(items))]


def _c(v: Any) -> dict:
    return {"const": v}


def _v(name: str) -> dict:
    return {"var": name}


def _op(op: str, *args: dict) -> dict:
    return {"op": op, "args": list(args)}


def generate(seed: int) -> dict:
    """One finite transition system, a pure function of `seed`.

    Shape: 2–3 processes, each a straight-line sequence of 1–3 guarded steps
    over its own program counter and 1–3 shared variables of the three kinds.
    A process may own one obligation, acquired at one step and discharged at a
    later one; the steps strictly after the acquire are guarded by the program
    counter alone, so a process holding an obligation is never blocked and a
    generated system cannot leak one. Footprints are the exact variables each
    guard and update mention. Independence is declared for cross-process pairs
    whose footprints do not overlap (docs/02 §4 dependence sources); conflicts
    are declared for cross-process pairs where one writes a variable the
    other's guard reads (a conservative static superset of disabling).
    """
    rng = SplitMix64(seed)
    variables: list[dict] = []
    init: dict[str, Any] = {}
    shared = []
    kinds = ["int", "bool", "enum"]
    for i in range(1 + rng.below(3)):
        kind = kinds[(i + rng.below(3)) % 3]
        vname = f"v{i}"
        if kind == "int":
            decl = {"kind": "int", "min": 0, "max": 1 + rng.below(3)}
            init[vname] = 0
        elif kind == "bool":
            decl = {"kind": "bool"}
            init[vname] = False
        else:
            decl = {"kind": "enum", "values": ["a", "b", "c"][: 2 + rng.below(2)]}
            init[vname] = "a"
        variables.append({"name": vname, "type": decl})
        shared.append((vname, decl))

    transitions: list[dict] = []
    obligations: list[dict] = []
    nproc = 2 + rng.below(2)
    for p in range(nproc):
        proc = f"p{p}"
        pc = f"pc_{proc}"
        steps = 1 + rng.below(3)
        variables.append({"name": pc, "type": {"kind": "int", "min": 0, "max": steps}})
        init[pc] = 0
        window: tuple[int, int] | None = None
        if steps >= 2 and rng.below(2) == 0:
            a = rng.below(steps - 1)
            d = a + 1 + rng.below(steps - 1 - a)
            window = (a, d)
            obligations.append({"name": f"o_{proc}", "owner": proc})
        for k in range(steps):
            conds = [_op("eq", _v(pc), _c(k))]
            unconditional = window is not None and window[0] < k <= window[1]
            if not unconditional and rng.below(3) != 0:
                vname, decl = rng.choice(shared)
                if decl["kind"] == "int":
                    conds.append(_op(rng.choice(["lt", "eq"]), _v(vname), _c(rng.below(decl["max"] + 1))))
                elif decl["kind"] == "bool":
                    conds.append(_op("eq", _v(vname), _c(rng.below(2) == 0)))
                else:
                    conds.append(_op("eq", _v(vname), _c(rng.choice(decl["values"]))))
            updates: dict[str, dict] = {pc: _op("add", _v(pc), _c(1))}
            if rng.below(4) != 0:
                vname, decl = rng.choice(shared)
                form = rng.below(2)
                if decl["kind"] == "int":
                    if form == 0 and not unconditional:
                        conds.append(_op("lt", _v(vname), _c(decl["max"])))
                        updates[vname] = _op("add", _v(vname), _c(1))
                    else:
                        updates[vname] = _c(rng.below(decl["max"] + 1))
                elif decl["kind"] == "bool":
                    updates[vname] = _op("not", _v(vname)) if form == 0 else _c(rng.below(2) == 0)
                else:
                    updates[vname] = _c(rng.choice(decl["values"]))
            guard = conds[0] if len(conds) == 1 else _op("and", *conds)
            reads = set(expr_vars(guard))
            for expr in updates.values():
                reads |= expr_vars(expr)
            writes = set(updates)
            t: dict[str, Any] = {"name": f"{proc}.s{k}", "process": proc, "guard": guard, "updates": updates}
            if window is not None and k == window[0]:
                t["acquire"] = [f"o_{proc}"]
            if window is not None and k == window[1]:
                t["discharge"] = [f"o_{proc}"]
            if "acquire" in t or "discharge" in t:
                res = obligation_resource(f"o_{proc}")
                reads.add(res)
                writes.add(res)
            t["reads"] = sorted(reads)
            t["writes"] = sorted(writes)
            transitions.append(t)

    independent = []
    conflicts = []
    for i, a in enumerate(transitions):
        for b in transitions[i + 1 :]:
            if a["process"] == b["process"]:
                continue
            ra, wa, rb, wb = set(a["reads"]), set(a["writes"]), set(b["reads"]), set(b["writes"])
            if not ((wa & wb) | (wa & rb) | (ra & wb)):
                independent.append([a["name"], b["name"]])
            if (wa & expr_vars(b["guard"])) or (wb & expr_vars(a["guard"])):
                conflicts.append([a["name"], b["name"]])
    return {
        "name": f"gen-{seed}",
        "variables": variables,
        "init": init,
        "obligations": obligations,
        "transitions": transitions,
        "independent": independent,
        "conflicts": conflicts,
    }
