#!/usr/bin/env python3
"""Independent path audit for C023: the semantic triptych avoids circular self-validation.

Claim row: `notes/plan/docs/18_CLAIMS_MATRIX.md` C023, required evidence
"independent path audit and mutation tests". Owning Bone: bn-2270.

RFC 0013 splits meaning into three planes (Model, Program, Proof) and forbids a
single implementation from justifying agreement with itself: "They may not all
call the same evaluator to justify agreement." docs/33 "Architectural
separation" names the producer/checker pairs, and plan §20 names the edges the
trusted checking base may not have. This tool checks those separations over the
real workspace, by a path that shares no code with `tools/check_crate_boundaries.py`:

- the graph comes from `cargo metadata --format-version 1 --locked --offline`
  with the full `resolve` section, so external crates (the `asupersync` runtime
  itself) and dev edges are in it. The boundary gate reads `--no-deps` and
  enforces normal and build edges only;
- the roles come from name patterns over the actual members
  (`continuum-kernel-*`, `continuum-engine-*`), not from a static list. A new
  kernel or engine crate joins its role without an edit here, and
  `gate-coverage` fails if the boundary gate's own lists did not follow.

Rules (each a finding with a witness path when it fails):

- `checker-closed`: the checking base (`continuum-kernel-*` plus
  `continuum-certificate`) links, in normal/build position, only other
  checking-base crates, and never the `asupersync` runtime. Stronger than
  "not search": the checker cannot reach any producer, synchronizer or runtime.
- `checker-dev-closed`: no dev-dependency of a checking-base crate reaches a
  producer. A checker test suite that links an engine could validate the
  checker against the engine's in-memory output. The boundary gate does not
  enforce dev edges, by design (its docstring), so this rule is audit-only.
- `producer-does-not-link-checker`: no producer (`continuum-engine-*`,
  `continuum-forge`, `continuum-asupersync`) links the checking base in
  normal/build position, so no producer can check its own output in-process
  and ship the verdict. Dev position is the admitted differential shape.
- `model-not-derived-from-program`: the model plane (`continuum-model-core`,
  `continuum-cml-*`, the reference evaluator `continuum-engine-reference`) does
  not link the program plane (`continuum-asupersync`, `continuum-effects-*`,
  the `asupersync` runtime).
- `program-not-derived-from-model`: the reverse direction. Shared declarative
  crates (`continuum-value`, schemas) are allowed on both sides (RFC 0013,
  "Shared artifacts").
- `refinement-independent`: `continuum-refinement`, the model/program
  correspondence checker, links no producer and no program-plane crate.
- `validator-not-upstream`: no model, program, checker or producer crate links
  `continuum-refinement`, so the validated side cannot share the validator's
  decision logic.
- `acyclic`: the workspace graph with normal, build AND dev edges has no cycle.
  Cargo refuses normal cycles itself; a dev cycle is legal to Cargo and is
  exactly how a checker and a producer could test each other against each other.
- `wire-form-only`: every public function of a checking-base crate that returns
  a verdict (`Verdict`, `Outcome`) takes exactly `bytes: &[u8]`, each crate has
  at least one (anti-vacuity), and no checking-base source line names a
  producer crate.
- `cross-path-tests-live`: the Rust tests that pin the wire-form boundary and
  the engine-to-kernel differential exist and are not `#[ignore]`d. This is
  the tie from this audit to real Rust tests over the real code.
- `a7-model-independent`: arrow A7's conformance model
  (`crates/continuum-asupersync/tests/support/primitive_conformance_model.rs`,
  bn-ujpz0) is a second path, not the adapter's own lift. It imports `std` or
  `core` only, loads no other file (`mod x;`, `#[path]`, `include!`), and no code
  line names a `continuum_*` crate or the `asupersync` runtime, so it can reach
  neither the adapter's types, decoder, lift or scripted source, nor the region
  calculus the lift uses. The A7 witness must load exactly that file. The model
  lives beside the adapter's tests only because Cargo integration tests are
  per-crate; the rule, not the directory, is what makes it the model side.
- `gate-coverage`: the boundary gate's `KERNEL`, `ENGINES` and `CHECKER` lists
  equal the pattern-derived roles, and for every (checker, producer) pair an
  injected edge makes `check_crate_boundaries.evaluate` report a violation.

The arrow table (`arrows`) is a report, not a rule: for each RFC 0013
cross-path arrow it records whether both sides exist today or one side is a
scaffold (a crate whose `src/` holds no code line). It is part of the retained
evidence, so when a scaffold side lands the evidence goes stale and the gate
fails until the C023 evidence record in docs/18 is re-scoped.

Mutation campaign (`--self-test`):

- every fixture in `tools/triptych/fixtures/*.json` overlays the real graph (or
  a checking-base source file) with one circular edge. Each must be rejected by
  this audit with exactly its expected rules, and by the boundary gate's
  `evaluate` with exactly its expected rules. Where the gate owns no rule for a
  mutant, the fixture says so and why. The unmutated graph must pass both;
- fixtures with an `e2e` block are also applied for real: the workspace
  manifests are copied under `$TMPDIR`, the manifest is edited, and the gate
  and this audit run as subprocesses on the copy. Both must exit 1 naming the
  expected rule; the unedited copy must exit 0 for both.

The Rust mutant (run by `--evidence` only, since it compiles a crate) applies the in-memory entry-point mutant
to a copy of `continuum-kernel-core` and runs the real Rust test that owns it;
the test must pass before the edit and fail after it.

The A7 Rust mutant (also `--evidence` only) edits a copy of the adapter's binding so
it journals a cancelled task's effect aborts before the task's acknowledgement. The A7
witness must pass before the edit and fail after it. The lift-based conformance test
of the same family must also pass before and fail after: since bn-1i050 the adapter's
own check refuses the order too, so both paths catch the mutant.

Usage:

    python3 tools/check_triptych_independence.py --self-test
    python3 tools/check_triptych_independence.py
    python3 tools/check_triptych_independence.py --evidence   # rewrite evidence

The real run also fails when `tools/triptych/evidence/c023.json` does not match
this revision's rules, roles, arrows, required tests and mutant catalogue.

Stdlib only. Exit 0 when every rule holds, 1 otherwise.
"""

