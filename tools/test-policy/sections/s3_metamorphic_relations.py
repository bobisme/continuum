"""docs/19 §3 "Metamorphic relations" — TEST-3-01 … TEST-3-06.

The section text lists, under "Expected preservation": alpha-renaming;
stable reordering of declarations; set/map insertion order; splitting a
deterministic action into stuttering substeps with a valid view; joining
adjacent internal stutter steps; symmetry renaming. (Two further bullets,
independent-event swap and equivalent guard normalization, plus the
serialization/snapshot pair and the "expected non-preservation" tests, are
left to a later section module — TEST-3-07..10 are not claimed here.)

Each of the six relations here is a *transform* on a `tsys` system plus an
*equivalence checker* that must hold between the system and its transform.
The transform and the checker are independent code paths (the checker never
assumes its input came from the matching transform), so a checker that never
fires would be a vacuous pass; the corpus-level non-vacuity checks below and
the mutation checks (a deliberately broken transform, which the checker must
catch) both guard against that.

Two equivalence notions recur:

1. **Renaming equivalence** (`_renaming_equivalence`): the transform is a
   bijection on names (variables, obligations); the checker translates every
   reachable state of the base system through that bijection and requires the
   translated set to equal the transformed system's own reachable set exactly,
   plus equal state/interleaving/trace-class counts. TEST-3-01 uses an
   arbitrary bijection over every identifier; TEST-3-06 uses one restricted to
   swapping the labels of two processes, which is a symmetry exactly when the
   swap turns out to be an automorphism.
2. **View equivalence** (`_view_equal`): the transform changes the system's
   own state shape (an extra pc value, an extra stutter transition); the
   checker projects every reachable state of each system onto "every variable
   except the split process's own program counter" and requires the projected
   sets to be equal. TEST-3-04 and TEST-3-05 use this.

TEST-3-02 and TEST-3-03 use a third, simpler notion (`_reorder_equivalence`):
the transform only reorders a declaration list, so no renaming and no state
reshaping is involved, and the checker requires the reachable state sets and
oracle metrics (including the sleep-set reduction counts, which do depend on
the *set*, not the order, of the declared independence relation) to be
identical.

Boundaries common to every ID here are in `BOUNDARIES`; see also
`tools/test-policy/README-test.md`.
"""

from __future__ import annotations

import copy
import functools
from typing import Any, Callable

import tsys

SECTION = 3
TITLE = "Metamorphic relations"
OBLIGATIONS = {
    "TEST-3-01": "alpha-renaming",
    "TEST-3-02": "stable reordering of declarations",
    "TEST-3-03": "set/map insertion order",
    "TEST-3-04": "splitting a deterministic action into stuttering substeps with a valid view",
    "TEST-3-05": "joining adjacent internal stutter steps",
    "TEST-3-06": "symmetry renaming",
}
RULES = {
    "alpha-rename-preserved": "TEST-3-01",
    "declaration-reorder-preserved": "TEST-3-02",
    "relation-order-preserved": "TEST-3-03",
    "split-stutter-preserved": "TEST-3-04",
    "join-stutter-preserved": "TEST-3-05",
    "symmetry-rename-preserved": "TEST-3-06",
}
SEEDS = range(256)

_ENGINE_BOUNDARY = (
    "continuum-engine-reference's semantic oracle module "
    "(crates/continuum-engine-reference/src/semantic/, bn-1zgs) is a differential "
    "defect oracle for the same generated-system concept as TEST §2; it exposes no "
    "alpha-renaming, reordering, splitting, or symmetry transform of its own. "
    "continuum-engine-explicit and continuum-engine-dpor, where a real reduction "
    "or symmetry engine would run, remain documented stubs (the same boundary "
    "TEST-2-04 records). This harness's own tsys-based oracle is therefore what "
    "checks every TEST-3-01..06 relation; no optimized or Rust-side engine is "
    "compared against it yet."
)

