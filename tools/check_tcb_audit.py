#!/usr/bin/env python3
"""C018 audit of the trusted checking base, and its mutation lane (bn-2npu).

docs/18 C018: "Proof-carrying results materially shrink the TCB", required
evidence "independent checker audit and mutation". This tool is the audit half,
and the driver of the source-mutation half. The verdicts of the mutation half
come from real Rust tests over the real kernel and producer code:

- `crates/continuum-certificate/tests/c018_checker_mutation.rs` — corrupted
  certificates of every checked class, judged by an independent oracle;
- `crates/continuum-engine-reference/tests/c018_producer_corpus.rs` — the
  reference engine's certificates for a model corpus, checked by the kernel.

This file ports no checker rule. It reads manifests and sources, applies text
edits to a scratch copy, runs `cargo test`, and records which tests failed.

Modes:

    python3 tools/check_tcb_audit.py --self-test   # every rule catches its fixture
    python3 tools/check_tcb_audit.py               # the audit, and evidence freshness
    python3 tools/check_tcb_audit.py --evidence    # run the mutation lane, write evidence

Rules (the gate):

  TCB-01 checking-base-links-only-itself
         Over the resolved `cargo metadata` graph, by Cargo package *id*, normal,
         build and dev edges alike: the allowed set is the workspace-member ids of
         `continuum-certificate` and the four `continuum-kernel-*`. Every package
         reached from them must be in it. A checking-base name that also names a
         second package (an external crate, another version), a checking-base name
         whose package is not a member, and a renamed dependency of a checking-base
         crate each fail, with a witness path of ids (cr-10mg1y).
  TCB-02 verdict-from-wire-bytes
         A conservative second line behind the Rust tests
         `checking_has_exactly_one_public_entry_point_and_it_takes_wire_form_bytes`
         in each checking crate, which stay the authority. It is NOT complete over
         the public surface, and it makes no such claim (Codex cr-3i3rst round 7).
         It checks exactly these forms: `fn`, method, trait-method and trait-impl
         method signatures; type aliases and renames; public statics, consts
         (inherent associated consts included), aliases, struct, tuple and enum
         variant fields; `#[path]`/`cfg_attr`/`include!`/macro constructs; and,
         through rustc's dep-info, which files were compiled. It does NOT check
         trait-associated consts or types declared in traits, and other forms it
         does not name. One such route is pinned as a known miss
         (`tcb02-trait-associated-const`). A complete check needs the public API
         surface derived from rustc itself. Every public item of the
         checking base's shipped code (pub fns, pub methods of inherent impls, trait
         impls, pub type aliases, pub `use` re-exports, pub traits and their
         methods, pub types, consts, statics and modules) must appear in the
         approved inventory `tools/tcb-audit/public-api.txt`, and every inventory
         line must name a live item. Alias handling fails closed by grammar: a
         `type` item anywhere in the shipped code is accepted only as `type Name;`
         or `type Name = Path;` (optionally `Path<Path, ...>` naming no verdict
         type), with no generic parameters, defaults, const generics, bounds or
         where clause. Any other `type` item, a `use ... as ...` rename of a
         verdict type, and a generic equality binding a verdict type fail as
         "alias the scan cannot resolve", whatever the inventory says. The
         accepted aliases and renames close the verdict types to a fixed point,
         private ones and associated types included, in any order and over any
         number of hops. A function, method, trait method or trait-impl method
         that returns a type in that closure must be a free function taking exactly
         `bytes: &[u8]`, or a `&self` method of a verdict type; anything else
         "yields a verdict from non-wire input". An associated type of a
         non-verdict type bound to a verdict fails the same way, and a return type
         projecting an associated type no item binds fails closed. The closure is
         reported next to every entry point in the evidence. A macro definition
         or item-level macro invocation, and any item the scan cannot classify, fail
         closed: no inventory line approves one. The shipped code is the KCOV
         counting method's (`check_kernel_covenant.py`).
  TCB-03 campaign-tests-live
         Every Rust test the C018 evidence names exists as a `#[test]` and is
         not `#[ignore]`d.
  TCB-04 mutant-inventory
         `tools/tcb-audit/mutants.toml` parses; ids are unique; each mutant's
         `side` matches the crate of its `file`; `find` occurs exactly once in
         the live file; `expect` is in the side's vocabulary; `survives` and
         `missed` carry a reason.
  TCB-05 evidence-current
         The whole retained record `tools/tcb-audit/evidence/c018.json` is
         validated, not only its digest (cr-10mg1y round 4). Every tree-derived
         field equals a recomputation: the digest over every file under each
         checking crate's directory (any extension) plus the workspace
         `Cargo.toml`, `Cargo.lock` and `rust-toolchain.toml`, the surface, the
         checking-base measurements. The lane's control passed and every TCB-02
         fixture compiled. The rows name each inventory mutant once, with its side,
         file, target and expected outcome, and the summary is recomputed from them.
         `--evidence` writes the record atomically and only when all of that holds,
         so a failed campaign leaves the previous record byte-identical.

TCB-02 is conservative and partial. It refuses whole syntactic classes rather than
proving an instance safe, and it covers only the classes named here and above: a public static, const (associated too), alias, struct or tuple field,
or enum variant field whose type names a callable (`Fn`, `FnMut`, `FnOnce`, `fn(`,
`dyn`, `impl `) or a verdict-closure type is "a public value the scan cannot prove
wire-only" (a verdict type's own variants may hold verdict types, never callables);
a public trait method returning a verdict fails; and the one approved
`check_certificate(bytes: &[u8])` per crate is the only verdict producer. The Rust
authority tests in each crate apply the same rule and remain the authority.

Compiled inputs (TCB-02, TCB-05; Codex cr-3i3rst): the audit runs `cargo check
--locked` for the five checking crates under the pinned toolchain and reads rustc's
dep-info (`.d`) for each. The files rustc read must EQUAL the files the scan
inventories: a file rustc read that the scan did not ("a file rustc compiled that
the scan did not see"), or a scanned file rustc did not read, fails. A failed build,
a missing dep-info file, or one older than a source it names fails closed. Every
file rustc read is also in the TCB-05 digest. The lexical rules below stay as
defence in depth, and `cfg_attr` whose attribute list names `path` is banned in all
five crates.

Signatures (TCB-02): every `fn` token of the shipped code, items, nested functions
and function-pointer types alike, must parse under the signature grammar (raw
identifiers and qualified paths included); one that does not is "a signature the
scan cannot parse" and fails. A `#[path]`, an `include!`-family macro or a `mod`
with no file under `src/`, in any of the five crates, is "a module the scan cannot
see" and fails (KCOV-07's rule, extended to `continuum-certificate`).

The producer side is gated for freshness too: an edit to `continuum-engine-reference`
or `continuum-model-core` sources fails TCB-05 until the lane runs again, because the
record's producer measurements and residual would no longer be a recomputation.

Stdlib only, plus `cargo` for the graph and for the lane. Exit 0 when every
rule holds, 1 otherwise.
"""

from __future__ import annotations

import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))

import check_kernel_covenant as kcov  # noqa: E402  (the counting method's one authority)
import check_triptych_independence as trip  # noqa: E402  (the graph reader's one authority)

AUDIT_DIR = "tools/tcb-audit"
MUTANTS = f"{AUDIT_DIR}/mutants.toml"
EVIDENCE = f"{AUDIT_DIR}/evidence/c018.json"

CERTIFICATE = "continuum-certificate"
KERNELS = (
    "continuum-kernel-core",
    "continuum-kernel-sat",
    "continuum-kernel-smt",
    "continuum-kernel-temporal",
)
CHECKING_BASE = (CERTIFICATE, *KERNELS)
REFERENCE = "continuum-engine-reference"
MODEL_CORE = "continuum-model-core"
PRODUCER_CRATES = (REFERENCE, MODEL_CORE)
SCAFFOLD_ENGINES = (
    "continuum-engine-explicit",
    "continuum-engine-dpor",
    "continuum-engine-symbolic",
    "continuum-engine-liveness",
)

CHECKER_TEST = "crates/continuum-certificate/tests/c018_checker_mutation.rs"
CHECKER_GOLDEN = "crates/continuum-certificate/tests/golden/c018_checker_ledger.txt"
CHECKER_CORPUS = "crates/continuum-certificate/tests/c018-corpus/cases.txt"
PRODUCER_TEST = "crates/continuum-engine-reference/tests/c018_producer_corpus.rs"

# Test name -> the kill class a failure of it proves.
KILL_CLASS = {
    "no_lie_verifies": "soundness",
    "no_single_byte_mutant_verifies_a_claim_the_oracle_refutes": "soundness",
    "an_accepted_lemma_names_its_trusted_solver_assurance": "assurance",
    "every_green_certificate_verifies_and_the_oracle_agrees": "completeness",
    "the_trusted_lemma_is_accepted_because_lemmas_are_not_checked": "completeness",
    "every_lie_is_a_false_claim_so_the_corpus_is_not_vacuous": "completeness",
    "every_lie_is_rejected_for_the_reason_it_targets": "precision",
    "the_c018_checker_ledger_matches_the_golden": "drift",
    "the_committed_corpus_replays_with_its_recorded_verdicts": "drift",
}
KILL_ORDER = ("soundness", "assurance", "completeness", "precision", "drift", "timeout")

REQUIRED_TESTS = (
    *((CHECKER_TEST, name) for name in KILL_CLASS),
    (PRODUCER_TEST, "the_producer_corpus_verifies_from_bytes_with_pinned_certificates"),
    (PRODUCER_TEST, "the_probe_is_deterministic"),
    (
        "crates/continuum-kernel-temporal/src/check.rs",
        "a_reachable_non_goal_deadlock_refutes_fair_cycle_exclusion",
    ),
    (
        "crates/continuum-kernel-temporal/src/check.rs",
        "an_unreachable_or_goal_dead_end_does_not_refute_fair_cycle_exclusion",
    ),
    (
        "crates/continuum-kernel-core/tests/pr9_exit_evidence.rs",
        "checking_has_exactly_one_public_entry_point_and_it_takes_wire_form_bytes",
    ),
    (
        "crates/continuum-certificate/tests/inv004_no_self_certification.rs",
        "checking_has_exactly_one_public_entry_point_and_it_takes_wire_form_bytes",
    ),
)

EXPECT = {
    "checker": ("killed", "survives"),
    "producer": ("caught", "missed", "refused", "no-effect"),
}
NEEDS_REASON = ("survives", "missed")

SIDE_CRATES = {"checker": CHECKING_BASE, "producer": PRODUCER_CRATES}


# --- inputs ----------------------------------------------------------------------


def crate_of(rel: str) -> str | None:
    parts = rel.split("/")
    return parts[1] if len(parts) > 2 and parts[0] == "crates" else None