from __future__ import annotations

import importlib.util
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field

ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tools" / "triptych" / "fixtures"
EVIDENCE = ROOT / "tools" / "triptych" / "evidence" / "c023.json"
GATE = ROOT / "tools" / "check_crate_boundaries.py"

# --- roles -----------------------------------------------------------------------

CERTIFICATE = "continuum-certificate"
FORGE = "continuum-forge"
ADAPTER = "continuum-asupersync"
RUNTIME = "asupersync"  # the external runtime crate the adapter binds
REFINEMENT = "continuum-refinement"
MODEL_NAMED = (
    "continuum-model-core",
    "continuum-cml-syntax",
    "continuum-cml-elab",
    "continuum-engine-reference",
)
VERDICT_TYPES = ("Verdict", "Outcome")
PRODUCER_IDENTS = ("continuum_engine_", "continuum_forge", "continuum_asupersync", "asupersync::")

RULES: dict[str, str] = {
    "checker-closed": "the checking base links only checking-base crates, never the runtime",
    "checker-dev-closed": "no checking-base dev-dependency reaches a producer",
    "producer-does-not-link-checker": "no producer links the checking base in normal/build position",
    "model-not-derived-from-program": "the model plane does not link the program plane",
    "program-not-derived-from-model": "the program plane does not link the model plane",
    "refinement-independent": "the refinement checker links no producer and no program-plane crate",
    "validator-not-upstream": "no validated-side crate links the refinement checker",
    "acyclic": "the workspace graph with normal, build and dev edges is acyclic",
    "wire-form-only": "every public verdict-returning checker function takes exactly `bytes: &[u8]`",
    "cross-path-tests-live": "the Rust tests pinning the wire boundary and the differential are live",
    "a7-model-independent": "the A7 conformance model imports std only, names no workspace crate or runtime, and the witness loads it",
    "gate-coverage": "the boundary gate's lists match the roles and catch every checker->producer edge",
}

# The Rust tests this audit leans on. Each is (crate-relative test file, test fn).
REQUIRED_TESTS: tuple[tuple[str, str], ...] = (
    *(
        (f"crates/continuum-kernel-{k}/tests/pr9_exit_evidence.rs", name)
        for k in ("core", "sat", "smt", "temporal")
        for name in (
            "checking_has_exactly_one_public_entry_point_and_it_takes_wire_form_bytes",
            "certificate_has_no_public_constructor_no_public_field_and_no_manual_trait_impl",
        )
    ),
    (
        "crates/continuum-certificate/tests/inv004_no_self_certification.rs",
        "checking_has_exactly_one_public_entry_point_and_it_takes_wire_form_bytes",
    ),
    (
        "crates/continuum-certificate/tests/inv004_no_self_certification.rs",
        "no_source_in_this_crate_names_a_decoded_kernel_structure",
    ),
    (
        "crates/continuum-certificate/tests/inv004_no_self_certification.rs",
        "the_manifest_names_no_forbidden_dependency_even_if_a_table_is_ever_added",
    ),
    (
        "crates/continuum-engine-reference/tests/certificate_kernel_differential.rs",
        "the_engines_die_hard_certificate_is_verified_by_the_independent_checker",
    ),
    (
        "crates/continuum-engine-reference/tests/certificate_kernel_differential.rs",
        "every_single_byte_mutation_of_the_state_table_is_rejected",
    ),
    (
        "crates/continuum-engine-reference/tests/semantic_differential.rs",
        "differential_corpus_agrees_with_python_oracle",
    ),
    *(
        ("crates/continuum-asupersync/tests/a7_primitive_conformance.rs", name)
        for name in (
            "every_substrate_journal_is_a_trace_the_conformance_model_accepts",
            "the_models_enabled_steps_include_what_the_substrate_did",
            "a_leaking_run_is_rejected_by_the_model_and_by_the_lift",
            "perturbed_journals_are_rejected_by_the_conformance_model",
            "deletions_and_swaps_get_the_same_verdict_from_the_model_and_the_lift",
        )
    ),
)