BOUNDARIES = {
    "TEST-3-01": [
        _ENGINE_BOUNDARY,
        "The renaming covers structural identifiers (variable, process, transition, "
        "and obligation names) only. Enum values are literal data, not identifiers, "
        "and are not renamed.",
    ],
    "TEST-3-02": [
        _ENGINE_BOUNDARY,
        "'Declaration' means the system's own `variables` and `transitions` lists. "
        "tsys.compile_system already sorts both by name, so this relation also "
        "documents an existing invariant of the oracle, not just a property under test.",
    ],
    "TEST-3-03": [
        _ENGINE_BOUNDARY,
        "'Set/map' means the system's own unordered declaration lists — "
        "`independent`, `conflicts`, `obligations` — spelled as JSON arrays. tsys "
        "has no first-class set or map variable type (TEST-2-01's boundary: "
        "'Records, sets, maps, and sequences are not generated'), so this is the "
        "closest set/map-shaped construct the modeled language has.",
    ],
    "TEST-3-04": [
        _ENGINE_BOUNDARY,
        "Only the generator's straight-line per-process action sequence is split, "
        "and only at the last step of the lowest-named process (so no later step's "
        "guard needs renumbering). The view used to check the relation is fixed: "
        "every variable except the split process's own program counter. The "
        "view-indexed Indep(e,f | View, PropertyClass) of docs/02 §4 is not modeled.",
    ],
    "TEST-3-05": [
        _ENGINE_BOUNDARY,
        "Built on TEST-3-04's split (two adjacent stutter steps are produced by "
        "splitting with `extra=2`), so it shares that boundary's scope.",
    ],
    "TEST-3-06": [
        _ENGINE_BOUNDARY,
        "In this generator's model, processes never read one another's program "
        "counter, so swapping two process labels is *always* sound (an "
        "isomorphism), whether or not the two processes run the same code — it "
        "is mathematically the alpha-renaming of TEST-3-01 restricted to one "
        "transposition. Symmetry in the sense that matters for a symmetry-"
        "reduction engine (collapsing genuinely interchangeable states, "
        "recognizing that a swap is a no-op on the declaration, not just "
        "semantics-preserving) needs the swap to be checked against the "
        "declaration itself, which this harness does not do; "
        "continuum-engine-explicit/continuum-engine-dpor, where such a "
        "reduction would run, remain stubs (the same boundary TEST-2-04 "
        "records). Positive evidence here uses a purpose-built two-process-"
        "symmetric generator (`generate_symmetric`) so the corpus is at least "
        "about literally interchangeable components; the corruption mutant "
        "confirms the checker is not vacuous.",
    ],
}


# ---------------------------------------------------------------------------
# Shared equivalence checkers
# ---------------------------------------------------------------------------


def _renaming_equivalence(
    base: dict, transformed: dict, rename_var: Callable[[str], str], rename_obl: Callable[[str], str]
) -> tuple[bool, str]:
    """`transformed` must be the image of `base` under the bijection
    (`rename_var` on variable names, `rename_obl` on obligation names): every
    reachable state of `base`, translated through the bijection, must be a
    reachable state of `transformed`, and no other state of `transformed` may
    be reachable."""
    fb = tsys.check_static(base)
    ft = tsys.check_static(transformed)
    if fb or ft:
        return False, f"static findings: base={len(fb)} transformed={len(ft)}"
    rb = tsys.explore(base)
    rt = tsys.explore(transformed)
    bc = tsys.compile_system(base)
    tc = tsys.compile_system(transformed)

    def translate(state: tsys.State) -> tuple | None:
        values, held = state
        env = dict(zip(bc.order, values))
        new_env = {rename_var(k): v for k, v in env.items()}
        if set(tc.order) - set(new_env):
            return None
        new_values = tuple(new_env[k] for k in tc.order)
        new_held = tuple(sorted(rename_obl(o) for o in held))
        return (new_values, new_held)

    mapped = set()
    for s in rb.reachable:
        ts = translate(s)
        if ts is None:
            return False, "the renaming does not cover the transformed system's variables"
        mapped.add(ts)
    if mapped != set(rt.reachable):
        return False, "reachable state sets disagree under the renaming"
    if (rb.states, rb.interleavings, rb.trace_classes) != (rt.states, rt.interleavings, rt.trace_classes):
        return False, "aggregate oracle metrics disagree under the renaming"
    return True, ""


def _reorder_equivalence(base: dict, transformed: dict) -> tuple[bool, str]:
    """`transformed` must differ from `base` only in the order of a
    declaration list: same reachable states, same oracle metrics (including
    the sleep-set reduction counts, which depend on the declared independence
    *set*, not its order)."""
    fb = tsys.check_static(base)
    ft = tsys.check_static(transformed)
    if fb or ft:
        return False, f"static findings: base={len(fb)} transformed={len(ft)}"
    rb = tsys.explore(base)
    rt = tsys.explore(transformed)
    if set(rb.reachable) != set(rt.reachable):
        return False, "reachable state sets differ"
    metrics_b = (rb.states, rb.interleavings, rb.trace_classes, rb.reduced_runs, rb.reduced_classes)
    metrics_t = (rt.states, rt.interleavings, rt.trace_classes, rt.reduced_runs, rt.reduced_classes)
    if metrics_b != metrics_t:
        return False, f"aggregate oracle metrics differ: {metrics_b} != {metrics_t}"
    return True, ""