def load_mutants(tree: kcov.Tree) -> tuple[list[dict], list[str]]:
    text = tree.read_text(MUTANTS)
    if text is None:
        return [], [f"[mutant-inventory] {MUTANTS} is missing"]
    try:
        doc = tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:
        return [], [f"[mutant-inventory] {MUTANTS} does not parse: {error}"]
    mutants = doc.get("mutant", [])
    if not isinstance(mutants, list) or not mutants:
        return [], [f"[mutant-inventory] {MUTANTS} declares no [[mutant]]"]
    return mutants, []


BUILD_INPUTS = ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml")


def _bytes(tree: kcov.Tree, rel: str) -> bytes:
    if rel in tree.overlay:
        return tree.overlay[rel].encode()
    if rel in tree.removed:
        return b""
    path = tree.root / rel
    return path.read_bytes() if path.is_file() else b""


def source_digest(
    tree: kcov.Tree, mutants: list[dict], inputs: dict[str, set[str]] | None = None
) -> tuple[str, list[str]]:
    """SHA-256 over every input whose change invalidates the checker campaign.

    Every file under each checking crate's directory, whatever its extension (a
    `#[path]` module or an `include!`d file is a compiler input too; cr-10mg1y round
    4), the workspace build inputs, and the campaign's own tool, inventory, mutants
    and fixtures.
    """
    del mutants
    files = sorted(
        {
            *(rel for crate in CHECKING_BASE for rel in tree.glob(f"crates/{crate}/**/*")),
            # Every file rustc read for the checking base, wherever it lives (cr-3i3rst).
            *(rel for files in (inputs or {}).values() for rel in files),
            *BUILD_INPUTS,
            MUTANTS,
            INVENTORY,
            "tools/check_tcb_audit.py",
            *tree.glob(f"{AUDIT_DIR}/fixtures/*.json"),
        }
    )
    h = hashlib.sha256()
    for rel in files:
        h.update(rel.encode() + b"\0" + _bytes(tree, rel) + b"\0")
    return h.hexdigest(), files


def producer_digest(tree: kcov.Tree) -> str:
    h = hashlib.sha256()
    for crate in PRODUCER_CRATES:
        for rel in tree.glob(f"crates/{crate}/src/**/*.rs"):
            h.update(rel.encode() + b"\0" + (tree.read_text(rel) or "").encode() + b"\0")
    h.update((tree.read_text(PRODUCER_TEST) or "").encode())
    return h.hexdigest()


# --- rules -----------------------------------------------------------------------


# --- TCB-01: the identity graph ----------------------------------------------------
#
# Cargo package *names* do not identify packages: an external crate, a second version,
# or a renamed dependency can carry a checking-base name. This rule therefore walks the
# resolve graph by package *id* (the `name version (source)` identity cargo prints) and
# allows exactly the workspace-member ids of the five checking-base crates.


def id_graph(meta: dict) -> tuple[dict[str, dict], dict[str, list[tuple[str, str]]], set[str]]:
    """(packages by id, edges by id as (dst id, extern name), workspace member ids)."""
    packages = {pkg["id"]: pkg for pkg in meta.get("packages", [])}
    edges: dict[str, list[tuple[str, str]]] = {pid: [] for pid in packages}
    for node in (meta.get("resolve") or {}).get("nodes", []):
        for dep in node.get("deps", []):
            edges.setdefault(node["id"], []).append((dep["pkg"], dep.get("name", "")))
    return packages, edges, set(meta.get("workspace_members", []))


def label(packages: dict[str, dict], members: set[str], pid: str) -> str:
    pkg = packages.get(pid, {})
    where = "workspace member" if pid in members else f"not a workspace member, source {pkg.get('source') or 'path'}"
    return f"{pkg.get('name', '?')} {pkg.get('version', '?')} ({where}; id {pid})"


def rule_links(meta: dict) -> list[str]:
    tag = "[checking-base-links-only-itself]"
    out: list[str] = []
    packages, edges, members = id_graph(meta)
    by_name: dict[str, list[str]] = {}
    for pid, pkg in packages.items():
        by_name.setdefault(pkg["name"], []).append(pid)
    allowed: set[str] = set()
    for name in CHECKING_BASE:
        ids = sorted(by_name.get(name, []))
        member_ids = [pid for pid in ids if pid in members]
        if not member_ids:
            out.append(f"{tag} {name} is not a workspace member")
        if len(ids) > 1:
            out.append(f"{tag} the name {name} names {len(ids)} packages: " + "; ".join(label(packages, members, i) for i in ids))
        for pid in ids:
            if pid not in members:
                out.append(f"{tag} {label(packages, members, pid)} carries a checking-base name but is not a workspace member")
        allowed.update(member_ids)
    for start in sorted(allowed):
        paths = {start: [start]}
        frontier = [start]
        while frontier:
            here = frontier.pop(0)
            for dst, extern in sorted(edges.get(here, [])):
                expected = packages.get(dst, {}).get("name", "").replace("-", "_")
                if here in allowed and extern and extern != expected:
                    out.append(
                        f"{tag} {label(packages, members, here)} renames its dependency {label(packages, members, dst)} "
                        f"to `{extern}`; a checking-base crate declares every dependency under its own name"
                    )
                if dst in paths:
                    continue
                paths[dst] = [*paths[here], dst]
                if dst in allowed:
                    frontier.append(dst)
                else:
                    witness = " -> ".join(label(packages, members, p) for p in paths[dst])
                    out.append(f"{tag} {witness}")
    return sorted(set(out))


# --- TCB-02: the public API inventory ------------------------------------------------
#
# A second line behind the Rust tests `checking_has_exactly_one_public_entry_point_...`
# in each checking crate, which stay the authority. This scan is lexical and covers
# only the forms the module docstring names; it is not complete over the public
# surface. Within those forms, every public item it records must appear in the
# committed, approved inventory `tools/tcb-audit/public-api.txt`, and a construct it
# cannot classify is a failure that no inventory line can approve.

INVENTORY = f"{AUDIT_DIR}/public-api.txt"
VERDICT_TYPES = frozenset({"Verdict", "Outcome", "KernelVerdict", "CheckedClaim"})
VALUE_CONSTRUCTORS = ("From", "TryFrom", "FromStr", "Default", "FromIterator")
UNCLASSIFIED = "UNCLASSIFIED"


def _match(code: str, i: int, open_ch: str, close_ch: str) -> int:
    """The index just past the delimiter that closes the one at `i`, or len(code)."""
    depth = 0
    for j in range(i, len(code)):
        if code[j] == open_ch:
            depth += 1
        elif code[j] == close_ch:
            depth -= 1
            if depth == 0:
                return j + 1
    return len(code)


def _header(code: str, i: int, end: int) -> tuple[str, int, str]:
    """Read an item header from `i` to the first `{` or `;` outside parens/brackets."""
    depth = 0
    j = i
    while j < end:
        ch = code[j]
        if ch in "([":
            depth += 1
        elif ch in ")]":
            depth -= 1
        elif depth == 0 and ch in "{;":
            return " ".join(code[i:j].split()), j, ch
        j += 1
    return " ".join(code[i:end].split()), end, ""


PUB = re.compile(r"^pub\s")
# Not anchored with `^`: `Pattern.match(code, pos)` already anchors at `pos`, and `^`
# would only ever match at the start of the file.
MACRO = re.compile(r"(?:pub(?:\([^)]*\))?\s+)?(macro_rules\s*!\s*(\w+)|(\w+)\s*!)")


def items(code: str, i: int = 0, end: int | None = None, ctx: tuple[str, str] = ("mod", "")) -> list[tuple[str, str]]:
    """Every item the public-API rule counts, as (kind, key). Recursive over blocks."""
    end = len(code) if end is None else end
    out: list[tuple[str, str]] = []
    while i < end:
        while i < end and (code[i].isspace() or code[i] == ";"):
            i += 1
        if i >= end:
            break
        if code[i] == "#":
            k = code.find("[", i)
            i = _match(code, k, "[", "]") if k >= 0 else end
            continue
        macro = MACRO.match(code, i)
        if macro:
            bang = code.index("!", i) + 1
            opener = min((p for p in (code.find(c, bang) for c in "{([") if p >= 0), default=end)
            close = {"{": "}", "(": ")", "[": "]"}.get(code[opener] if opener < end else "", "")
            stop = _match(code, opener, code[opener], close) if close else end
            key = f"macro_rules! {macro.group(2)}" if macro.group(2) else f"macro-invocation {macro.group(3)}!"
            out.append(("macro", key))
            i = stop
            continue
        head, j, stop = _header(code, i, end)
        body_end = _match(code, j, "{", "}") if stop == "{" else j + 1
        if stop == "{" and re.match(r"^(pub(\([^)]*\))?\s+)?use\b", head):
            # A braced `use` tree: the item runs to its `;`, braces included.
            semi = code.find(";", body_end)
            body_end = end if semi < 0 else semi + 1
            head = " ".join(code[i : body_end - 1].split())
        kind_ctx, owner = ctx
        public = bool(PUB.match(head))
        bare = re.sub(r"^pub(?:\([^)]*\))?\s+", "", head)
        if re.search(r"(^|\s)fn\s", " " + bare):
            if kind_ctx == "mod" and public:
                out.append(("fn", f"fn {bare.split('fn ', 1)[1]}"))
            elif kind_ctx == "impl" and public:
                out.append(("method", f"impl {owner}: fn {bare.split('fn ', 1)[1]}"))
            elif kind_ctx == "trait" and owner:
                out.append(("trait-method", f"trait {owner}: fn {bare.split('fn ', 1)[1]}"))
            elif kind_ctx == "trait-impl":
                out.append(("impl-method", f"{owner}: fn {bare.split('fn ', 1)[1]}"))
        elif re.match(r"^(unsafe\s+)?impl\b", bare):
            if " for " in f" {bare} ":
                out.append(("trait-impl", bare))
                out.extend(items(code, j + 1, body_end - 1, ("trait-impl", bare)) if stop == "{" else [])
            else:
                target = re.sub(r"^(unsafe\s+)?impl(<[^{]*?>)?\s+", "", bare)
                out.extend(items(code, j + 1, body_end - 1, ("impl", target)) if stop == "{" else [])
        elif re.match(r"^(unsafe\s+)?(auto\s+)?trait\s", bare):
            name = re.match(r"^(?:unsafe\s+)?(?:auto\s+)?trait\s+(\w+)", bare).group(1)
            if public:
                out.append(("trait", f"trait {name}"))
            out.extend(items(code, j + 1, body_end - 1, ("trait", name if public else "")) if stop == "{" else [])
        elif re.match(r"^mod\s", bare):
            if public:
                out.append(("mod", f"mod {bare.split()[1]}"))
            if stop == "{":
                out.extend(items(code, j + 1, body_end - 1, ("mod", "")))
        elif re.match(r"^(struct|enum|union|type|use|const|static|extern\s+crate)\b", bare):
            if public and kind_ctx in ("mod", "impl"):
                word = bare.split()[0]
                if word in ("type", "use"):
                    out.append((word, bare))
                else:
                    name = re.match(r"^(?:extern\s+crate|\w+)\s+(?:mut\s+)?(\w+)", bare)
                    out.append((word, f"{word} {name.group(1) if name else bare}" if kind_ctx == "mod" else f"impl {owner}: {word} {name.group(1) if name else bare}"))
            elif kind_ctx == "trait" and owner and bare.split()[0] in ("type", "const"):
                out.append(("trait-item", f"trait {owner}: {bare}"))
            elif kind_ctx == "trait-impl" and bare.startswith("type"):
                out.append(("assoc", f"{owner}: {bare}"))
        elif kind_ctx == "trait-impl":
            pass
        else:
            out.append((UNCLASSIFIED, head or code[i : i + 40].strip()))
        i = body_end
    return out