# Arrow A7's two sides (bn-ujpz0): the witness drives the real substrate; the model is
# the independent path it is checked against.
A7_WITNESS = "crates/continuum-asupersync/tests/a7_primitive_conformance.rs"
A7_MODEL = "crates/continuum-asupersync/tests/support/primitive_conformance_model.rs"

# RFC 0013 "Cross-path matrix", plus the two arrows the repository already has.
# Each arrow is present only when every crate named in `needs` holds code.
ARROWS: tuple[dict, ...] = (
    {
        "id": "A1",
        "arrow": "reference engine certificate -> independent kernel checker (wire form)",
        "needs": ["continuum-engine-reference", "continuum-kernel-core", CERTIFICATE],
        "witness": "crates/continuum-engine-reference/tests/certificate_kernel_differential.rs",
    },
    {
        "id": "A2",
        "arrow": "Rust reference oracle <-> independent Python oracle (TEST §2)",
        "needs": ["continuum-engine-reference"],
        "witness": "crates/continuum-engine-reference/tests/semantic_differential.rs",
    },
    {
        "id": "A3",
        "arrow": "CIR journal <-> model/refinement checker",
        "needs": ["continuum-cir", REFINEMENT, "continuum-model-core"],
        "witness": None,
    },
    {
        "id": "A4",
        "arrow": "CML reference <-> optimized Rust evaluator",
        "needs": ["continuum-cml-elab", "continuum-engine-explicit"],
        "witness": None,
    },
    {
        "id": "A5",
        "arrow": "Rust evaluator <-> SMT/PDR encodings",
        "needs": ["continuum-engine-symbolic"],
        "witness": None,
    },
    {
        "id": "A6",
        "arrow": "native certificate checker <-> Lean reflective checker",
        "needs": ["continuum-proof-client"],
        "witness": None,
    },
    {
        "id": "A7",
        "arrow": "asupersync adapter <-> executable primitive conformance models",
        "needs": [ADAPTER, "continuum-model-core"],
        "witness": A7_WITNESS,
    },
)


# --- graph -----------------------------------------------------------------------


@dataclass
class Graph:
    members: set[str]
    normal: dict[str, set[str]]  # normal + build edges, every package
    dev: dict[str, set[str]]  # dev edges, workspace members only
    overlay: dict[str, str] = field(default_factory=dict)  # rel path -> appended text
    replace: dict[str, list[tuple[str, str]]] = field(default_factory=dict)  # rel path -> edits

    def copy(self) -> Graph:
        return Graph(
            set(self.members),
            {k: set(v) for k, v in self.normal.items()},
            {k: set(v) for k, v in self.dev.items()},
            dict(self.overlay),
            {k: list(v) for k, v in self.replace.items()},
        )


def cargo_metadata(root: pathlib.Path) -> dict:
    proc = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--offline"],
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        raise SystemExit(f"cargo metadata failed with exit {proc.returncode}")
    return json.loads(proc.stdout)


def build_graph(meta: dict) -> Graph:
    names = {pkg["id"]: pkg["name"] for pkg in meta["packages"]}
    members = {names[pid] for pid in meta["workspace_members"]}
    normal: dict[str, set[str]] = {name: set() for name in names.values()}
    dev: dict[str, set[str]] = {name: set() for name in members}
    for node in meta["resolve"]["nodes"]:
        src = names[node["id"]]
        for dep in node["deps"]:
            dst = names[dep["pkg"]]
            for kind in dep["dep_kinds"]:
                if kind.get("kind") == "dev":
                    dev.setdefault(src, set()).add(dst)
                else:
                    normal[src].add(dst)
    return Graph(members, normal, dev)


def reach(edges: dict[str, set[str]], start: str) -> dict[str, list[str]]:
    """Transitive closure of `start` with a shortest witness path per node."""
    paths: dict[str, list[str]] = {}
    frontier = [[start]]
    while frontier:
        nxt_frontier = []
        for path in frontier:
            for nxt in sorted(edges.get(path[-1], ())):
                if nxt in paths or nxt == start:
                    continue
                paths[nxt] = [*path, nxt]
                nxt_frontier.append(paths[nxt])
        frontier = nxt_frontier
    return paths


def arrow(path: list[str]) -> str:
    return " -> ".join(path)


@dataclass
class Roles:
    checker: list[str]
    kernel: list[str]
    engines: list[str]
    producers: list[str]
    model: list[str]
    program: list[str]