def _view_equal(base: dict, transformed: dict, pc: str) -> tuple[bool, str]:
    """`transformed` must present the same behaviour as `base` to a view that
    does not observe `pc`: the two systems' reachable states, projected onto
    every variable except `pc`, must be the same set."""
    fb = tsys.check_static(base)
    ft = tsys.check_static(transformed)
    if fb or ft:
        return False, f"static findings: base={len(fb)} transformed={len(ft)}"
    rb = tsys.explore(base)
    rt = tsys.explore(transformed)
    bc = tsys.compile_system(base)
    tc = tsys.compile_system(transformed)

    def view(c: tsys.Compiled, s: tsys.State) -> tuple:
        env = dict(zip(c.order, s[0]))
        return (tuple(sorted((k, v) for k, v in env.items() if k != pc)), s[1])

    bv = {view(bc, s) for s in rb.reachable}
    tv = {view(tc, s) for s in rt.reachable}
    if bv != tv:
        return False, f"view-projected reachable sets differ under a view excluding {pc!r}"
    return True, ""


# ---------------------------------------------------------------------------
# TEST-3-01: alpha-renaming
# ---------------------------------------------------------------------------


def _rename_id(name: str) -> str:
    """A fixed injective string transform used as one uniform alpha-renaming
    of every identifier: different inputs give different outputs (a reversed
    string is injective, and the constant prefix keeps it that way)."""
    return "r_" + name[::-1]


def alpha_rename(system: dict) -> dict:
    def rres(res: str) -> str:
        if res.startswith("obligation:"):
            return "obligation:" + _rename_id(res[len("obligation:") :])
        return _rename_id(res)

    def rexpr(e: Any) -> Any:
        if isinstance(e, dict):
            if "var" in e:
                return {"var": _rename_id(e["var"])}
            if "const" in e:
                return dict(e)
            if "op" in e:
                return {"op": e["op"], "args": [rexpr(a) for a in e["args"]]}
        return e

    new_vars = [{"name": _rename_id(v["name"]), "type": v["type"]} for v in system["variables"]]
    new_init = {_rename_id(k): v for k, v in system["init"].items()}
    new_obls = [{"name": _rename_id(o["name"]), "owner": _rename_id(o["owner"])} for o in system["obligations"]]
    new_trans = []
    for t in system["transitions"]:
        nt: dict[str, Any] = {
            "name": _rename_id(t["name"]),
            "process": _rename_id(t["process"]),
            "updates": {_rename_id(k): rexpr(v) for k, v in t["updates"].items()},
        }
        if "guard" in t:
            nt["guard"] = rexpr(t["guard"])
        nt["reads"] = sorted(rres(r) for r in t.get("reads", []))
        nt["writes"] = sorted(rres(w) for w in t.get("writes", []))
        if "acquire" in t:
            nt["acquire"] = [_rename_id(o) for o in t["acquire"]]
        if "discharge" in t:
            nt["discharge"] = [_rename_id(o) for o in t["discharge"]]
        new_trans.append(nt)
    new_indep = []
    for item in system["independent"]:
        if isinstance(item, dict):
            a, b = item["pair"]
            new_indep.append({"pair": [_rename_id(a), _rename_id(b)], "justification": item["justification"]})
        else:
            a, b = item
            new_indep.append([_rename_id(a), _rename_id(b)])
    new_conf = [[_rename_id(a), _rename_id(b)] for a, b in system["conflicts"]]
    return {
        "name": _rename_id(system["name"]),
        "variables": new_vars,
        "init": new_init,
        "obligations": new_obls,
        "transitions": new_trans,
        "independent": new_indep,
        "conflicts": new_conf,
    }


def _corrupt_rename(base: dict, renamed: dict) -> dict:
    """A partial rename: one variable's `init` entry is left under its
    original (pre-rename) key, as an incomplete find/replace would leave it."""
    orig = base["variables"][0]["name"]
    ren = _rename_id(orig)
    m = copy.deepcopy(renamed)
    if ren in m["init"]:
        m["init"][orig] = m["init"].pop(ren)
    return m


# ---------------------------------------------------------------------------
# TEST-3-02 / TEST-3-03: reordering
# ---------------------------------------------------------------------------