def public_api(tree: kcov.Tree) -> dict[str, list[tuple[str, str, str]]]:
    """crate -> [(file, kind, key)] over the KCOV method's shipped code."""
    api: dict[str, list[tuple[str, str, str]]] = {}
    for crate in CHECKING_BASE:
        rows: list[tuple[str, str, str]] = []
        for rel, code in shipped_sources(tree, crate).items():
            rows.extend((rel, kind, key) for kind, key in items(code))
        api[crate] = rows
    return api


def inventory_lines(api: dict[str, list[tuple[str, str, str]]]) -> list[str]:
    return sorted(
        {
            f"{rel}: {key}"
            for rows in api.values()
            for rel, kind, key in rows
            if kind not in (UNCLASSIFIED, "macro", "assoc", "impl-method")
        }
    )


def read_inventory(tree: kcov.Tree) -> set[str] | None:
    text = tree.read_text(INVENTORY)
    if text is None:
        return None
    return {line for line in (l.strip() for l in text.splitlines()) if line and not line.startswith("#")}


def shipped_sources(tree: kcov.Tree, crate: str) -> dict[str, str]:
    """The KCOV method's shipped code: test-only modules and `#[cfg(test)]` items gone."""
    rels = tree.glob(f"crates/{crate}/src/**/*.rs")
    scans = {rel: kcov.scan_rust(tree.read_text(rel) or "") for rel in rels}
    test_only = kcov.test_only_files(scans)
    return {rel: scans[rel].shipped_code() for rel in sorted(scans) if rel not in test_only}


UNPARSED = "signature the scan cannot parse"
HIDDEN = "a module the scan cannot see"
IDENT = re.compile(r"(?:r#)?[A-Za-z_][A-Za-z0-9_]*")
TYPE_TEXT = re.compile(r"^[A-Za-z0-9_#:&'<>,()\[\]; *!+=-]+$")


def _balanced(text: str, i: int, open_ch: str, close_ch: str) -> int | None:
    """Index just past the delimiter closing the one at `i`, skipping `->` inside
    angle brackets; None if it never closes."""
    depth = 0
    j = i
    while j < len(text):
        ch = text[j]
        if open_ch == "<" and text.startswith("->", j):
            j += 2
            continue
        if ch == open_ch:
            depth += 1
        elif ch == close_ch:
            depth -= 1
            if depth == 0:
                return j + 1
        j += 1
    return None


def parse_fn(text: str) -> tuple[str, str, str] | None:
    """`fn NAME [<generics>] ( params ) [-> ret] [where ...]` -> (name, params, ret).

    Raw identifiers and qualified paths are part of the grammar. Anything else is
    `None`, and a caller must fail on it (cr-10mg1y round 4): a signature the scan
    cannot parse is never skipped.
    """
    text = " ".join(text.split())
    m = re.match(r"fn\s+", text)
    if not m:
        return None
    name = IDENT.match(text, m.end())
    if not name:
        return None
    i = name.end()
    while i < len(text) and text[i] == " ":
        i += 1
    if i < len(text) and text[i] == "<":
        close = _balanced(text, i, "<", ">")
        if close is None:
            return None
        i = close
    while i < len(text) and text[i] == " ":
        i += 1
    if i >= len(text) or text[i] != "(":
        return None
    close = _balanced(text, i, "(", ")")
    if close is None:
        return None
    params = text[i + 1 : close - 1].strip().rstrip(",").strip()
    rest = text[close:].strip()
    ret = ""
    if rest.startswith("->"):
        ret = re.split(r"\bwhere\b", rest[2:], maxsplit=1)[0].strip()
        if not ret or not TYPE_TEXT.match(ret):
            return None
    elif rest and not rest.startswith("where"):
        return None
    return name.group(0), params, ret


FN_TOKEN = re.compile(r"\bfn\b")


def fn_tokens(tree: kcov.Tree) -> list[tuple[str, str, str, str]]:
    """Every `fn` token of the shipped code, parsed: (file, text, params, ret).

    Items, nested functions and function-pointer types alike. An unparsable one comes
    back with `ret` set to the UNPARSED marker.
    """
    out: list[tuple[str, str, str, str]] = []
    for crate in CHECKING_BASE:
        for rel, code in shipped_sources(tree, crate).items():
            flat = " ".join(ATTRIBUTE.sub(" ", code).split())
            for m in FN_TOKEN.finditer(flat):
                j = m.end()
                while j < len(flat) and flat[j] == " ":
                    j += 1
                if j < len(flat) and flat[j] == "(":
                    # A function-pointer type: `fn(params) [-> ret]`.
                    close = _balanced(flat, j, "(", ")")
                    if close is None:
                        out.append((rel, flat[m.start() : m.start() + 60], "", UNPARSED))
                        continue
                    tail = flat[close:].lstrip()
                    ret = ""
                    if tail.startswith("->"):
                        ret = re.split(r"[,;{})=]|\bwhere\b", tail[2:], maxsplit=1)[0].strip()
                    out.append((rel, flat[m.start() : close], flat[j + 1 : close - 1].strip(), ret))
                    continue
                depth, stop = 0, len(flat)
                for k in range(m.end(), len(flat)):
                    if flat[k] in "([":
                        depth += 1
                    elif flat[k] in ")]":
                        depth -= 1
                    elif depth == 0 and flat[k] in "{;":
                        stop = k
                        break
                text = flat[m.start() : stop]
                parsed = parse_fn(text)
                if parsed is None:
                    out.append((rel, text, "", UNPARSED))
                else:
                    out.append((rel, text, parsed[1], parsed[2]))
    return out


YIELDS = "yields a verdict from non-wire input"
SELF = re.compile(r"^&?\s*(?:'\w+\s+)?(?:mut\s+)?self\b")
USE = re.compile(r"\buse\s+([^;]+);")
RENAME = re.compile(r"(\w+)\s+as\s+(\w+)")
PROJECTION = re.compile(r"(?:\bSelf|>)\s*::\s*(\w+)")


def projections(text: str) -> list[str]:
    return PROJECTION.findall(text)


def impl_target(header: str) -> str:
    """The implementing type's last path segment, without generics."""
    rest = header.rsplit(" for ", 1)[-1] if " for " in header else re.sub(r"^(unsafe\s+)?impl(<.*?>)?\s+", "", header)
    return rest.split("<")[0].strip().split("::")[-1]


UNRESOLVABLE = "alias the scan cannot resolve"
TYPE_ITEM = re.compile(r"(?:^|(?<=[\s{};]))(?:pub(?:\([^)]*\))?\s+)?type\s")
PLAIN_ALIAS = re.compile(r"^(?:pub(?:\([^)]*\))?\s+)?type\s+(\w+)\s*=\s*(.+)$")
PLAIN_DECL = re.compile(r"^(?:pub(?:\([^)]*\))?\s+)?type\s+(\w+)$")
PATH = r"(?:(?:crate|self|super|Self|\w+)::)*\w+"
PLAIN_RHS = re.compile(rf"^(?P<head>{PATH})(?:<(?P<args>{PATH}(?:\s*,\s*{PATH})*)>)?$")
ATTRIBUTE = re.compile(r"#!?\[[^\]]*\]")
EQUALITY = re.compile(r"[<,]\s*(\w+)\s*=\s*([^,<>;{}]+(?:<[^<>]*>)?)")


def _statement(flat: str, start: int) -> str:
    """From `start` to the `;` that ends the item, outside brackets and parens."""
    depth = 0
    for j in range(start, len(flat)):
        ch = flat[j]
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
            if depth < 0:
                return flat[start:j].strip()
        elif ch == ";" and depth == 0:
            return flat[start:j].strip()
    return flat[start:].strip()


def verdict_closure(tree: kcov.Tree) -> tuple[set[str], dict[str, str], set[str], list[str]]:
    """Every type name that is, or aliases, a verdict type, to a fixed point, and every
    alias construct the scan refuses.

    Alias handling fails closed by grammar (cr-10mg1y round 3, lead ruling). In the
    shipped code of the checking base, a `type` item at any depth (module, inherent
    impl, trait impl, trait) is accepted only as `type Name;` (an associated type
    declaration) or `type Name = Path;` / `type Name = Path<Path, ...>;`: no generic
    parameters, no defaults, no const generics, no bounds, no where clause, a plain
    path on the right, and generic arguments that are plain paths naming nothing in
    the closure. Every other `type` item is refused. So is a `use ... as ...` rename
    of a name in the closure, and a generic equality (`<T = X>`, `Trait<Out = X>`)
    whose right side names one. The accepted aliases and renames then join the
    closure until nothing new joins, so order and hop count do not matter.

    Returns (closure, bindings, unbound associated type names, refusals).
    """
    aliases: list[tuple[str, str, str]] = []  # (name, rhs, where)
    renames: list[tuple[str, str, str]] = []
    equalities: list[tuple[str, str, str]] = []
    declared: set[str] = set()
    refusals: list[str] = []
    for crate in CHECKING_BASE:
        for rel, code in shipped_sources(tree, crate).items():
            flat = ATTRIBUTE.sub(" ", " ".join(code.split()))
            for m in TYPE_ITEM.finditer(flat):
                item = _statement(flat, m.start())
                if (decl := PLAIN_DECL.match(item)) is not None:
                    declared.add(decl.group(1))
                    continue
                alias = PLAIN_ALIAS.match(item)
                if alias is None or PLAIN_RHS.match(alias.group(2).strip()) is None:
                    refusals.append(f"{rel}: `{item}`")
                    continue
                aliases.append((alias.group(1), alias.group(2).strip(), rel))
            for use in USE.finditer(flat):
                renames.extend((new, old, rel) for old, new in RENAME.findall(use.group(1)))
            equalities.extend((name, rhs.strip(), rel) for name, rhs in EQUALITY.findall(flat))
    closure = set(VERDICT_TYPES)
    changed = True
    while changed:
        changed = False
        for name, rhs, _ in aliases:
            head = PLAIN_RHS.match(rhs).group("head").split("::")[-1]
            if name not in closure and head in closure:
                closure.add(name)
                changed = True
        for new, old, _ in renames:
            if new not in closure and old in closure:
                closure.add(new)
                changed = True
    names = re.compile(r"\b(" + "|".join(sorted(closure)) + r")\b")
    for name, rhs, rel in aliases:
        args = PLAIN_RHS.match(rhs).group("args") or ""
        if names.search(args):
            refusals.append(f"{rel}: `type {name} = {rhs}` hides a verdict type in a generic argument")
    for new, old, rel in renames:
        if old in closure:
            refusals.append(f"{rel}: `use ... {old} as {new}` renames a verdict type")
    for name, rhs, rel in equalities:
        if names.search(rhs):
            refusals.append(f"{rel}: the generic equality `{name} = {rhs}` binds a verdict type")
    bound = {name for name, _, _ in aliases}
    bindings = {name: rhs for name, rhs, _ in sorted(aliases) if name in closure}
    bindings.update({new: old for new, old, _ in sorted(renames) if new in closure})
    return closure, bindings, declared - bound, sorted(set(refusals))