def roles(members: set[str]) -> Roles:
    kernel = sorted(m for m in members if m.startswith("continuum-kernel-"))
    engines = sorted(m for m in members if m.startswith("continuum-engine-"))
    effects = sorted(m for m in members if m.startswith("continuum-effects-"))
    model = sorted({m for m in members if m.startswith("continuum-cml-")} | set(MODEL_NAMED))
    return Roles(
        checker=sorted([*kernel, CERTIFICATE]),
        kernel=kernel,
        engines=engines,
        producers=sorted([*engines, FORGE, ADAPTER]),
        model=model,
        program=sorted([ADAPTER, *effects, RUNTIME]),
    )


# --- sources ---------------------------------------------------------------------


def read(root: pathlib.Path, rel: str, overlay: dict[str, str], replace=None) -> str:
    path = root / rel
    text = path.read_text(encoding="utf-8") if path.exists() else ""
    for old, new in (replace or {}).get(rel, []):
        if old not in text:
            raise SystemExit(f"fixture edit target not found in {rel}: {old!r}")
        text = text.replace(old, new, 1)
    return text + overlay.get(rel, "")


def code_lines(text: str) -> list[str]:
    return [
        line
        for line in text.splitlines()
        if line.strip() and not line.strip().startswith("//")
    ]


def crate_sources(root: pathlib.Path, crate: str, overlay: dict[str, str]) -> dict[str, str]:
    base = root / "crates" / crate / "src"
    rels = sorted(
        str(p.relative_to(root)) for p in base.rglob("*.rs")
    ) if base.exists() else []
    for rel in overlay:
        if rel.startswith(f"crates/{crate}/src/") and rel not in rels:
            rels.append(rel)
    return {rel: read(root, rel, overlay) for rel in sorted(rels)}


def is_scaffold(root: pathlib.Path, crate: str) -> bool:
    return not any(code_lines(text) for text in crate_sources(root, crate, {}).values())


PUB_FN = re.compile(r"^\s*pub fn\s+(\w+)\s*(?:<[^>]*>)?\s*\((?P<params>[^)]*)\)\s*->\s*(?P<ret>[\w:<>, &']+)")


# --- rules -----------------------------------------------------------------------


def evaluate(graph: Graph, root: pathlib.Path, gate) -> list[str]:
    """Every finding as `[rule] detail`."""
    r = roles(graph.members)
    out: list[str] = []
    checker = set(r.checker)
    producers = set(r.producers)
    program = set(r.program)
    model = set(r.model)

    for name in [*MODEL_NAMED, CERTIFICATE, FORGE, ADAPTER, REFINEMENT]:
        if name not in graph.members:
            out.append(f"[roles] named role crate is not a workspace member: {name}")

    for src in r.checker:
        for dst, path in sorted(reach(graph.normal, src).items()):
            if (dst in graph.members and dst not in checker) or dst == RUNTIME:
                out.append(f"[checker-closed] {arrow(path)}")
        for first in sorted(graph.dev.get(src, ())):
            hops = {first: [src, first], **{d: [src, *p] for d, p in reach(graph.normal, first).items()}}
            for dst, path in sorted(hops.items()):
                if dst in producers:
                    out.append(f"[checker-dev-closed] {arrow(path)} (dev)")

    for src in r.producers:
        if src not in graph.members:
            continue
        for dst, path in sorted(reach(graph.normal, src).items()):
            if dst in checker:
                out.append(f"[producer-does-not-link-checker] {arrow(path)}")

    for src in r.model:
        for dst, path in sorted(reach(graph.normal, src).items()):
            if dst in program:
                out.append(f"[model-not-derived-from-program] {arrow(path)}")
    for src in r.program:
        if src not in graph.members:
            continue
        for dst, path in sorted(reach(graph.normal, src).items()):
            if dst in model:
                out.append(f"[program-not-derived-from-model] {arrow(path)}")

    for dst, path in sorted(reach(graph.normal, REFINEMENT).items()):
        if dst in producers or dst in program:
            out.append(f"[refinement-independent] {arrow(path)}")
    for src in sorted(model | program | checker | producers):
        path = reach(graph.normal, src).get(REFINEMENT)
        if path:
            out.append(f"[validator-not-upstream] {arrow(path)}")

    out.extend(cycles(graph))
    out.extend(wire_form(graph, root, r))
    out.extend(tests_live(root, graph.overlay, graph.replace))
    out.extend(a7_model(root, graph.overlay, graph.replace))
    if gate is not None:
        out.extend(gate_coverage(gate, r, graph.members))
    return out


def cycles(graph: Graph) -> list[str]:
    edges = {
        m: {d for d in graph.normal.get(m, set()) | graph.dev.get(m, set()) if d in graph.members}
        for m in graph.members
    }
    state: dict[str, int] = {}
    found: list[str] = []

    def visit(node: str, stack: list[str]) -> None:
        state[node] = 1
        stack.append(node)
        for nxt in sorted(edges[node]):
            if state.get(nxt) == 1:
                found.append(f"[acyclic] {arrow([*stack[stack.index(nxt):], nxt])}")
            elif nxt not in state:
                visit(nxt, stack)
        stack.pop()
        state[node] = 2

    for node in sorted(edges):
        if node not in state:
            visit(node, [])
    return found