def _shuffle(items: list, rng: "tsys.SplitMix64") -> list:
    items = list(items)
    for i in range(len(items) - 1, 0, -1):
        j = rng.below(i + 1)
        items[i], items[j] = items[j], items[i]
    return items


def _order_changed(a: list, b: list) -> bool:
    return [tsys.canonical(x) for x in a] != [tsys.canonical(x) for x in b]


def shuffle_declarations(system: dict, seed: int) -> dict:
    rng = tsys.SplitMix64(seed ^ 0xD5123456789ABC)
    m = copy.deepcopy(system)
    m["variables"] = _shuffle(m["variables"], rng)
    m["transitions"] = _shuffle(m["transitions"], rng)
    return m


def shuffle_relations(system: dict, seed: int) -> dict:
    rng = tsys.SplitMix64(seed ^ 0xA59876543210FE)
    m = copy.deepcopy(system)
    m["independent"] = _shuffle(m["independent"], rng)
    m["conflicts"] = _shuffle(m["conflicts"], rng)
    m["obligations"] = _shuffle(m["obligations"], rng)
    return m


def _corrupt_drop_transition(system: dict, name: str) -> dict:
    """Drop the named transition. The caller picks `name` from a prior oracle
    run's `fired` set, so the drop is guaranteed to change reachability
    instead of removing dead code a guard-blocked corpus never reaches."""
    m = copy.deepcopy(system)
    m["transitions"] = [t for t in m["transitions"] if t["name"] != name]

    def mentions(item: Any) -> bool:
        pair = item["pair"] if isinstance(item, dict) else item
        return name in pair

    m["independent"] = [p for p in m["independent"] if not mentions(p)]
    m["conflicts"] = [p for p in m["conflicts"] if not mentions(p)]
    return m


def _corrupt_drop_relation(system: dict, pair: tuple[str, str]) -> dict:
    """Drop the named independent pair. The caller picks `pair` from a prior
    oracle run's `coenabled_independent` set, so the pair actually fed the
    sleep-set reduction instead of being declaratively inert."""
    m = copy.deepcopy(system)
    a, b = pair

    def mentions(item: Any) -> bool:
        p = item["pair"] if isinstance(item, dict) else item
        return set(p) == {a, b}

    m["independent"] = [p for p in m["independent"] if not mentions(p)]
    return m


# ---------------------------------------------------------------------------
# TEST-3-04 / TEST-3-05: splitting and joining stutter steps
# ---------------------------------------------------------------------------


def _first_process(system: dict) -> str:
    return min(t["process"] for t in system["transitions"])


def split_last_step(system: dict, extra: int) -> dict:
    """Stretch the completion of the lowest-named process's last transition
    across `1 + extra` steps: the transition itself is untouched (it now lands
    on an intermediate pc value instead of the old terminal one), followed by
    `extra` pure pc-advancing stutter transitions that reach the real terminal
    value. No later step needs renumbering, because this is the last one."""
    m = copy.deepcopy(system)
    if extra <= 0:
        return m
    proc = _first_process(m)
    pc = f"pc_{proc}"
    var = next(v for v in m["variables"] if v["name"] == pc)
    old_max = var["type"]["max"]
    var["type"]["max"] = old_max + extra
    for i in range(extra):
        frm, to = old_max + i, old_max + i + 1
        m["transitions"].append(
            {
                "name": f"{proc}.stutter{i}",
                "process": proc,
                "guard": {"op": "eq", "args": [{"var": pc}, {"const": frm}]},
                "updates": {pc: {"const": to}},
                "reads": [pc],
                "writes": [pc],
            }
        )
    return m


def join_last_two_stutters(system: dict) -> dict:
    """The converse of `split_last_step(_, 2)`: merge the two adjacent
    internal stutter transitions it produced into one."""
    m = copy.deepcopy(system)
    stutters = sorted((t for t in m["transitions"] if ".stutter" in t["name"]), key=lambda t: t["name"])
    if len(stutters) != 2:
        raise ValueError(f"expected exactly two stutter transitions to join, found {len(stutters)}")
    first, second = stutters
    proc = first["process"]
    pc = f"pc_{proc}"
    frm = first["guard"]["args"][1]["const"]
    to = second["updates"][pc]["const"]
    joined = {
        "name": f"{proc}.stutter0",
        "process": proc,
        "guard": {"op": "eq", "args": [{"var": pc}, {"const": frm}]},
        "updates": {pc: {"const": to}},
        "reads": [pc],
        "writes": [pc],
    }
    m["transitions"] = [t for t in m["transitions"] if t not in stutters] + [joined]
    return m