PUBLIC_VALUE = "public value the scan cannot prove wire-only"
CALLABLE_WORDS = frozenset({"Fn", "FnMut", "FnOnce", "dyn"})


def _depth_zero(text: str, stops: str) -> int:
    depth = 0
    i = 0
    while i < len(text):
        if text.startswith("->", i):
            i += 2
            continue
        ch = text[i]
        if depth == 0 and ch in stops:
            return i
        if ch in "([{<":
            depth += 1
        elif ch in ")]}>":
            depth -= 1
        i += 1
    return len(text)


def _inside(text: str) -> str:
    inner = text[1:]
    return inner[: _depth_zero(inner, ")}")]


def _fields(text: str) -> list[str]:
    out = []
    while text.strip():
        end = _depth_zero(text, ",")
        out.append(text[:end].strip())
        text = text[end + 1 :]
    return out


def public_values(code: str, closure: set[str]) -> list[str]:
    """Public statics, consts (associated too), aliases, struct and tuple fields and enum
    variant fields whose type names a callable or a verdict-closure type (Codex
    cr-3i3rst round 6). The class is refused whole; the scan does not try to prove an
    instance wire-only. A verdict type's own variants may hold verdict-closure types (a
    verdict carries its claim, and is not a producer of one); they may still hold no
    callable.
    """

    def carries(ty: str, verdicts: bool = True) -> bool:
        squeezed = "".join(ty.split())
        words = set(re.split(r"[^A-Za-z0-9_]+", ty))
        callable_ = "fn(" in squeezed or "impl " in ty or bool(words & CALLABLE_WORDS)
        return callable_ or (verdicts and bool(words & closure))

    flat = " ".join(ATTRIBUTE.sub(" ", code).split())
    out: list[str] = []
    for m in re.finditer(r"(?<![A-Za-z0-9_])pub ", flat):
        rest = flat[m.end() :]
        if (rest.startswith("static ") or rest.startswith("const ")) and not rest.startswith("const fn "):
            item = rest[: _depth_zero(rest, ";")]
            ty = item.split(":", 1)[1].split("=", 1)[0] if ":" in item else ""
            if carries(ty):
                out.append(f"pub {item}")
        elif rest.startswith("type "):
            item = rest[: _depth_zero(rest, ";")]
            if carries(item.split("=", 1)[1] if "=" in item else ""):
                out.append(f"pub {item}")
        elif rest.startswith("struct "):
            body = rest[len("struct ") :]
            opener = min((i for i in (body.find(c) for c in "{(;") if i >= 0), default=len(body))
            if opener < len(body) and body[opener] in "{(":
                for field in _fields(_inside(body[opener:])):
                    if field.startswith("pub ") and carries(field.split(":", 1)[-1] if ":" in field else field[4:]):
                        out.append(f"pub field {field}")
        elif rest.startswith("enum "):
            body = rest[len("enum ") :]
            name = re.match(r"\w+", body)
            if name is None:
                continue
            own_verdict = name.group(0) in closure
            opener = body.find("{")
            if opener < 0:
                continue
            for variant in _fields(_inside(body[opener:])):
                start = min((i for i in (variant.find(c) for c in "{(") if i >= 0), default=-1)
                if start < 0:
                    continue
                for field in _fields(_inside(variant[start:])):
                    if carries(field.split(":", 1)[-1], verdicts=not own_verdict):
                        out.append(f"pub enum {name.group(0)} field {field}")
    return out


def rule_wire_bytes(tree: kcov.Tree) -> tuple[list[str], dict]:
    tag = "[verdict-from-wire-bytes]"
    out: list[str] = []
    api = public_api(tree)
    approved = read_inventory(tree)
    live = set(inventory_lines(api))
    if approved is None:
        out.append(f"{tag} {INVENTORY} is missing; every public item of the checking base must be approved")
        approved = set()
    for line in sorted(live - approved):
        out.append(f"{tag} unlisted public item: {line}")
    for line in sorted(approved - live):
        out.append(f"{tag} inventory entry names no live item: {line}")

    # Code the scan cannot see: `#[path]`, `include!`-family, a `mod` with no file
    # under `src/`. KCOV-07's own rule over all five crates, relayed as a TCB-02
    # failure too, because the inventory below would be blind to it.
    for violation in kcov.rule_no_dynamic_loading(tree):
        out.append(f"{tag} {HIDDEN}: {violation}")
    closure, bindings, unresolved_names, refusals = verdict_closure(tree)
    closure_names = re.compile(r"\b(" + "|".join(sorted(closure)) + r")\b")
    for crate in CHECKING_BASE:
        for rel, code in shipped_sources(tree, crate).items():
            out.extend(f"{tag} {rel}: {PUBLIC_VALUE}: `{value}`" for value in public_values(code, closure))
    for rel, text, params, ret in fn_tokens(tree):
        if ret == UNPARSED:
            out.append(f"{tag} {rel}: {UNPARSED}: `{text}`")
        elif text.startswith("fn(") or text.startswith("fn ("):
            if closure_names.search(ret) and params != "&[u8]":
                out.append(f"{tag} {rel}: {YIELDS}: the function-pointer type `{text} -> {ret}`")
    # Refused regardless of the inventory: no line approves a construct the scan
    # cannot resolve.
    out.extend(f"{tag} {UNRESOLVABLE}: {r}" for r in refusals)
    mentions = re.compile(r"\b(" + "|".join(sorted(closure)) + r")\b")

    surface: dict[str, dict] = {}
    for crate, rows in api.items():
        entries: list[str] = []
        for rel, kind, key in rows:
            if kind == UNCLASSIFIED:
                out.append(f"{tag} {rel}: the scan cannot classify `{key}`; it fails closed")
                continue
            if kind == "macro":
                # A macro can generate public items no lexical scan sees, so no inventory
                # line can approve one: the checking base declares and invokes none.
                out.append(f"{tag} {rel}: `{key}` can generate items the scan cannot see; it fails closed")
                continue
            if kind == "trait-impl":
                target = impl_target(key)
                trait = re.sub(r"^(unsafe\s+)?impl(<.*?>)?\s+", "", key).split(" for ")[0]
                if target in closure and trait.split("<")[0].split("::")[-1] in VALUE_CONSTRUCTORS:
                    out.append(f"{tag} {rel}: {YIELDS}: `{key}` builds one by conversion")
                continue
            if kind == "assoc":
                header, _, binding = key.partition(": ")
                target = impl_target(header)
                if mentions.search(binding.split("=", 1)[-1]) and target not in closure:
                    out.append(f"{tag} {rel}: {YIELDS}: `{key}` binds a verdict to an associated type of {target}")
                continue
            if kind not in ("fn", "method", "trait-method", "impl-method"):
                continue
            sig = key.split("fn ", 1)[1]
            parsed = parse_fn("fn " + sig)
            if parsed is None:
                out.append(f"{tag} {rel}: {UNPARSED}: `{key}`")
                continue
            if not parsed[2]:
                continue
            ret = parsed[2]
            unresolved = [name for name in projections(ret) if name in unresolved_names]
            if unresolved:
                out.append(
                    f"{tag} {rel}: `{key}` returns the projection {unresolved}, which no associated "
                    "type in the checking base binds; the scan cannot resolve it and fails closed"
                )
                continue
            if not mentions.search(ret):
                continue
            params = parsed[1]
            if kind == "impl-method":
                owner = impl_target(key.split(": fn ", 1)[0])
            else:
                owner = key.split(":", 1)[0].removeprefix("impl ").removeprefix("trait ").split("<")[0].strip()
            if kind == "fn" and params == "bytes: &[u8]":
                entries.append(f"{rel}: {key}")
            elif kind in ("method", "impl-method") and owner in VERDICT_TYPES and SELF.match(params):
                continue  # reads a verdict already decided
            else:
                out.append(f"{tag} {rel}: {YIELDS}: `{key}`")
        if not entries:
            out.append(f"{tag} {crate} has no public verdict entry point to audit")
        surface[crate] = {
            "public_items": sum(1 for r in rows if r[1] not in (UNCLASSIFIED, "macro", "assoc", "impl-method")),
            "verdict_entry_points": [
                {"entry": e, "verdict_type_closure": sorted(closure), "alias_bindings": bindings}
                for e in sorted(entries)
            ],
        }
    return out, surface


# --- what rustc actually compiled (Codex cr-3i3rst) --------------------------------------
#
# A lexical scan cannot know which file a `mod` declaration loads: `#[path]`, a
# `cfg_attr` that expands to one, or anything rustc resolves that the scan does not
# model can redirect it. So the audit reads rustc's own dep-info (`.d`) for each
# checking-base crate, from a `cargo check --locked` under the pinned toolchain, and
# requires the set of files rustc read to EQUAL the set the scan inventories. A file
# rustc read that the scan did not, or a scanned file rustc did not read, fails.

COMPILED = "a file rustc compiled that the scan did not see"
NOT_COMPILED = "a scanned file rustc did not compile"


def scanned_files(tree: kcov.Tree, crate: str) -> set[str]:
    return set(shipped_sources(tree, crate))