def wire_form(graph: Graph, root: pathlib.Path, r: Roles) -> list[str]:
    out: list[str] = []
    for crate in r.checker:
        entries = 0
        for rel, text in crate_sources(root, crate, graph.overlay).items():
            for line in code_lines(text):
                for ident in PRODUCER_IDENTS:
                    if ident in line:
                        out.append(f"[wire-form-only] {rel} names producer `{ident}`: {line.strip()}")
                match = PUB_FN.match(line)
                if not match:
                    continue
                ret = match.group("ret")
                if not any(re.search(rf"\b{t}\b", ret) for t in VERDICT_TYPES):
                    continue
                entries += 1
                if match.group("params").strip() != "bytes: &[u8]":
                    out.append(
                        f"[wire-form-only] {rel}: `pub fn {match.group(1)}({match.group('params').strip()})` "
                        "returns a verdict from something other than wire bytes"
                    )
        if entries == 0:
            out.append(f"[wire-form-only] {crate} has no public verdict entry point to audit")
    return out


def tests_live(root: pathlib.Path, overlay: dict[str, str], replace) -> list[str]:
    out: list[str] = []
    for rel, name in REQUIRED_TESTS:
        lines = read(root, rel, overlay, replace).splitlines()
        index = next((i for i, l in enumerate(lines) if re.match(rf"\s*fn {name}\s*\(", l)), None)
        if index is None:
            out.append(f"[cross-path-tests-live] {rel}::{name} does not exist")
            continue
        attrs: list[str] = []
        j = index - 1
        while j >= 0 and (lines[j].strip().startswith(("#[", "//")) or not lines[j].strip()):
            attrs.append(lines[j].strip())
            j -= 1
        if "#[test]" not in attrs:
            out.append(f"[cross-path-tests-live] {rel}::{name} is not a #[test]")
        if any(a.startswith("#[ignore") for a in attrs):
            out.append(f"[cross-path-tests-live] {rel}::{name} is #[ignore]d")
    return out


A7_USE = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?use\s+(?:::)?(\w+)")
A7_FOREIGN = re.compile(r"\b(continuum_\w+|asupersync)\b")
A7_LOADS = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*;|#\[path\b|\binclude(?:_str|_bytes)?!|\bextern\s+crate\b")


def a7_model(root: pathlib.Path, overlay: dict[str, str], replace) -> list[str]:
    out: list[str] = []
    text = read(root, A7_MODEL, overlay, replace)
    if not code_lines(text):
        return [f"[a7-model-independent] {A7_MODEL} does not exist or holds no code"]
    for line in code_lines(text):
        code = line.split("//", 1)[0]
        use = A7_USE.match(code)
        if use and use.group(1) not in ("std", "core"):
            out.append(f"[a7-model-independent] {A7_MODEL} imports `{use.group(1)}`: {line.strip()}")
        for ident in sorted(set(A7_FOREIGN.findall(code))):
            out.append(f"[a7-model-independent] {A7_MODEL} names `{ident}`: {line.strip()}")
        if A7_LOADS.search(code):
            out.append(f"[a7-model-independent] {A7_MODEL} loads another file: {line.strip()}")
    witness = read(root, A7_WITNESS, overlay, replace)
    loaded = re.findall(r'#\[path\s*=\s*"([^"]+)"\]', witness)
    expected = pathlib.PurePosixPath(A7_MODEL).relative_to(pathlib.PurePosixPath(A7_WITNESS).parent)
    if loaded != [str(expected)]:
        out.append(f"[a7-model-independent] {A7_WITNESS} loads {loaded}, not exactly {expected}")
    return out