def _always_different(name: str, decl: dict) -> dict:
    """An update expression whose result is always different from `name`'s
    current value, whatever that value is (a fixed constant could coincide
    with the value the real computation already produces, especially over
    small domains, making the corruption an accidental no-op)."""
    kind = decl["kind"]
    if kind == "bool":
        return {"op": "not", "args": [{"var": name}]}
    if kind == "int":
        return {
            "op": "ite",
            "args": [
                {"op": "lt", "args": [{"var": name}, {"const": decl["max"]}]},
                {"op": "add", "args": [{"var": name}, {"const": 1}]},
                {"const": decl["min"]},
            ],
        }
    values = decl["values"]
    expr: dict = {"const": values[0]}
    for i, v in enumerate(values):
        nxt = values[(i + 1) % len(values)]
        expr = {"op": "ite", "args": [{"op": "eq", "args": [{"var": name}, {"const": v}]}, {"const": nxt}, expr]}
    return expr


def _corrupt_stutter(system: dict) -> dict:
    """Make the (sole) `.stutter0` step also touch a shared variable, so it is
    no longer a pure stutter under the view that excludes only the pc."""
    m = copy.deepcopy(system)
    shared = next(v for v in m["variables"] if not v["name"].startswith("pc_"))
    name = shared["name"]
    stutter = next(t for t in m["transitions"] if t["name"].endswith(".stutter0"))
    stutter["updates"][name] = _always_different(name, shared["type"])
    stutter["reads"] = sorted(set(stutter["reads"]) | {name})
    stutter["writes"] = sorted(set(stutter["writes"]) | {name})
    return m


# ---------------------------------------------------------------------------
# TEST-3-06: symmetry renaming
# ---------------------------------------------------------------------------


def _swap_name(pa: str, pb: str, name: str) -> str:
    return pb if name == pa else pa if name == pb else name


def _swap_var(pa: str, pb: str, name: str) -> str:
    if name == f"pc_{pa}":
        return f"pc_{pb}"
    if name == f"pc_{pb}":
        return f"pc_{pa}"
    return name


def _swap_obl(pa: str, pb: str, name: str) -> str:
    if name == f"o_{pa}":
        return f"o_{pb}"
    if name == f"o_{pb}":
        return f"o_{pa}"
    return name


def swap_processes(system: dict, pa: str, pb: str) -> dict:
    sw = functools.partial(_swap_name, pa, pb)
    sw_var = functools.partial(_swap_var, pa, pb)
    sw_obl = functools.partial(_swap_obl, pa, pb)

    def sw_res(res: str) -> str:
        if res.startswith("obligation:"):
            return "obligation:" + sw_obl(res[len("obligation:") :])
        return sw_var(res)

    def sw_expr(e: Any) -> Any:
        if isinstance(e, dict):
            if "var" in e:
                return {"var": sw_var(e["var"])}
            if "const" in e:
                return dict(e)
            if "op" in e:
                return {"op": e["op"], "args": [sw_expr(a) for a in e["args"]]}
        return e

    def sw_tname(tname: str) -> str:
        proc, rest = tname.split(".", 1)
        return f"{sw(proc)}.{rest}"

    new_vars = [{"name": sw_var(v["name"]), "type": v["type"]} for v in system["variables"]]
    new_init = {sw_var(k): v for k, v in system["init"].items()}
    new_obls = [{"name": sw_obl(o["name"]), "owner": sw(o["owner"])} for o in system["obligations"]]
    new_trans = []
    for t in system["transitions"]:
        nt: dict[str, Any] = {
            "name": sw_tname(t["name"]),
            "process": sw(t["process"]),
            "updates": {sw_var(k): sw_expr(v) for k, v in t["updates"].items()},
        }
        if "guard" in t:
            nt["guard"] = sw_expr(t["guard"])
        nt["reads"] = sorted(sw_res(r) for r in t.get("reads", []))
        nt["writes"] = sorted(sw_res(w) for w in t.get("writes", []))
        if "acquire" in t:
            nt["acquire"] = [sw_obl(o) for o in t["acquire"]]
        if "discharge" in t:
            nt["discharge"] = [sw_obl(o) for o in t["discharge"]]
        new_trans.append(nt)
    new_indep = []
    for item in system["independent"]:
        if isinstance(item, dict):
            a, b = item["pair"]
            new_indep.append({"pair": [sw_tname(a), sw_tname(b)], "justification": item["justification"]})
        else:
            a, b = item
            new_indep.append([sw_tname(a), sw_tname(b)])
    new_conf = [[sw_tname(a), sw_tname(b)] for a, b in system["conflicts"]]
    return {
        "name": system["name"] + "-swap",
        "variables": new_vars,
        "init": new_init,
        "obligations": new_obls,
        "transitions": new_trans,
        "independent": new_indep,
        "conflicts": new_conf,
    }