def parse_dep_info(text: str, root: pathlib.Path) -> tuple[set[str], list[str]]:
    """The prerequisites of the first rule of a make-style `.d` file, relative to root."""
    problems: list[str] = []
    lines = [line for line in text.splitlines() if line and not line.startswith("#")]
    if not lines or ": " not in lines[0] + " ":
        return set(), ["the dep-info file has no rule"]
    body = lines[0].split(":", 1)[1] if not lines[0].startswith("/") else lines[0].split(": ", 1)[1]
    paths = re.split(r"(?<!\\) ", body.strip())
    out: set[str] = set()
    for raw in (p.replace("\\ ", " ") for p in paths if p):
        path = pathlib.Path(raw)
        if not path.is_absolute():
            path = root / path  # rustc runs from the workspace root
        try:
            out.add(path.resolve().relative_to(root.resolve()).as_posix())
        except ValueError:
            problems.append(f"rustc read {raw}, outside the workspace")
    return out, problems


def compiler_inputs(
    root: pathlib.Path, target: pathlib.Path | None = None, locked: bool = True
) -> tuple[dict[str, set[str]], list[str]]:
    """crate -> files rustc read, from a fresh `cargo check --locked`; fail closed.

    `locked=False` only for the lane's scratch workspace, which holds seven members and
    must prune the copied lockfile; nothing is fetched either way (`--offline`).
    """
    problems: list[str] = []
    env = {**os.environ}
    if target is not None:
        env["CARGO_TARGET_DIR"] = str(target)
    cmd = ["cargo", "check", *(["--locked"] if locked else []), "--offline", "--message-format=json"]
    for crate in CHECKING_BASE:
        cmd += ["-p", crate]
    proc = subprocess.run(cmd, cwd=root, capture_output=True, text=True, env=env, check=False)
    if proc.returncode != 0:
        return {}, [f"`cargo check` of the checking base failed, so the compiled inputs are unknown: {proc.stderr[-400:]}"]
    inputs: dict[str, set[str]] = {}
    for line in proc.stdout.splitlines():
        try:
            message = json.loads(line)
        except ValueError:
            continue
        if message.get("reason") != "compiler-artifact":
            continue
        name = message.get("package_id", "")
        crate = next((c for c in CHECKING_BASE if f"/{c}#" in name or name.startswith(f"{c} ")), None)
        if crate is None or "lib" not in message.get("target", {}).get("kind", []):
            continue
        for artifact in message.get("filenames", []):
            path = pathlib.Path(artifact)
            stem = path.name.split(".", 1)[0].removeprefix("lib")
            dep_info = path.with_name(f"{stem}.d")
            if not dep_info.is_file():
                problems.append(f"{crate}: rustc's dep-info {dep_info} is missing")
                continue
            files, parse_problems = parse_dep_info(dep_info.read_text(encoding="utf-8"), root)
            problems.extend(f"{crate}: {p}" for p in parse_problems)
            stamp = dep_info.stat().st_mtime
            for rel in files:
                source = root / rel
                if not source.is_file() or source.stat().st_mtime > stamp:
                    problems.append(f"{crate}: rustc's dep-info is stale relative to {rel}")
            inputs.setdefault(crate, set()).update(files)
    for crate in CHECKING_BASE:
        if crate not in inputs:
            problems.append(f"{crate}: cargo reported no library artifact, so its compiled inputs are unknown")
    return inputs, problems


def rule_compiled_inputs(tree: kcov.Tree, inputs: dict[str, set[str]], problems: list[str]) -> list[str]:
    tag = "[verdict-from-wire-bytes]"
    out = [f"{tag} {p}" for p in problems]
    for crate in CHECKING_BASE:
        compiled = inputs.get(crate, set())
        scanned = scanned_files(tree, crate)
        out.extend(f"{tag} {COMPILED}: {rel}" for rel in sorted(compiled - scanned))
        out.extend(f"{tag} {NOT_COMPILED}: {rel}" for rel in sorted(scanned - compiled))
    return out


def rule_tests_live(tree: kcov.Tree) -> list[str]:
    out: list[str] = []
    for rel, name in REQUIRED_TESTS:
        text = tree.read_text(rel)
        if text is None:
            out.append(f"[campaign-tests-live] {rel} is missing")
            continue
        lines = text.splitlines()
        index = next((i for i, l in enumerate(lines) if re.match(rf"\s*fn {name}\s*\(", l)), None)
        if index is None:
            out.append(f"[campaign-tests-live] {rel}::{name} does not exist")
            continue
        attrs: list[str] = []
        j = index - 1
        while j >= 0 and (lines[j].strip().startswith(("#[", "//")) or not lines[j].strip()):
            attrs.append(lines[j].strip())
            j -= 1
        if "#[test]" not in attrs:
            out.append(f"[campaign-tests-live] {rel}::{name} is not a #[test]")
        if any(a.startswith("#[ignore") for a in attrs):
            out.append(f"[campaign-tests-live] {rel}::{name} is #[ignore]d")
    return out


def rule_inventory(tree: kcov.Tree, mutants: list[dict]) -> list[str]:
    out: list[str] = []
    seen: set[str] = set()
    for m in mutants:
        mid = m.get("id", "?")
        if mid in seen:
            out.append(f"[mutant-inventory] duplicate id {mid}")
        seen.add(mid)
        side = m.get("side")
        if side not in EXPECT:
            out.append(f"[mutant-inventory] {mid}: side {side!r} is not checker or producer")
            continue
        if crate_of(m.get("file", "")) not in SIDE_CRATES[side]:
            out.append(f"[mutant-inventory] {mid}: {m.get('file')} is not a {side} crate")
        if m.get("expect") not in EXPECT[side]:
            out.append(f"[mutant-inventory] {mid}: expect {m.get('expect')!r} is not one of {EXPECT[side]}")
        if m.get("expect") in NEEDS_REASON and len(str(m.get("reason", ""))) < 24:
            out.append(f"[mutant-inventory] {mid}: expect {m.get('expect')} needs a reason")
        for key in ("find", "replace", "targets"):
            if not isinstance(m.get(key), str):
                out.append(f"[mutant-inventory] {mid}: `{key}` is missing")
        text = tree.read_text(m.get("file", "")) or ""
        count = text.count(m.get("find") or "\0")
        if count != 1:
            out.append(f"[mutant-inventory] {mid}: `find` occurs {count} times in {m.get('file')}, not once")
    return out


def summarize(rows: list[dict]) -> dict:
    """The summary, recomputed from the mutant rows: one definition for write and check."""
    checker = [r for r in rows if r.get("side") == "checker"]
    producer = [r for r in rows if r.get("side") == "producer"]
    return {
        "checker_mutants": len(checker),
        "checker_killed": sum(r.get("outcome") == "killed" for r in checker),
        "checker_survived": sorted(r.get("id") for r in checker if r.get("outcome") == "survives"),
        "checker_kill_classes": {k: sum(r.get("kill_class") == k for r in checker) for k in KILL_ORDER},
        "producer_mutants": len(producer),
        "producer_outcomes": {k: sorted(r.get("id") for r in producer if r.get("outcome") == k) for k in EXPECT["producer"]},
    }


RULES = [
    "TCB-01 checking-base-links-only-itself",
    "TCB-02 verdict-from-wire-bytes",
    "TCB-03 campaign-tests-live",
    "TCB-04 mutant-inventory",
    "TCB-05 evidence-current",
]


def stable_record(
    tree: kcov.Tree, mutants: list[dict], surface: dict, inputs: dict[str, set[str]] | None = None
) -> dict:
    """Every retained field that is a function of the tree alone."""
    digest, files = source_digest(tree, mutants, inputs)
    m = measure(tree)
    return {
        "claim": "C018",
        "bone": "bn-2npu",
        "tool": "tools/check_tcb_audit.py",
        "graph_source": "cargo metadata --format-version 1 --locked --offline (resolve by package id; normal, build and dev edges)",
        "checking_base": list(CHECKING_BASE),
        "rules": RULES,
        "surface": surface,
        "measurement": {k: m[k] for k in ("method", "checking_base", "checking_base_total", "finite_closure_checker")},
        "checker_inputs": files,
        "checker_inputs_sha256": digest,
        "tcb02_known_misses": {f["id"]: f["why"] for f in load_api_fixtures() if f.get("known_miss")},
        "lane_targets": {
            "checker": f"{CERTIFICATE} --test c018_checker_mutation",
            "producer": f"{REFERENCE} --test c018_producer_corpus",
        },
    }


def producer_record(tree: kcov.Tree, mutants: list[dict], rows: list[dict]) -> dict:
    """The retained fields that depend on producer sources as well."""
    m = measure(tree)
    return {
        "producer_inputs_sha256": producer_digest(tree),
        "producer_measurement": {k: m[k] for k in ("producers", "scaffold_engines_code_lines")},
        "producer_residual": residual(tree, mutants, {r.get("id"): r.get("outcome") for r in rows}),
    }


CAMPAIGN_KEYS = ("control", "summary", "mutants")