def load_gate(path: pathlib.Path = GATE):
    spec = importlib.util.spec_from_file_location("check_crate_boundaries", path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def gate_rules(gate, graph: Graph) -> list[str]:
    enforced = {m: set(graph.normal.get(m, set())) for m in graph.members}
    found = {v[1 : v.index("]")] for v in gate.evaluate(enforced, graph.members)}
    return sorted(found)


def gate_coverage(gate, r: Roles, members: set[str]) -> list[str]:
    out: list[str] = []
    for label, listed, derived in (
        ("KERNEL", gate.KERNEL, r.kernel),
        ("ENGINES", gate.ENGINES, r.engines),
        ("CHECKER", gate.CHECKER, r.checker),
    ):
        if sorted(listed) != derived:
            out.append(f"[gate-coverage] check_crate_boundaries.{label} = {sorted(listed)} but the members give {derived}")
    clean = {m: set() for m in gate.PLAN_20_CRATES}
    for src in r.checker:
        for dst in r.producers:
            if src not in clean or dst not in clean:
                continue
            graph = {k: set(v) for k, v in clean.items()}
            graph[src].add(dst)
            if not gate.evaluate(graph, set(clean)):
                out.append(f"[gate-coverage] the boundary gate accepts {src} -> {dst}")
    return out


def rule_ids(findings: list[str]) -> list[str]:
    return sorted({f[1 : f.index("]")] for f in findings})


# --- mutation campaign -----------------------------------------------------------


def load_fixtures() -> list[dict]:
    return [json.loads(p.read_text(encoding="utf-8")) for p in sorted(FIXTURES.glob("*.json"))]


def apply(graph: Graph, fixture: dict) -> Graph:
    mutated = graph.copy()
    for src, dst, kind in fixture.get("add_edges", []):
        bucket = mutated.dev if kind == "dev" else mutated.normal
        bucket.setdefault(src, set()).add(dst)
    for rel, text in fixture.get("source_overlay", {}).items():
        mutated.overlay[rel] = mutated.overlay.get(rel, "") + text
    for rel, (old, new) in fixture.get("source_replace", {}).items():
        mutated.replace.setdefault(rel, []).append((old, new))
    return mutated


def run_fixtures(graph: Graph, gate) -> tuple[list[dict], list[str]]:
    results: list[dict] = []
    failures: list[str] = []
    control_audit = evaluate(graph, ROOT, gate)
    control_gate = gate_rules(gate, graph)
    if control_audit or control_gate:
        failures.append(f"control: the unmutated workspace is not clean: {control_audit + control_gate}")
    for fixture in load_fixtures():
        mutated = apply(graph, fixture)
        audit = rule_ids(evaluate(mutated, ROOT, gate))
        caught_by_gate = gate_rules(gate, mutated)
        results.append({"id": fixture["id"], "audit": audit, "gate": caught_by_gate})
        if audit != sorted(fixture["expect_audit"]):
            failures.append(f"{fixture['id']}: audit rules {audit} != expected {sorted(fixture['expect_audit'])}")
        if caught_by_gate != sorted(fixture["expect_gate"]):
            failures.append(f"{fixture['id']}: gate rules {caught_by_gate} != expected {sorted(fixture['expect_gate'])}")
        if not audit:
            failures.append(f"{fixture['id']}: the audit accepted a circular mutant")
        if not fixture["expect_gate"] and not fixture.get("gate_note"):
            failures.append(f"{fixture['id']}: the gate does not catch it and no gate_note says why")
    return results, failures


def scratch_copy() -> pathlib.Path:
    base = os.environ.get("TMPDIR") or str(ROOT / "target" / "tmp")
    pathlib.Path(base).mkdir(parents=True, exist_ok=True)
    work = pathlib.Path(tempfile.mkdtemp(prefix="c023-e2e-", dir=base))
    for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml"):
        shutil.copy2(ROOT / name, work / name)
    shutil.copytree(ROOT / "crates", work / "crates", ignore=shutil.ignore_patterns("target"))
    (work / "tools").mkdir()
    shutil.copy2(GATE, work / "tools" / GATE.name)
    shutil.copy2(pathlib.Path(__file__).resolve(), work / "tools" / pathlib.Path(__file__).name)
    shutil.copytree(ROOT / "tools" / "triptych", work / "tools" / "triptych")
    return work


def run_tools(work: pathlib.Path) -> dict:
    env = {**os.environ, "CARGO_TARGET_DIR": str(work / "target")}
    gate = subprocess.run(
        [sys.executable, "tools/check_crate_boundaries.py"], cwd=work, capture_output=True, text=True, env=env
    )
    audit = subprocess.run(
        [sys.executable, "tools/check_triptych_independence.py", "--no-evidence-check"],
        cwd=work,
        capture_output=True,
        text=True,
        env=env,
    )

    def rules_of(proc) -> list[str]:
        try:
            return rule_ids(json.loads(proc.stdout)["violations"])
        except (ValueError, KeyError):
            return [f"unparseable: {proc.stderr.strip()[:200]}"]

    return {
        "gate_exit": gate.returncode,
        "gate_rules": rules_of(gate),
        "audit_exit": audit.returncode,
        "audit_rules": rules_of(audit),
    }


def run_e2e() -> tuple[list[dict], list[str]]:
    fixtures = [f for f in load_fixtures() if "e2e" in f]
    results: list[dict] = []
    failures: list[str] = []
    work = scratch_copy()
    try:
        control = run_tools(work)
        results.append({"id": "control", **control})
        if control["gate_exit"] != 0 or control["audit_exit"] != 0:
            failures.append(f"e2e control: the unedited copy is not clean: {control}")
        for fixture in fixtures:
            manifest = work / fixture["e2e"]["manifest"]
            original = manifest.read_text(encoding="utf-8")
            manifest.write_text(original + fixture["e2e"]["append"], encoding="utf-8")
            lock = (work / "Cargo.lock").read_text(encoding="utf-8")
            # Refresh the copy's lockfile for the new path edge; no network is needed.
            subprocess.run(
                ["cargo", "metadata", "--format-version", "1", "--offline"],
                cwd=work,
                capture_output=True,
                check=False,
            )
            outcome = run_tools(work)
            results.append({"id": fixture["id"], **outcome})
            if outcome["gate_exit"] != 1 or outcome["gate_rules"] != sorted(fixture["expect_gate"]):
                failures.append(f"e2e {fixture['id']}: gate {outcome}")
            if outcome["audit_exit"] != 1 or not set(fixture["e2e"]["expect_audit"]) <= set(outcome["audit_rules"]):
                failures.append(f"e2e {fixture['id']}: audit {outcome}")
            manifest.write_text(original, encoding="utf-8")
            (work / "Cargo.lock").write_text(lock, encoding="utf-8")
    finally:
        shutil.rmtree(work, ignore_errors=True)
    return results, failures


RUST_MUTANT = {
    "id": "c023-rust-kernel-core-in-memory-entry",
    "file": "crates/continuum-kernel-core/src/check.rs",
    "append": (
        "\n/// Mutant (C023): a verdict from a decoded, in-memory certificate.\n"
        "pub fn check_decoded(certificate: &crate::wire::Certificate) -> Verdict {\n"
        "    let _ = certificate;\n"
        "    check_certificate(&[])\n"
        "}\n"
    ),
    "test": ("pr9_exit_evidence", "checking_has_exactly_one_public_entry_point_and_it_takes_wire_form_bytes"),
}


# Arrow A7's program-side mutant (bn-ujpz0): the binding journals a cancelled task's
# effect aborts before the task's acknowledgement, i.e. cleanup before the task entered
# `Cancelling` (docs/02 §7). The A7 model refuses it. When bn-ujpz0 landed the adapter's
# own lift admitted that order; bn-1i050 closed the gap, so both paths now fail on the
# mutant: the A7 witness and the lift-based conformance test of the same family.
A7_ACK = """            if track.acknowledged {
                self.record
                    .append(EventBody::Cancellation(CancellationEvent::Acknowledged {
                        task: *task,
                    }));
            }
"""
A7_ABORTS_END = """                    cause: AbortCause::Cancel,
                }));
            }
"""
A7_RUST_MUTANT = {
    "id": "c023-a7-rust-binding-cleanup-before-acknowledgement",
    "file": "crates/continuum-asupersync/src/binding.rs",
    "edits": [(A7_ACK, ""), (A7_ABORTS_END, A7_ABORTS_END + A7_ACK)],
    "model_test": ("a7_primitive_conformance", "every_substrate_journal_is_a_trace_the_conformance_model_accepts"),
    "lift_test": ("pr14_impl02_reserve_commit_abort", "every_effect_journal_conforms_and_resolves_every_reservation_once"),
}


def run_a7_rust_mutant() -> tuple[dict, list[str]]:
    work = scratch_copy()
    env = {**os.environ, "CARGO_TARGET_DIR": str(work / "target")}

    def test(target: str, name: str):
        cmd = ["cargo", "test", "--locked", "--offline", "-p", ADAPTER, "--test", target, "--", "--exact", name]
        return subprocess.run(cmd, cwd=work, capture_output=True, text=True, env=env)

    try:
        before_model = test(*A7_RUST_MUTANT["model_test"])
        before_lift = test(*A7_RUST_MUTANT["lift_test"])
        path = work / A7_RUST_MUTANT["file"]
        text = path.read_text(encoding="utf-8")
        applied = True
        for old, new in A7_RUST_MUTANT["edits"]:
            if text.count(old) != 1:
                applied = False
                break
            text = text.replace(old, new, 1)
        path.write_text(text, encoding="utf-8")
        after_model = test(*A7_RUST_MUTANT["model_test"])
        after_lift = test(*A7_RUST_MUTANT["lift_test"])
    finally:
        shutil.rmtree(work, ignore_errors=True)

    def passed(proc) -> bool:
        return proc.returncode == 0 and "test result: ok. 1 passed" in proc.stdout

    result = {
        "id": A7_RUST_MUTANT["id"],
        "model_test": f"{ADAPTER} --test {' '.join(A7_RUST_MUTANT['model_test'])}",
        "lift_test": f"{ADAPTER} --test {' '.join(A7_RUST_MUTANT['lift_test'])}",
        "applied": applied,
        "mutated_compiled": "error[E" not in after_model.stderr + after_lift.stderr,
        "unmutated_model_test_passed": passed(before_model),
        "unmutated_lift_test_passed": passed(before_lift),
        "mutated_model_test_failed": after_model.returncode != 0 and "1 failed" in after_model.stdout,
        "mutated_lift_test_failed": after_lift.returncode != 0 and "1 failed" in after_lift.stdout,
    }
    failures = []
    if not all(v for k, v in result.items() if k not in ("id", "model_test", "lift_test")):
        failures.append(f"a7 rust mutant: {result}")
    return result, failures


def run_rust_mutant() -> tuple[dict, list[str]]:
    work = scratch_copy()
    env = {**os.environ, "CARGO_TARGET_DIR": str(work / "target")}
    target, name = RUST_MUTANT["test"]
    cmd = ["cargo", "test", "--locked", "--offline", "-p", "continuum-kernel-core", "--test", target, "--", "--exact", name]
    try:
        before = subprocess.run(cmd, cwd=work, capture_output=True, text=True, env=env)
        path = work / RUST_MUTANT["file"]
        path.write_text(path.read_text(encoding="utf-8") + RUST_MUTANT["append"], encoding="utf-8")
        after = subprocess.run(cmd, cwd=work, capture_output=True, text=True, env=env)
        compiled = "error[E" not in after.stderr
    finally:
        shutil.rmtree(work, ignore_errors=True)
    result = {
        "id": RUST_MUTANT["id"],
        "test": f"continuum-kernel-core --test {target} {name}",
        "unmutated_exit": before.returncode,
        "mutated_exit": after.returncode,
        "mutated_compiled": compiled,
        "unmutated_ran_one_passing_test": "test result: ok. 1 passed" in before.stdout,
        "mutated_ran_one_failing_test": "1 failed" in after.stdout,
    }
    failures = []
    if (
        before.returncode != 0
        or after.returncode == 0
        or not compiled
        or not result["unmutated_ran_one_passing_test"]
        or not result["mutated_ran_one_failing_test"]
    ):
        failures.append(f"rust mutant: {result}")
    return result, failures


# --- evidence ----------------------------------------------------------------------


def stable_record(graph: Graph) -> dict:
    r = roles(graph.members)
    arrows = []
    for a in ARROWS:
        scaffolds = sorted(c for c in a["needs"] if is_scaffold(ROOT, c))
        arrows.append(
            {
                "id": a["id"],
                "arrow": a["arrow"],
                "status": "present" if not scaffolds else "scaffold",
                "scaffold_crates": scaffolds,
                "witness": a["witness"],
            }
        )
    return {
        "claim": "C023",
        "bone": "bn-2270",
        "tool": "tools/check_triptych_independence.py",
        "graph_source": "cargo metadata --format-version 1 --locked --offline (resolve, dev edges included)",
        "rules": RULES,
        "roles": {
            "checker": r.checker,
            "producers": r.producers,
            "model": r.model,
            "program": r.program,
            "refinement": [REFINEMENT],
        },
        "arrows": arrows,
        "required_tests": [f"{rel}::{name}" for rel, name in REQUIRED_TESTS],
        "mutants": [
            {
                "id": f["id"],
                "expect_audit": sorted(f["expect_audit"]),
                "expect_gate": sorted(f["expect_gate"]),
                "gate_note": f.get("gate_note"),
                "e2e": "e2e" in f,
            }
            for f in load_fixtures()
        ],
    }


def main() -> int:
    args = set(sys.argv[1:])
    gate = load_gate()
    graph = build_graph(cargo_metadata(ROOT))

    if "--self-test" in args or "--evidence" in args:
        results, failures = run_fixtures(graph, gate)
        e2e, e2e_failures = run_e2e()
        failures += e2e_failures
        report: dict = {"self_test": "fail" if failures else "pass", "mutants": results, "e2e": e2e, "failures": failures}
        if "--evidence" in args:
            rust, rust_failures = run_rust_mutant()
            failures += rust_failures
            report["rust_mutant"] = rust
            a7_rust, a7_failures = run_a7_rust_mutant()
            failures += a7_failures
            report["a7_rust_mutant"] = a7_rust
            report["self_test"] = "fail" if failures else "pass"
            if not failures:
                record = {
                    **stable_record(graph),
                    "campaign": {"mutants": results, "e2e": e2e, "rust_mutant": rust, "a7_rust_mutant": a7_rust},
                }
                EVIDENCE.parent.mkdir(parents=True, exist_ok=True)
                EVIDENCE.write_text(json.dumps(record, indent=2, sort_keys=True, ensure_ascii=False) + "\n", encoding="utf-8")
                report["evidence"] = str(EVIDENCE.relative_to(ROOT))
        print(json.dumps(report, indent=2, sort_keys=True))
        return 1 if failures else 0

    violations = evaluate(graph, ROOT, gate)
    if "--no-evidence-check" not in args:
        record = stable_record(graph)
        try:
            committed = json.loads(EVIDENCE.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            committed = {}
        stale = sorted(k for k in record if committed.get(k) != record[k])
        if stale:
            violations.append(
                f"[evidence-stale] {EVIDENCE.relative_to(ROOT)} differs in {stale}; re-run with "
                "--evidence and re-scope C023's evidence record in docs/18 if an arrow changed"
            )
    internal = sum(len(v & graph.members) for k, v in graph.normal.items() if k in graph.members)
    report = {
        "members": len(graph.members),
        "packages": len(graph.normal),
        "workspace_internal_normal_edges": internal,
        "workspace_dev_edges": sum(len(v & graph.members) for v in graph.dev.values()),
        "rules": sorted(RULES),
        "violations": violations,
        "status": "fail" if violations else "pass",
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if violations else 0


if __name__ == "__main__":
    raise SystemExit(main())