def _corrupt_swap(swapped: dict, pa: str, pb: str) -> dict:
    """Leave one transition's *declared* footprint naming the pre-swap pc
    while its guard/update correctly reference the post-swap one — a
    declared-footprint/actual-reference mismatch, as an incomplete
    substitution would create. Both `pc_{pa}` and `pc_{pb}` remain declared
    variables after a swap (only which process each one belongs to changes),
    so this is a real defect, not a reference to an undeclared name."""
    m = copy.deepcopy(swapped)
    right = f"pc_{pb}"  # pa's pc after the swap
    wrong = f"pc_{pa}"
    for t in m["transitions"]:
        if right in t.get("reads", []):
            t["reads"] = sorted((set(t["reads"]) - {right}) | {wrong})
            return m
    raise ValueError(f"no transition reads {right!r} to corrupt")


def generate_symmetric(seed: int) -> dict:
    """A system whose two processes p0 and p1 are literally interchangeable:
    both run one identical template of guarded steps over the same shared
    variables (built once, applied twice), so swapping their labels is a true
    automorphism. `tsys.generate` does not guarantee this (its processes are
    independently randomized), so this is a separate, minimal generator."""
    rng = tsys.SplitMix64(seed)
    kinds = ["int", "bool", "enum"]
    shared: list[tuple[str, dict]] = []
    variables: list[dict] = []
    init: dict[str, Any] = {}
    for i in range(1 + rng.below(2)):
        kind = kinds[(i + rng.below(3)) % 3]
        vname = f"v{i}"
        if kind == "int":
            decl: dict[str, Any] = {"kind": "int", "min": 0, "max": 1 + rng.below(3)}
            init[vname] = 0
        elif kind == "bool":
            decl = {"kind": "bool"}
            init[vname] = False
        else:
            decl = {"kind": "enum", "values": ["a", "b", "c"][: 2 + rng.below(2)]}
            init[vname] = "a"
        variables.append({"name": vname, "type": decl})
        shared.append((vname, decl))

    steps = 1 + rng.below(3)
    template: list[tuple[list[tuple[str, str, Any]], tuple[str, Any] | None]] = []
    for _k in range(steps):
        conds: list[tuple[str, str, Any]] = []
        if rng.below(3) != 0:
            vname, decl = rng.choice(shared)
            if decl["kind"] == "int":
                conds.append((vname, rng.choice(["lt", "eq"]), rng.below(decl["max"] + 1)))
            elif decl["kind"] == "bool":
                conds.append((vname, "eq", rng.below(2) == 0))
            else:
                conds.append((vname, "eq", rng.choice(decl["values"])))
        update = None
        if rng.below(2) != 0:
            vname, decl = rng.choice(shared)
            if decl["kind"] == "int":
                update = (vname, rng.below(decl["max"] + 1))
            elif decl["kind"] == "bool":
                update = (vname, rng.below(2) == 0)
            else:
                update = (vname, rng.choice(decl["values"]))
        template.append((conds, update))

    transitions = []
    for proc in ("p0", "p1"):
        pc = f"pc_{proc}"
        variables.append({"name": pc, "type": {"kind": "int", "min": 0, "max": steps}})
        init[pc] = 0
        for k, (conds, update) in enumerate(template):
            gconds: list[dict] = [{"op": "eq", "args": [{"var": pc}, {"const": k}]}]
            for vname, op, val in conds:
                gconds.append({"op": op, "args": [{"var": vname}, {"const": val}]})
            guard = gconds[0] if len(gconds) == 1 else {"op": "and", "args": gconds}
            updates: dict[str, Any] = {pc: {"op": "add", "args": [{"var": pc}, {"const": 1}]}}
            if update is not None:
                vname, val = update
                updates[vname] = {"const": val}
            reads = set(tsys.expr_vars(guard))
            for e in updates.values():
                reads |= tsys.expr_vars(e)
            writes = set(updates)
            transitions.append(
                {
                    "name": f"{proc}.s{k}",
                    "process": proc,
                    "guard": guard,
                    "updates": updates,
                    "reads": sorted(reads),
                    "writes": sorted(writes),
                }
            )
    return {
        "name": f"sym-{seed}",
        "variables": variables,
        "init": init,
        "obligations": [],
        "transitions": transitions,
        "independent": [],
        "conflicts": [],
    }