def validate_record(
    tree: kcov.Tree,
    mutants: list[dict],
    record: dict | None,
    surface: dict,
    inputs: dict[str, set[str]] | None = None,
) -> list[str]:
    """TCB-05: the whole retained record, not only its digest (cr-10mg1y round 4).

    Every tree-derived field equals a recomputation; the campaign's control passed and
    every TCB-02 fixture compiled; the rows name each inventory mutant exactly once,
    with its side, file and target, and the outcome it expects; the summary is the one
    `summarize` recomputes from the rows. The producer-derived fields (producer digest,
    producer measurements, residual) are compared too, so an edit to
    `continuum-engine-reference` or `continuum-model-core` sources also requires the
    lane to run again.
    """
    tag = "[evidence-current]"
    if not isinstance(record, dict):
        return [f"{tag} {EVIDENCE} is missing or not a JSON object; run --evidence"]
    out: list[str] = []
    stable = stable_record(tree, mutants, surface, inputs)
    expected_keys = set(stable) | set(CAMPAIGN_KEYS) | {"producer_inputs_sha256", "producer_measurement", "producer_residual"}
    if set(record) != expected_keys:
        out.append(f"{tag} the record's fields {sorted(record)} are not {sorted(expected_keys)}")
    for key, value in stable.items():
        if record.get(key) != value:
            out.append(f"{tag} `{key}` differs from a recomputation over this tree; re-run --evidence")
    control = record.get("control")
    fixtures = {f["id"] for f in load_api_fixtures()}
    if not isinstance(control, dict):
        out.append(f"{tag} `control` is missing")
    else:
        if control.get("checker") != "passed" or control.get("producer") != "passed":
            out.append(f"{tag} the lane's unmutated control did not pass: {control}")
        if not isinstance(control.get("producer_lines"), list) or len(control.get("producer_lines")) != 3:
            out.append(f"{tag} the control does not record one probe line per corpus model")
        compiled = control.get("tcb02_fixtures_compile")
        if compiled != {fid: True for fid in fixtures}:
            out.append(f"{tag} not every TCB-02 fixture is recorded as compiling: {compiled}")
        expected_authority = {
            f["id"]: bool(f.get("expect_authority_failure")) for f in load_api_fixtures() if f.get("authority_test")
        }
        if control.get("tcb02_fixtures_fail_the_authority_test") != expected_authority:
            out.append(
                f"{tag} the Rust authority test's recorded answer per fixture is not the expected one: "
                f"{control.get('tcb02_fixtures_fail_the_authority_test')}"
            )
        expected_match = {f["id"]: not f.get("expect_compiled_mismatch", False) for f in load_api_fixtures()}
        if control.get("tcb02_fixtures_compiled_inputs_match") != expected_match:
            out.append(
                f"{tag} the dep-info rule's recorded answer per fixture is not the expected one: "
                f"{control.get('tcb02_fixtures_compiled_inputs_match')}"
            )
    rows = record.get("mutants")
    if not isinstance(rows, list) or not all(isinstance(r, dict) for r in rows):
        out.append(f"{tag} `mutants` is not a list of rows")
        rows = []
    ids = [r.get("id") for r in rows]
    for mid in sorted({i for i in ids if ids.count(i) > 1}, key=str):
        out.append(f"{tag} mutant {mid} is recorded {ids.count(mid)} times")
    recorded = {r.get("id"): r for r in rows}
    for m in mutants:
        row = recorded.pop(m.get("id"), None)
        if row is None:
            out.append(f"{tag} mutant {m.get('id')} has no recorded outcome")
            continue
        for key in ("side", "file", "targets"):
            if row.get(key) != m.get(key):
                out.append(f"{tag} mutant {m.get('id')}: recorded {key} {row.get(key)!r} is not the inventory's")
        if row.get("outcome") != m.get("expect"):
            out.append(f"{tag} mutant {m.get('id')}: recorded {row.get('outcome')!r}, the inventory expects {m.get('expect')!r}")
        if row.get("side") == "checker":
            killed = row.get("outcome") == "killed"
            if killed != bool(row.get("failed_tests")) or killed != (row.get("kill_class") in KILL_ORDER):
                out.append(f"{tag} mutant {m.get('id')}: its failed tests and kill class do not match its outcome")
    for mid in sorted((k for k in recorded), key=str):
        out.append(f"{tag} recorded mutant {mid} is not in the inventory")
    if record.get("summary") != summarize(rows):
        out.append(f"{tag} `summary` is not the summary of the recorded rows")
    for key, value in producer_record(tree, mutants, rows).items():
        if record.get(key) != value:
            out.append(
                f"{tag} `{key}` differs from a recomputation over this tree; the producer "
                "sources moved since the lane ran, so re-run --evidence"
            )
    return out