# ---------------------------------------------------------------------------
# Fixtures dispatch
# ---------------------------------------------------------------------------

_CHECKERS: dict[str, Callable[[dict], tuple[bool, str]]] = {
    "alpha-rename": lambda p: _renaming_equivalence(p["base"], p["transformed"], _rename_id, _rename_id),
    "declaration-reorder": lambda p: _reorder_equivalence(p["base"], p["transformed"]),
    "relation-order": lambda p: _reorder_equivalence(p["base"], p["transformed"]),
    "split-stutter": lambda p: _view_equal(p["base"], p["transformed"], p["pc"]),
    "join-stutter": lambda p: _view_equal(p["base"], p["transformed"], p["pc"]),
    "symmetry-rename": lambda p: _renaming_equivalence(
        p["base"],
        p["transformed"],
        functools.partial(_swap_var, p["pa"], p["pb"]),
        functools.partial(_swap_obl, p["pa"], p["pb"]),
    ),
}

_RELATION_RULE = {
    "alpha-rename": "alpha-rename-preserved",
    "declaration-reorder": "declaration-reorder-preserved",
    "relation-order": "relation-order-preserved",
    "split-stutter": "split-stutter-preserved",
    "join-stutter": "join-stutter-preserved",
    "symmetry-rename": "symmetry-rename-preserved",
}


def check_fixture(system: Any) -> list[dict[str, str]]:
    payload = system
    relation = payload["relation"]
    ok, detail = _CHECKERS[relation](payload)
    if ok:
        return []
    rule = _RELATION_RULE[relation]
    return [
        {
            "rule": rule,
            "requirement": RULES[rule],
            "subject": payload.get("subject", relation),
            "message": detail,
        }
    ]


# ---------------------------------------------------------------------------
# The real run
# ---------------------------------------------------------------------------


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
        """Like `mutate`, but a single miss is not itself a failure: over a
        tiny reachable state space (a handful of bool/enum values), a
        corrupted stutter's new state can coincide with an already-reachable
        view by chance, independent of whether the checker works. Only
        `detected == 0` across the whole corpus (checked after the loop) means
        the checker is vacuous."""
        mutants[rid]["applied"] += 1
        if detected:
            mutants[rid]["detected"] += 1

    for seed in SEEDS:
        base = tsys.generate(seed)
        proc = _first_process(base)
        pc = f"pc_{proc}"
        rb = tsys.explore(base)
        last_step = max(int(t["name"].rsplit(".s", 1)[1]) for t in base["transitions"] if t["process"] == proc)
        last_name = f"{proc}.s{last_step}"

        # TEST-3-01: alpha-renaming.
        renamed = alpha_rename(base)
        ok, detail = _renaming_equivalence(base, renamed, _rename_id, _rename_id)
        bump("TEST-3-01", "systems")
        if ok:
            bump("TEST-3-01", "identifiers_renamed", len(base["variables"]) + len(base["transitions"]) + len(base["obligations"]))
        else:
            failures["TEST-3-01"].append(f"gen-{seed}: {detail}")
        corrupted = _corrupt_rename(base, renamed)
        cok, _ = _renaming_equivalence(base, corrupted, _rename_id, _rename_id)
        mutate("TEST-3-01", not cok, f"gen-{seed} (partial rename)")

        # TEST-3-02: stable reordering of declarations.
        reordered = shuffle_declarations(base, seed)
        ok2, detail2 = _reorder_equivalence(base, reordered)
        bump("TEST-3-02", "systems")
        if ok2:
            if _order_changed(base["variables"], reordered["variables"]) or _order_changed(base["transitions"], reordered["transitions"]):
                bump("TEST-3-02", "orders_changed")
        else:
            failures["TEST-3-02"].append(f"gen-{seed}: {detail2}")
        if rb.fired:
            drop_name = sorted(rb.fired)[-1]
            dropped = _corrupt_drop_transition(reordered, drop_name)
            dok, _ = _reorder_equivalence(base, dropped)
            mutate("TEST-3-02", not dok, f"gen-{seed} (dropped {drop_name})")

        # TEST-3-03: set/map insertion order.
        relord = shuffle_relations(base, seed)
        ok3, detail3 = _reorder_equivalence(base, relord)
        bump("TEST-3-03", "systems")
        if ok3:
            if len(base["independent"]) >= 2 or len(base["conflicts"]) >= 2 or len(base["obligations"]) >= 2:
                bump("TEST-3-03", "shuffle_eligible")
        else:
            failures["TEST-3-03"].append(f"gen-{seed}: {detail3}")
        if rb.coenabled_independent:
            pair = sorted(rb.coenabled_independent)[0]
            dropped3 = _corrupt_drop_relation(relord, pair)
            dok3, _ = _reorder_equivalence(base, dropped3)
            mutate("TEST-3-03", not dok3, f"gen-{seed} (dropped {pair[0]}|{pair[1]})")

        # TEST-3-04: split a deterministic action into stuttering substeps.
        split = split_last_step(base, 1)
        ok4, detail4 = _view_equal(base, split, pc)
        bump("TEST-3-04", "systems")
        if ok4:
            rt4 = tsys.explore(split)
            bump("TEST-3-04", "extra_states_from_the_stutter", rt4.states - rb.states)
        else:
            failures["TEST-3-04"].append(f"gen-{seed}: {detail4}")
        if last_name in rb.fired:
            bad_split = _corrupt_stutter(split)
            bok4, _ = _view_equal(base, bad_split, pc)
            mutate_soft("TEST-3-04", not bok4)

        # TEST-3-05: join adjacent internal stutter steps.
        before5 = split_last_step(base, 2)
        after5 = join_last_two_stutters(before5)
        ok5, detail5 = _view_equal(before5, after5, pc)
        bump("TEST-3-05", "systems")
        if not ok5:
            failures["TEST-3-05"].append(f"gen-{seed}: {detail5}")
        if last_name in rb.fired:
            bad_join = _corrupt_stutter(after5)
            bok5, _ = _view_equal(before5, bad_join, pc)
            mutate_soft("TEST-3-05", not bok5)

        # TEST-3-06: symmetry renaming. A swap of two process labels is sound
        # under any independent-process system (it is a restricted case of
        # TEST-3-01's alpha-renaming; see BOUNDARIES), so positive evidence
        # does not need contrived symmetry — the purpose-built generator below
        # is used anyway, to keep this obligation's evidence about literally
        # interchangeable components rather than piggybacking on TEST-3-01's.
        # The mutation is a corrupted (partial) swap, exactly as TEST-3-01's.
        sym = generate_symmetric(seed)
        swapped = swap_processes(sym, "p0", "p1")
        sw_var6 = functools.partial(_swap_var, "p0", "p1")
        sw_obl6 = functools.partial(_swap_obl, "p0", "p1")
        ok6, detail6 = _renaming_equivalence(sym, swapped, sw_var6, sw_obl6)
        bump("TEST-3-06", "symmetric_systems")
        if not ok6:
            failures["TEST-3-06"].append(f"sym-{seed}: {detail6}")
        bad_swap = _corrupt_swap(swapped, "p0", "p1")
        bok6, _ = _renaming_equivalence(sym, bad_swap, sw_var6, sw_obl6)
        mutate("TEST-3-06", not bok6, f"sym-{seed} (partial swap)")

    def need(rid: str, key: str, what: str) -> None:
        if corpus[rid].get(key, 0) == 0:
            failures[rid].append(f"corpus exercises no {what} ({key} = 0): the pass would be vacuous")

    need("TEST-3-01", "identifiers_renamed", "successfully renamed system")
    need("TEST-3-02", "orders_changed", "system whose declaration order actually changed under the shuffle")
    need("TEST-3-03", "shuffle_eligible", "system with a reorderable relation list (>=2 entries)")
    need("TEST-3-04", "extra_states_from_the_stutter", "split that actually introduces a new reachable state")
    need("TEST-3-05", "systems", "joined system")
    need("TEST-3-06", "symmetric_systems", "symmetric system pair")

    for rid in OBLIGATIONS:
        if mutants[rid]["detected"] == 0:
            failures[rid].append(f"{rid}: the mutant was never detected across the corpus (the checker would be vacuous)")

    results = {}
    for rid, summary in OBLIGATIONS.items():
        results[rid] = {
            "status": "enforced",
            "rules": sorted(r for r, q in RULES.items() if q == rid),
            "failures": failures[rid],
            "corpus": corpus[rid],
            "mutant": mutants[rid],
            "boundaries": BOUNDARIES[rid],
        }
    return {
        "seeds": f"{SEEDS.start}..{SEEDS.stop - 1}",
        "requirements": results,
        "unclaimed": ["TEST-3-07", "TEST-3-08", "TEST-3-09", "TEST-3-10"],
    }