def write_evidence(record: dict, problems: list[str], path: pathlib.Path) -> bool:
    """Replace `path` atomically with `record`, only when there are no problems.

    A failed campaign leaves the previous record byte-identical: the new one is
    written to a temporary file in the same directory and moved over the old one only
    after every control, fixture, outcome and invariant has passed.
    """
    if problems:
        return False
    path.parent.mkdir(parents=True, exist_ok=True)
    handle, temporary = tempfile.mkstemp(prefix=".c018-", suffix=".json", dir=path.parent)
    try:
        with os.fdopen(handle, "w", encoding="utf-8") as stream:
            stream.write(json.dumps(record, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
    return True


# --- measurement -----------------------------------------------------------------


def measure(tree: kcov.Tree) -> dict:
    def count(crate: str) -> dict:
        c = kcov.count_crate(tree, crate)
        return {
            "lines": c.total,
            "code_lines": c.non_blank_non_comment,
            "by_file": {rel.rsplit("/", 1)[-1]: n for rel, n in sorted(c.counted.items())},
        }

    base = {crate: count(crate) for crate in CHECKING_BASE}
    producers = {crate: count(crate) for crate in PRODUCER_CRATES}
    scaffolds = {crate: count(crate)["code_lines"] for crate in SCAFFOLD_ENGINES}
    return {
        "method": "tools/check_kernel_covenant.py count_crate: shipped src lines; code_lines are non-blank non-comment",
        "checking_base": base,
        "checking_base_total": {
            "lines": sum(v["lines"] for v in base.values()),
            "code_lines": sum(v["code_lines"] for v in base.values()),
        },
        "finite_closure_checker": {
            "crates": [CERTIFICATE, "continuum-kernel-core"],
            "lines": base[CERTIFICATE]["lines"] + base["continuum-kernel-core"]["lines"],
            "code_lines": base[CERTIFICATE]["code_lines"] + base["continuum-kernel-core"]["code_lines"],
        },
        "producers": producers,
        "scaffold_engines_code_lines": scaffolds,
    }


def residual(tree: kcov.Tree, mutants: list[dict], outcomes: dict[str, str]) -> dict:
    """Which producer files the kernel's verdict removes from trust, per the lane.

    A file with a `missed` mutant stays trusted: a bug there reaches a verified
    certificate. A file whose every mutant is caught (or refused before emission)
    has no observed escape. This is a lower bound on the residual, not a proof of
    removal: an uncaught mutant class nobody wrote is not in the table.
    """
    per_file: dict[str, dict[str, int]] = {}
    for m in mutants:
        if m.get("side") != "producer":
            continue
        row = per_file.setdefault(m["file"], {})
        outcome = outcomes.get(m["id"], "unrun")
        row[outcome] = row.get(outcome, 0) + 1
    lines = {}
    for crate in PRODUCER_CRATES:
        c = kcov.count_crate(tree, crate)
        lines.update(c.counted)
    trusted = sorted(f for f, r in per_file.items() if r.get("missed"))
    untrusted = sorted(f for f, r in per_file.items() if not r.get("missed"))
    return {
        "per_file": per_file,
        "still_trusted": {f: lines.get(f, 0) for f in trusted},
        "no_observed_escape": {f: lines.get(f, 0) for f in untrusted},
        "still_trusted_lines": sum(lines.get(f, 0) for f in trusted),
        "no_observed_escape_lines": sum(lines.get(f, 0) for f in untrusted),
    }


# --- the mutation lane -------------------------------------------------------------


LANE_CRATES = (*CHECKING_BASE, *PRODUCER_CRATES)


def scratch_workspace() -> pathlib.Path:
    base = pathlib.Path(os.environ.get("TMPDIR") or ROOT / "target" / "tmp")
    base.mkdir(parents=True, exist_ok=True)
    work = pathlib.Path(tempfile.mkdtemp(prefix="c018-lane-", dir=base))
    for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml"):
        shutil.copy2(ROOT / name, work / name)
    for crate in LANE_CRATES:
        shutil.copytree(ROOT / "crates" / crate, work / "crates" / crate, ignore=shutil.ignore_patterns("target"))
    return work


def cargo_test(work: pathlib.Path, package: str, target: str, nocapture: bool) -> subprocess.CompletedProcess:
    # A target directory private to this scratch copy. A shared one is unsound here:
    # `copy2` keeps the tree's old mtimes, so cargo would reuse the last run's build of
    # a *mutated* file as fresh for the pristine copy.
    env = {**os.environ, "CARGO_TARGET_DIR": str(work / "target")}
    # `--offline`, not `--locked`: the scratch workspace holds seven of the members, so
    # cargo must prune the copied lockfile. No package is fetched either way.
    cmd = ["cargo", "test", "--offline", "-p", package, "--test", target, "--", "--test-threads=1"]
    if nocapture:
        cmd.append("--nocapture")
    try:
        return subprocess.run(cmd, cwd=work, capture_output=True, text=True, env=env, check=False, timeout=900)
    except subprocess.TimeoutExpired as expired:
        out = expired.stdout.decode() if isinstance(expired.stdout, bytes) else (expired.stdout or "")
        return subprocess.CompletedProcess(cmd, -1, out, "TIMEOUT")


FAILED_TEST = re.compile(r"^test (\w+) \.\.\. FAILED$", re.M)
# Not anchored: libtest prints `test <name> ... ` before the captured output.
PRODUCER_LINE = re.compile(r"C018-PRODUCER [^\n]*")


def run_checker(work: pathlib.Path) -> tuple[str, list[str], str]:
    proc = cargo_test(work, CERTIFICATE, "c018_checker_mutation", nocapture=False)
    if proc.stderr == "TIMEOUT":
        return "failed", ["timeout"], "the target did not finish in 900 s"
    if "error[E" in proc.stderr or "could not compile" in proc.stderr:
        return "compile-error", [], proc.stderr[-400:]
    failed = sorted(set(FAILED_TEST.findall(proc.stdout)))
    if proc.returncode == 0:
        return "passed", [], ""
    if not failed:
        return "unparsed", [], (proc.stdout + proc.stderr)[-400:]
    return "failed", failed, ""


def run_producer(work: pathlib.Path) -> tuple[str, list[str], str]:
    proc = cargo_test(work, REFERENCE, "c018_producer_corpus", nocapture=True)
    if proc.stderr == "TIMEOUT":
        return "timeout", [], "the target did not finish in 900 s"
    if "error[E" in proc.stderr or "could not compile" in proc.stderr:
        return "compile-error", [], proc.stderr[-400:]
    # The deterministic-probe test prints nothing, so each model prints exactly once.
    lines = sorted(set(PRODUCER_LINE.findall(proc.stdout)))
    return ("passed" if proc.returncode == 0 else "failed"), lines, ""


def model_of(line: str) -> str:
    return line.split(" ", 2)[1] if line.count(" ") >= 2 else line


def classify_model(before: str, after: str) -> str:
    if after == before:
        return "no-effect"
    if " rejected:" in after or " unsupported:" in after:
        return "caught"
    if re.search(r" (explore|emit)-", after):
        return "refused"
    return "missed"


def classify_producer(baseline: list[str], lines: list[str]) -> str:
    """One outcome per corpus model, then the worst: missed > caught > refused.

    Every model must print exactly one line; a run that lost a line (a panic in the
    probe, say) is `unparsed`, never `missed` or `caught`.
    """
    before = {model_of(line): line for line in baseline}
    after = {model_of(line): line for line in lines}
    if len(after) != len(lines) or set(after) != set(before):
        return "unparsed"
    per_model = {classify_model(before[m], after[m]) for m in before}
    for outcome in ("missed", "caught", "refused"):
        if outcome in per_model:
            return outcome
    return "no-effect"


def run_lane(mutants: list[dict]) -> tuple[list[dict], dict, list[str]]:
    failures: list[str] = []
    work = scratch_workspace()
    results: list[dict] = []
    try:
        status, failed, detail = run_checker(work)
        if status != "passed":
            failures.append(f"lane control: the unmutated checker target did not pass ({status} {failed} {detail})")
        pstatus, baseline, pdetail = run_producer(work)
        if pstatus != "passed" or len(baseline) != 3:
            failures.append(f"lane control: the unmutated producer target did not pass ({pstatus} {pdetail})")
        control = {"checker": status, "producer": pstatus, "producer_lines": baseline}
        # The TCB-02 fixtures are real Rust: each must compile in the crate it names, or
        # it would prove only that the scan rejects text rustc also rejects.
        compiled: dict[str, bool] = {}
        matched: dict[str, bool] = {}
        authority_failed: dict[str, bool] = {}
        for fixture in load_api_fixtures():
            files = sorted({edit["file"] for edit in fixture["edits"]})
            pristine = {rel: (work / rel).read_text(encoding="utf-8") if (work / rel).exists() else None for rel in files}
            for edit in fixture["edits"]:
                path = work / edit["file"]
                before = path.read_text(encoding="utf-8") if path.exists() else ""
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(before + edit["append"], encoding="utf-8")
            try:
                env = {**os.environ, "CARGO_TARGET_DIR": str(work / "target")}
                proc = subprocess.run(
                    ["cargo", "check", "--offline", "-p", fixture["crate"]],
                    cwd=work, capture_output=True, text=True, env=env, check=False, timeout=900,
                )
                compiled[fixture["id"]] = proc.returncode == 0
                if proc.returncode != 0:
                    failures.append(f"{fixture['id']}: the TCB-02 fixture does not compile: {proc.stderr[-400:]}")
                else:
                    # What rustc read for the mutated tree, against what the scan sees.
                    got, got_problems = compiler_inputs(work, work / "target", locked=False)
                    mismatch = bool(rule_compiled_inputs(kcov.Tree(work), got, got_problems))
                    matched[fixture["id"]] = not mismatch
                    authority = fixture.get("authority_test")
                    if authority:
                        # The per-crate Rust authority test must fail on this fixture too.
                        run = subprocess.run(
                            ["cargo", "test", "--offline", "-p", authority["package"], "--test", authority["target"]],
                            cwd=work, capture_output=True, text=True, env=env, check=False, timeout=900,
                        )
                        failed = run.returncode != 0 and "test result: FAILED" in run.stdout
                        authority_failed[fixture["id"]] = failed
                        if failed != bool(fixture.get("expect_authority_failure")):
                            failures.append(f"{fixture['id']}: the Rust authority test {'failed' if failed else 'passed'}, expected the opposite")
                    if mismatch != bool(fixture.get("expect_compiled_mismatch", False)):
                        failures.append(
                            f"{fixture['id']}: the dep-info rule {'fired' if mismatch else 'did not fire'}, "
                            f"expected the opposite"
                        )
            finally:
                for rel, text in pristine.items():
                    if text is None:
                        (work / rel).unlink()
                        try:
                            (work / rel).parent.rmdir()  # a directory the fixture created
                        except OSError:
                            pass
                    else:
                        (work / rel).write_text(text, encoding="utf-8")
        control["tcb02_fixtures_compile"] = compiled
        control["tcb02_fixtures_compiled_inputs_match"] = matched
        control["tcb02_fixtures_fail_the_authority_test"] = authority_failed
        for m in mutants:
            path = work / m["file"]
            pristine = path.read_text(encoding="utf-8")
            if pristine.count(m["find"]) != 1:
                failures.append(f"{m['id']}: `find` does not occur exactly once in the scratch copy")
                continue
            path.write_text(pristine.replace(m["find"], m["replace"], 1), encoding="utf-8")
            try:
                if m["side"] == "checker":
                    status, failed, detail = run_checker(work)
                    classes = sorted({KILL_CLASS.get(t, "other") for t in failed}, key=_kill_rank)
                    outcome = "killed" if status == "failed" else "survives" if status == "passed" else status
                    row = {
                        "id": m["id"],
                        "side": "checker",
                        "file": m["file"],
                        "targets": m["targets"],
                        "outcome": outcome,
                        "failed_tests": failed,
                        "kill_class": classes[0] if classes else None,
                    }
                else:
                    status, lines, detail = run_producer(work)
                    outcome = classify_producer(baseline, lines) if status != "compile-error" else status
                    row = {
                        "id": m["id"],
                        "side": "producer",
                        "file": m["file"],
                        "targets": m["targets"],
                        "outcome": outcome,
                        "lines": [line for line in lines if line not in baseline],
                    }
                if detail:
                    row["detail"] = detail
                if outcome == "compile-error":
                    failures.append(f"{m['id']}: the mutant does not compile, so it tests nothing: {detail}")
                results.append(row)
                print(f"  {m['id']}: {outcome}", file=sys.stderr)
            finally:
                path.write_text(pristine, encoding="utf-8")
    finally:
        shutil.rmtree(work, ignore_errors=True)
    return results, control, failures


def _kill_rank(name: str) -> int:
    return KILL_ORDER.index(name) if name in KILL_ORDER else len(KILL_ORDER)


# --- self-test ---------------------------------------------------------------------


def fixture_overlay(tree: kcov.Tree, fixture: dict) -> dict[str, str]:
    """The fixture's source edits, plus its own inventory lines appended."""
    overlay: dict[str, str] = {}
    for edit in fixture["edits"]:
        overlay[edit["file"]] = overlay.get(edit["file"], tree.read_text(edit["file"]) or "") + edit["append"]
    overlay[INVENTORY] = (tree.read_text(INVENTORY) or "") + "".join(f"{line}\n" for line in fixture["inventory_add"])
    return overlay


def load_api_fixtures() -> list[dict]:
    return [json.loads(p.read_text(encoding="utf-8")) for p in sorted((ROOT / AUDIT_DIR / "fixtures").glob("tcb02-*.json"))]


def metadata_fixtures(meta: dict) -> list[tuple[str, dict]]:
    """Real `cargo metadata` with one identity defect each (cr-10mg1y)."""
    ids = {pkg["name"]: pkg["id"] for pkg in meta["packages"] if pkg["id"] in meta["workspace_members"]}
    external = next(pkg["id"] for pkg in meta["packages"] if pkg.get("source"))
    out: list[tuple[str, dict]] = []

    def with_package(pid: str, version: str, source: str | None, dep_from: str, kind: str | None) -> dict:
        m = json.loads(json.dumps(meta))
        m["packages"].append({"id": pid, "name": "continuum-kernel-core", "version": version, "source": source})
        m["resolve"]["nodes"].append({"id": pid, "deps": []})
        for node in m["resolve"]["nodes"]:
            if node["id"] == ids[dep_from]:
                node["deps"].append({"name": "continuum_kernel_core", "pkg": pid, "dep_kinds": [{"kind": kind, "target": None}]})
        return m

    out.append((
        "a same-name external package",
        with_package(
            "registry+https://github.com/rust-lang/crates.io-index#continuum-kernel-core@9.9.9",
            "9.9.9", "registry+https://github.com/rust-lang/crates.io-index", CERTIFICATE, None,
        ),
    ))
    out.append((
        "a second version of a checking-base crate",
        with_package(
            "path+file:///elsewhere/continuum-kernel-core#0.0.1", "0.0.1", None, "continuum-kernel-sat", "dev",
        ),
    ))
    aliased = json.loads(json.dumps(meta))
    for node in aliased["resolve"]["nodes"]:
        if node["id"] == ids[CERTIFICATE]:
            node["deps"].append({"name": "continuum_kernel_smt", "pkg": external, "dep_kinds": [{"kind": None, "target": None}]})
    out.append(("an aliased dependency", aliased))
    engine = json.loads(json.dumps(meta))
    for node in engine["resolve"]["nodes"]:
        if node["id"] == ids[CERTIFICATE]:
            node["deps"].append({"name": "continuum_engine_reference", "pkg": ids[REFERENCE], "dep_kinds": [{"kind": "dev", "target": None}]})
    out.append(("a checking-base dev edge to the reference engine", engine))
    return out


def self_test(tree: kcov.Tree, meta: dict, mutants: list[dict], evidence: dict | None) -> list[str]:
    """Each fixture is a real input with one defect; its rule must name it."""
    failures: list[str] = []

    def expect(label: str, findings: list[str], tag: str) -> None:
        if not any(f.startswith(f"[{tag}]") for f in findings):
            failures.append(f"self-test {label}: rule [{tag}] did not fire")

    for label, mutated in metadata_fixtures(meta):
        links = rule_links(mutated)
        expect(label, links, "checking-base-links-only-itself")
        if not any(" -> " in f or "names" in f or "renames" in f or "not a workspace member" in f for f in links):
            failures.append(f"self-test {label}: TCB-01 fired without a witness")
        if "reference engine" not in label:
            # The name-keyed graph C023 reads must see the same identity defect.
            c023 = trip.evaluate(trip.build_graph(mutated), ROOT, trip.load_gate())
            expect(f"{label} (C023's graph reader)", c023, "package-identity")

    rel = "crates/continuum-kernel-core/src/check.rs"
    decoded = (tree.read_text(rel) or "") + (
        "\n/// Mutant: a verdict from a decoded certificate.\n"
        "pub fn check_decoded(\n    certificate: &crate::wire::Certificate,\n) -> Verdict {\n"
        "    let _ = certificate;\n    check_certificate(&[])\n}\n"
    )
    expect("an in-memory verdict entry", rule_wire_bytes(tree.with_changes({rel: decoded}))[0], "verdict-from-wire-bytes")
    rel = "crates/continuum-certificate/src/check.rs"
    routed = (tree.read_text(rel) or "") + (
        "\n/// Mutant: routing from a decoded family.\n"
        "pub const fn check_family(family: Family) -> Outcome {\n    let _ = family;\n    check_certificate(&[])\n}\n"
    )
    expect("a router entry that skips bytes", rule_wire_bytes(tree.with_changes({rel: routed}))[0], "verdict-from-wire-bytes")
    rel = "crates/continuum-kernel-sat/src/check.rs"
    claimed = (tree.read_text(rel) or "") + (
        "\n/// Mutant: a claim from a decoded certificate.\n"
        "pub fn claim_decoded(certificate: &crate::wire::Certificate) -> Result<CheckedClaim, Rejection> {\n"
        "    let _ = certificate;\n    Err(Rejection::EmptyClauseNotDerived)\n}\n"
    )
    expect("a claim entry that skips bytes", rule_wire_bytes(tree.with_changes({rel: claimed}))[0], "verdict-from-wire-bytes")

    # Each TCB-02 fixture approves its own new public items in the inventory it runs
    # against, so inventory drift cannot be what fails it: the named semantic
    # diagnostic must appear, and no inventory finding may (cr-10mg1y round 2).
    for fixture in load_api_fixtures():
        overlay = fixture_overlay(tree, fixture)
        findings = rule_wire_bytes(tree.with_changes(overlay))[0]
        if fixture.get("known_miss"):
            # A pinned residual: the scan must still accept it, so the evidence can
            # say so. If a later change catches it, this fixture must be revisited.
            if findings:
                failures.append(f"self-test {fixture['id']}: a known miss is now caught; revisit it: {findings}")
            continue
        drift = [f for f in findings if "unlisted public item" in f or "inventory entry names no live item" in f]
        if drift:
            failures.append(f"self-test {fixture['id']}: its inventory_add is stale, so it tests drift: {drift}")
        if not any(fixture["expect"] in f for f in findings):
            failures.append(f"self-test {fixture['id']}: no finding carries `{fixture['expect']}`: {findings}")

    # A projection nothing in the checking base binds cannot be resolved: fail closed.
    rel = "crates/continuum-kernel-core/src/check.rs"
    opaque = (tree.read_text(rel) or "") + (
        "\n/// Fixture.\npub trait Opaque {\n    /// Fixture.\n    type Out;\n"
        "    /// Fixture.\n    fn run(&self) -> Self::Out;\n}\n"
    )
    findings = rule_wire_bytes(tree.with_changes({rel: opaque}))[0]
    if not any("cannot resolve it and fails closed" in f for f in findings):
        failures.append(f"self-test an unresolvable projection: no fail-closed finding: {findings}")

    text = tree.read_text(CHECKER_TEST) or ""
    ignored = text.replace("#[test]\nfn no_lie_verifies()", "#[test]\n#[ignore]\nfn no_lie_verifies()", 1)
    expect("an ignored campaign test", rule_tests_live(tree.with_changes({CHECKER_TEST: ignored})), "campaign-tests-live")
    renamed = text.replace("fn no_lie_verifies()", "fn no_lie_verifies_renamed()", 1)
    expect("a renamed campaign test", rule_tests_live(tree.with_changes({CHECKER_TEST: renamed})), "campaign-tests-live")

    if mutants:
        first = dict(mutants[0])
        expect("a duplicate mutant id", rule_inventory(tree, [first, dict(first)]), "mutant-inventory")
        drifted = {**first, "find": first["find"] + "  /* drifted */"}
        expect("a mutant whose target moved", rule_inventory(tree, [drifted]), "mutant-inventory")
        wrong_side = {**first, "side": "producer"}
        expect("a checker file on the producer side", rule_inventory(tree, [wrong_side]), "mutant-inventory")
        silent = {**first, "expect": "survives", "reason": ""}
        expect("a survivor with no reason", rule_inventory(tree, [silent]), "mutant-inventory")

    # The dep-info rule, over synthetic compiler inputs: a file rustc read that the scan
    # did not, and a scanned file rustc did not read, each fail (cr-3i3rst).
    scan_sets = {crate: scanned_files(tree, crate) for crate in CHECKING_BASE}
    hidden = "crates/continuum-certificate/hidden/decide.rs"
    extra = {**scan_sets, CERTIFICATE: scan_sets[CERTIFICATE] | {hidden}}
    if not any(COMPILED in f for f in rule_compiled_inputs(tree, extra, [])):
        failures.append("self-test the dep-info rule accepted a file rustc read and the scan did not")
    fewer = {**scan_sets, CERTIFICATE: scan_sets[CERTIFICATE] - {"crates/continuum-certificate/src/check.rs"}}
    if not any(NOT_COMPILED in f for f in rule_compiled_inputs(tree, fewer, [])):
        failures.append("self-test the dep-info rule accepted a scanned file rustc did not read")
    if rule_compiled_inputs(tree, scan_sets, []):
        failures.append("self-test the dep-info rule fired on equal sets")
    if not rule_compiled_inputs(tree, scan_sets, ["dep-info missing"]):
        failures.append("self-test a missing dep-info did not fail closed")

    surface = rule_wire_bytes(tree)[1]
    if evidence is not None and mutants:
        # The redirect target changes, and nothing else: TCB-05 must fail.
        tree_a = tree.with_changes({hidden: "// redirect target, version A\n"})
        tree_b = tree.with_changes({hidden: "// redirect target, version B\n"})
        record_a = {**json.loads(json.dumps(evidence)), **stable_record(tree_a, mutants, surface, extra)}
        if validate_record(tree_a, mutants, record_a, surface, extra):
            failures.append("self-test control: a record over the redirect target does not validate over it")
        if not validate_record(tree_b, mutants, record_a, surface, extra):
            failures.append("self-test a changed redirect target did not fail TCB-05")

    if evidence is not None and mutants:
        clean = validate_record(tree, mutants, evidence, surface)
        if clean:
            # The corruption cases below need a valid record to corrupt.
            failures.append(f"self-test control: the committed record does not validate: {clean}")
            evidence = None
    if evidence is not None and mutants:

        def corrupt(label: str, edit) -> None:
            broken = json.loads(json.dumps(evidence))
            edit(broken)
            expect(label, validate_record(tree, mutants, broken, surface), "evidence-current")

        # Every retained top-level field, corrupted.
        for key in sorted(evidence):
            corrupt(f"corrupted `{key}`", lambda r, key=key: r.__setitem__(key, "corrupt"))
        corrupt("a dropped field", lambda r: r.pop("summary"))
        corrupt("an extra field", lambda r: r.__setitem__("extra", 1))
        # The campaign's contents, corrupted where a digest would not notice.
        corrupt("a failed control", lambda r: r["control"].__setitem__("checker", "failed"))
        corrupt("a fixture that did not compile", lambda r: r["control"]["tcb02_fixtures_compile"].__setitem__(sorted(r["control"]["tcb02_fixtures_compile"])[0], False))
        corrupt("a duplicate row", lambda r: r["mutants"].append(dict(r["mutants"][0])))
        corrupt("a dropped row", lambda r: r["mutants"].pop(0))
        corrupt("a flipped outcome", lambda r: next(x for x in r["mutants"] if x["outcome"] == "killed").__setitem__("outcome", "survives"))
        corrupt("a kill with no failing test", lambda r: next(x for x in r["mutants"] if x["outcome"] == "killed").__setitem__("failed_tests", []))
        corrupt("a retargeted row", lambda r: r["mutants"][0].__setitem__("file", "crates/elsewhere.rs"))
        corrupt("a summary that is not the rows'", lambda r: r["summary"].__setitem__("checker_killed", -1))
        corrupt("an edited measurement", lambda r: r["measurement"]["checking_base_total"].__setitem__("lines", 1))
        corrupt("an edited surface", lambda r: r["surface"].__setitem__(CERTIFICATE, {}))
        corrupt("an edited producer residual", lambda r: r["producer_residual"].__setitem__("still_trusted_lines", -1))
        # Each copied build input, and a file outside src/**/*.rs, changed alone.
        for rel in (*BUILD_INPUTS, "crates/continuum-certificate/src/hidden.inc", "crates/continuum-kernel-core/tests/pr9_exit_evidence.rs"):
            changed = tree.with_changes({rel: (tree.read_text(rel) or "") + "\n# changed\n"})
            expect(f"a changed {rel}", validate_record(changed, mutants, evidence, rule_wire_bytes(changed)[1]), "evidence-current")
        # A failed generation leaves the previous record byte-identical.
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR") or None) as scratch:
            path = pathlib.Path(scratch) / "c018.json"
            path.write_bytes(b"previous record\n")
            if write_evidence(evidence, ["simulated failure"], path) or path.read_bytes() != b"previous record\n":
                failures.append("self-test a failed generation: the previous record was replaced")
            if not write_evidence(evidence, [], path) or json.loads(path.read_text()) != evidence:
                failures.append("self-test a passing generation: the record was not written")
            if [p.name for p in pathlib.Path(scratch).iterdir()] != ["c018.json"]:
                failures.append("self-test a generation left a temporary file behind")
    expect("missing evidence", validate_record(tree, mutants, None, surface), "evidence-current")

    # And the real tree is clean under every structural rule, so the fixtures are
    # not firing on a tree that already fails.
    for label, findings in (
        ("links", rule_links(meta)),
        ("wire bytes", rule_wire_bytes(tree)[0]),
        ("tests live", rule_tests_live(tree)),
        ("inventory", rule_inventory(tree, mutants)),
    ):
        if findings:
            failures.append(f"self-test control: the real tree fails {label}: {findings}")
    return failures


# --- main --------------------------------------------------------------------------


def read_evidence(tree: kcov.Tree) -> dict | None:
    data = tree.read_json(EVIDENCE)
    return data if isinstance(data, dict) else None


def main() -> int:
    args = set(sys.argv[1:])
    tree = kcov.Tree(ROOT)
    meta = trip.cargo_metadata(ROOT)
    mutants, load_findings = load_mutants(tree)
    evidence = read_evidence(tree)

    if "--print-inventory" in args:
        print("\n".join(inventory_lines(public_api(tree))))
        return 0

    if "--self-test" in args:
        failures = load_findings + self_test(tree, meta, mutants, evidence)
        print(json.dumps({"self_test": "fail" if failures else "pass", "failures": failures}, indent=2))
        return 1 if failures else 0

    wire_findings, surface = rule_wire_bytes(tree)
    inputs, input_problems = compiler_inputs(ROOT)
    structural = (
        load_findings
        + rule_links(meta)
        + wire_findings
        + rule_compiled_inputs(tree, inputs, input_problems)
        + rule_tests_live(tree)
        + rule_inventory(tree, mutants)
    )

    if "--evidence" in args:
        if structural:
            print(json.dumps({"status": "fail", "violations": structural}, indent=2))
            return 1
        results, control, failures = run_lane(mutants)
        outcomes = {r["id"]: r["outcome"] for r in results}
        record = {
            **stable_record(tree, mutants, surface, inputs),
            **producer_record(tree, mutants, results),
            "control": control,
            "summary": summarize(results),
            "mutants": results,
        }
        mismatches = [
            f"{m['id']}: expected {m['expect']}, got {outcomes.get(m['id'])}"
            for m in mutants
            if outcomes.get(m["id"]) != m["expect"]
        ]
        invariants = validate_record(tree, mutants, record, surface, inputs)
        written = write_evidence(record, failures + mismatches + invariants, ROOT / EVIDENCE)
        report = {
            "status": "pass" if written else "fail",
            "evidence": EVIDENCE if written else f"{EVIDENCE} left unchanged",
            "failures": failures,
            "expectation_mismatches": mismatches,
            "invariant_failures": invariants,
            "summary": record["summary"],
        }
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0 if written else 1

    violations = structural + validate_record(tree, mutants, evidence, surface, inputs)
    report = {
        "status": "fail" if violations else "pass",
        "violations": violations,
        "surface": surface,
        "checking_base_lines": measure(tree)["checking_base_total"],
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if violations else 0


if __name__ == "__main__":
    raise SystemExit(main())
