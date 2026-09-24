#!/usr/bin/env python3
"""Mechanically enforce the kernel covenant over the four `continuum-kernel-*` crates.

Source of truth:

- `notes/plan/docs/03_ASSURANCE_AND_TCB.md` §5 — the certificate-checker
  architecture and the binding code-size covenant: "fewer than 15,000 non-test
  lines across decoder, semantics, and certificate checks";
- `notes/plan/plan.md` §20 — "no shared optimized evaluator code with any engine,
  no async, no unsafe, no plugins or dynamic loading, a <15,000 non-test-line
  covenant (docs/03), and a serialization boundary between every engine and the
  kernel", plus "every receipt records the checker's build digest and toolchain
  identity (INV-014)";
- `notes/plan/notes/START_HERE_IMPLEMENTATION.md` PR 9 — IMPL-03 (line budget),
  IMPL-05 (receipts) and IMPL-06 (mutation tests);
- `notes/plan/schemas/proof-receipt.schema.json` — the receipt shape;
- `tools/kernel-covenant/mutation-matrix.toml` — the mutation class inventory.

Nine requirements, each with violating fixtures that `--self-test` replays:

  KCOV-01 line-budget                  the <15,000 non-test-line count, and the
                                       integrity of the counting method
  KCOV-02 lint-walls-present           the compile-time no-unsafe/no-panic walls
  KCOV-03 kernel-is-synchronous        no async anywhere in shipped kernel code
  KCOV-04 no-unsafe                    no `unsafe`, no `allow(unsafe_code)`
  KCOV-05 no-panic-escape              no unwind constructs, no lint escapes
  KCOV-06 dependency-freedom           the dependency set plan §20 admits
  KCOV-07 no-plugins-or-dynamic-loading no FFI, no dynamic loading, no splicing,
                                       no include_str!/include_bytes!, no `mod` without
                                       its file; also over continuum-certificate
  KCOV-08 mutation-matrix              every class owned or explicitly waived
  KCOV-09 receipt-conformance          receipts the schema admits, epochs named

THE COUNTING METHOD, stated exactly, because two earlier counts disagreed by
eight lines and a covenant needs one authority:

1. **Which files.** Every `*.rs` file under `crates/continuum-kernel-*/src/`,
   recursively. Files under `tests/`, `benches/` and `examples/` are test and
   harness code and are not counted; `build.rs` would be counted if one existed
   (none does).
2. **Test-only modules are excluded whole.** A file declared by a
   `#[cfg(test)] mod NAME;` in a counted file is unreachable outside `cfg(test)`,
   so `NAME.rs` (or `NAME/mod.rs`) is dropped entirely. Today that is the four
   `fixture.rs` producers, 1,504 lines that never ship.
3. **`#[cfg(test)]` items are excluded in place.** Inside a counted file, every
   `#[cfg(test)]` attribute and the item it gates are removed — from the
   attribute's line through the line holding the item's closing brace, or through
   its terminating semicolon for brace-less items such as `mod fixture;`. The
   scanner is comment-, string-, raw-string-, char- and lifetime-aware, so a brace
   inside a doc comment or a string literal cannot move the boundary. (The
   brace-less case is exactly the eight-line class of disagreement: a scanner that
   waits for a closing brace after `#[cfg(test)] mod fixture;` swallows the
   `pub use` block that follows it.)
4. **What counts as a line.** Every physical line that survives 1–3, blank lines
   and comments included. This is the plainest reading of "non-test lines", it
   requires no ruling on what a comment is, and it is conservative: if the plain
   count is under budget then every stricter count is too. The stricter figures
   (non-blank, and non-blank-non-comment) are reported alongside it as context and
   are not enforced.

Stdlib only. Exit 0 when every requirement holds, 1 otherwise.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import tomllib
from collections.abc import Iterable
from dataclasses import dataclass, field as dc_field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# --- what the covenant is about -------------------------------------------------

LINE_BUDGET = 15_000

KERNEL_CRATES: tuple[str, ...] = (
    "continuum-kernel-core",
    "continuum-kernel-sat",
    "continuum-kernel-smt",
    "continuum-kernel-temporal",
)

# Matrix column name -> crate directory.
MATRIX_CRATES: dict[str, str] = {
    "core": "continuum-kernel-core",
    "sat": "continuum-kernel-sat",
    "smt": "continuum-kernel-smt",
    "temporal": "continuum-kernel-temporal",
}

COVENANT_DIR = "tools/kernel-covenant"
MATRIX = f"{COVENANT_DIR}/mutation-matrix.toml"
FIXTURE_DIR = f"{COVENANT_DIR}/fixtures"
RECEIPT_SCHEMA = "notes/plan/schemas/proof-receipt.schema.json"

# plan §20 admits no dependency for the trusted checking base: the four crates
# depend on nothing at all, so their decoder, arithmetic and state representation
# are their own (docs/03 §8 diversity). Widening this set is a deliberate edit
# here plus an ADR, which is the intended friction.
ADMITTED_KERNEL_DEPENDENCIES: frozenset[str] = frozenset()

# The seven lints that make the no-panic covenant a compile error rather than a
# review note, plus the crate-level `forbid`.
REQUIRED_DENY_LINTS: tuple[str, ...] = (
    "clippy::indexing_slicing",
    "clippy::unwrap_used",
    "clippy::expect_used",
    "clippy::panic",
    "clippy::todo",
    "clippy::unimplemented",
    "clippy::arithmetic_side_effects",
)

# Unwind constructs. `\.unwrap\(` deliberately does not match `unwrap_or(`, which
# is the total form the shipped decoder uses.
PANIC_PATTERNS: tuple[tuple[str, str], ...] = (
    (r"\.unwrap\s*\(", ".unwrap()"),
    (r"\.expect\s*\(", ".expect(…)"),
    (r"\bpanic!", "panic!"),
    (r"\btodo!", "todo!"),
    (r"\bunimplemented!", "unimplemented!"),
    (r"\bunreachable!", "unreachable!"),
    (r"\bassert(_eq|_ne)?!", "assert!"),
    (r"\bdebug_assert(_eq|_ne)?!", "debug_assert!"),
)

ASYNC_PATTERNS: tuple[tuple[str, str], ...] = (
    (r"\basync\b", "async"),
    (r"\.await\b", ".await"),
    (r"\bFuture\b", "Future"),
    (r"\bPoll\b", "Poll"),
)

DYNAMIC_LOADING_PATTERNS: tuple[tuple[str, str], ...] = (
    (r"\blibloading\b", "libloading"),
    (r"\bdlopen\b", "dlopen"),
    (r"\bdlsym\b", "dlsym"),
    (r"\bextern\s+\"", 'extern "…"'),
    (r"#\s*\[\s*no_mangle", "#[no_mangle]"),
    (r"#\s*\[\s*unsafe\b", "#[unsafe(…)]"),
    (r"\binclude!\s*\(", "include!(…)"),
    (r"#\s*\[\s*path\s*=", "#[path = …]"),
    (r"\binclude_str!\s*\(", "include_str!(…)"),
    (r"\binclude_bytes!\s*\(", "include_bytes!(…)"),
)

# KCOV-07 alone also covers `continuum-certificate`, the composition above the four
# kernels: a `#[path]` module or an `include!` there hides checking code from the line
# count, the C018 public-API inventory and its digest just as it would in a kernel
# (Codex cr-10mg1y round 4). The other requirements keep their kernel-only scope.
SPLICING_SCOPE: tuple[str, ...] = (*KERNEL_CRATES, "continuum-certificate")


class Violation(str):
    """A single requirement failure, rendered as a human-readable line."""


# ============================================================================
# A read-only repository view, so fixtures run through the real code path
# ============================================================================


INDEXED_ROOTS = ("crates", "tools", "notes")
PRUNED_DIRS = {"target", ".git", ".maw", "node_modules", "__pycache__"}


class Tree:
    """The repository, optionally with fixture overlays and removals."""

    def __init__(
        self,
        root: Path,
        overlay: dict[str, str] | None = None,
        removed: Iterable[str] = (),
    ) -> None:
        self.root = root
        self.overlay = dict(overlay or {})
        self.removed = frozenset(removed)
        self._index: tuple[str, ...] | None = None

    def with_changes(self, overlay: dict[str, str], removed: Iterable[str] = ()) -> Tree:
        return Tree(self.root, {**self.overlay, **overlay}, self.removed | frozenset(removed))

    def _real_index(self) -> tuple[str, ...]:
        if self._index is None:
            found: list[str] = []
            for top in INDEXED_ROOTS:
                base = self.root / top
                if not base.is_dir():
                    continue
                for dirpath, dirnames, filenames in os.walk(base):
                    dirnames[:] = sorted(d for d in dirnames if d not in PRUNED_DIRS)
                    rel_dir = Path(dirpath).relative_to(self.root).as_posix()
                    for fname in sorted(filenames):
                        found.append(f"{rel_dir}/{fname}")
            self._index = tuple(sorted(found))
        return self._index

    def exists(self, rel: str) -> bool:
        if rel in self.removed:
            return False
        if rel in self.overlay:
            return True
        return (self.root / rel).is_file()

    def read_text(self, rel: str) -> str | None:
        if rel in self.removed:
            return None
        if rel in self.overlay:
            return self.overlay[rel]
        path = self.root / rel
        if not path.is_file():
            return None
        return path.read_text(encoding="utf-8")

    def read_json(self, rel: str) -> object | None:
        text = self.read_text(rel)
        if text is None:
            return None
        try:
            return json.loads(text)
        except json.JSONDecodeError:
            return None

    def read_toml(self, rel: str) -> dict | None:
        text = self.read_text(rel)
        if text is None:
            return None
        try:
            return tomllib.loads(text)
        except tomllib.TOMLDecodeError:
            return None

    def glob(self, pattern: str) -> list[str]:
        matcher = _glob_re(pattern)
        candidates = set(self._real_index()) | set(self.overlay)
        return sorted(p for p in candidates if p not in self.removed and matcher.fullmatch(p))


def _glob_re(pattern: str) -> re.Pattern[str]:
    """Translate a posix glob (`*`, `**`) into a full-match regex."""
    out: list[str] = []
    i = 0
    while i < len(pattern):
        if pattern.startswith("**/", i):
            out.append("(?:.*/)?")
            i += 3
        elif pattern[i] == "*":
            out.append("[^/]*")
            i += 1
        else:
            out.append(re.escape(pattern[i]))
            i += 1
    return re.compile("".join(out))


# ============================================================================
# The Rust scanner: comments, strings, and `#[cfg(test)]` regions
# ============================================================================

_RAW_STRING = re.compile(r'r(#*)"')
_CHAR_LITERAL = re.compile(r"'(\\.[^']*|[^\\'])'")
CFG_TEST = "#[cfg(test)]"


@dataclass(frozen=True)
class Scan:
    """One Rust file, split into what ships and what only tests."""

    lines: tuple[str, ...]
    #: 1-based inclusive line spans gated by `#[cfg(test)]`.
    test_spans: tuple[tuple[int, int], ...]
    #: Module names declared `#[cfg(test)] mod NAME;` — whole test-only files.
    test_modules: tuple[str, ...]
    #: The file with comment and literal *contents* blanked, newlines preserved.
    code: str

    def shipped_lines(self) -> list[int]:
        excluded: set[int] = set()
        for start, end in self.test_spans:
            excluded.update(range(start, end + 1))
        return [n for n in range(1, len(self.lines) + 1) if n not in excluded]

    def shipped_code(self) -> str:
        """The blanked code view with every `#[cfg(test)]` region blanked too."""
        excluded: set[int] = set()
        for start, end in self.test_spans:
            excluded.update(range(start, end + 1))
        out: list[str] = []
        for number, line in enumerate(self.code.splitlines(), start=1):
            out.append("" if number in excluded else line)
        return "\n".join(out)


def scan_rust(text: str) -> Scan:
    """Lex enough Rust to find `#[cfg(test)]` items and blank out comments/strings.

    Not a parser. It tracks brace depth outside comments, string literals, raw
    strings and char literals, distinguishing a char literal from a lifetime, which
    is all the line budget and the textual rules need. See the module docstring for
    why the brace-less `#[cfg(test)] mod NAME;` case matters.
    """
    n = len(text)
    line_of: list[int] = []
    line = 1
    for ch in text:
        line_of.append(line)
        if ch == "\n":
            line += 1
    line_of.append(line)

    code = list(text)

    def blank(start: int, stop: int) -> None:
        for k in range(start, min(stop, n)):
            if code[k] != "\n":
                code[k] = " "

    spans: list[tuple[int, int]] = []
    modules: list[str] = []
    pending: tuple[int, int] | None = None
    depth = 0
    i = 0
    while i < n:
        ch = text[i]
        pair = text[i : i + 2]
        if pair == "//":
            end = text.find("\n", i)
            end = n if end < 0 else end
            blank(i, end)
            i = end
            continue
        if pair == "/*":
            level, j = 1, i + 2
            while j < n and level:
                if text[j : j + 2] == "/*":
                    level += 1
                    j += 2
                elif text[j : j + 2] == "*/":
                    level -= 1
                    j += 2
                else:
                    j += 1
            blank(i, j)
            i = j
            continue
        raw = _RAW_STRING.match(text, i)
        if raw:
            close = '"' + raw.group(1)
            j = text.find(close, raw.end())
            j = n if j < 0 else j + len(close)
            blank(i, j)
            i = j
            continue
        if ch == '"':
            j = i + 1
            while j < n:
                if text[j] == "\\":
                    j += 2
                    continue
                if text[j] == '"':
                    j += 1
                    break
                j += 1
            blank(i, j)
            i = j
            continue
        if ch == "'":
            literal = _CHAR_LITERAL.match(text, i)
            if literal:
                blank(i, literal.end())
                i = literal.end()
            else:  # a lifetime, not a literal
                i += 1
            continue
        if text.startswith(CFG_TEST, i):
            if pending is None:
                pending = (i, depth)
            i += len(CFG_TEST)
            continue
        if ch == ";":
            i += 1
            if pending is not None and depth == pending[1]:
                item = text[pending[0] : i]
                declared = re.search(r"\bmod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;", item)
                if declared:
                    modules.append(declared.group(1))
                spans.append((line_of[pending[0]], line_of[i - 1]))
                pending = None
            continue
        if ch == "{":
            depth += 1
            i += 1
            continue
        if ch == "}":
            depth -= 1
            i += 1
            if pending is not None and depth == pending[1]:
                spans.append((line_of[pending[0]], line_of[i - 1]))
                pending = None
            continue
        i += 1

    return Scan(
        lines=tuple(text.splitlines()),
        test_spans=tuple(spans),
        test_modules=tuple(modules),
        code="".join(code),
    )


# ============================================================================
# The line budget
# ============================================================================


@dataclass
class CrateCount:
    crate: str
    counted: dict[str, int] = dc_field(default_factory=dict)
    excluded_files: list[str] = dc_field(default_factory=list)
    non_blank: int = 0
    non_blank_non_comment: int = 0

    @property
    def total(self) -> int:
        return sum(self.counted.values())


def count_crate(tree: Tree, crate: str) -> CrateCount:
    """Apply the four counting rules of the module docstring to one crate."""
    result = CrateCount(crate=crate)
    sources = tree.glob(f"crates/{crate}/src/**/*.rs")
    scans = {rel: scan_rust(tree.read_text(rel) or "") for rel in sources}
    test_only = test_only_files(scans)

    for rel in sorted(scans):
        scan = scans[rel]
        if rel in test_only:
            result.excluded_files.append(rel)
            continue
        kept = scan.shipped_lines()
        result.counted[rel] = len(kept)
        for number in kept:
            text = scan.lines[number - 1].strip()
            if not text:
                continue
            result.non_blank += 1
            if not text.startswith(("//", "/*", "*")):
                result.non_blank_non_comment += 1
    return result


def rule_line_budget(tree: Tree, budget: int = LINE_BUDGET) -> list[Violation]:
    violations: list[Violation] = []
    counts = [count_crate(tree, crate) for crate in KERNEL_CRATES]
    total = sum(c.total for c in counts)
    if total >= budget:
        breakdown = ", ".join(f"{c.crate}={c.total}" for c in counts)
        violations.append(
            Violation(
                f"[line-budget] the trusted checking base is {total} non-test lines, "
                f"budget is fewer than {budget} (docs/03 §5; plan §20) — {breakdown}"
            )
        )

    # The counting method's own integrity: shipped test code would otherwise be
    # invisible to rule 3 and would ship inside the trusted base.
    for rel, scan in kernel_sources(tree):
        gated: set[int] = set()
        for start, end in scan.test_spans:
            gated.update(range(start, end + 1))
        for number, line in enumerate(scan.shipped_code().splitlines(), start=1):
            if re.search(r"\bmod\s+tests\b", line) and number not in gated:
                violations.append(
                    Violation(
                        f"[line-budget] {rel}:{number} declares a `tests` module that is not "
                        "`#[cfg(test)]`-gated: it would ship inside the trusted base and would "
                        "count against the covenant"
                    )
                )
    return violations


# ============================================================================
# The compile-time walls
# ============================================================================


def test_only_files(scans: dict[str, Scan]) -> set[str]:
    """Files declared `#[cfg(test)] mod NAME;` — unreachable outside `cfg(test)`."""
    found: set[str] = set()
    for rel, scan in scans.items():
        base = rel.rsplit("/", 1)[0]
        for name in scan.test_modules:
            for candidate in (f"{base}/{name}.rs", f"{base}/{name}/mod.rs"):
                if candidate in scans:
                    found.add(candidate)
    return found


def kernel_sources(tree: Tree) -> list[tuple[str, Scan]]:
    """Every shipped kernel source: `src/**/*.rs` minus the test-only modules.

    The same exclusion the line budget applies, for the same reason — a
    `#[cfg(test)] mod fixture;` producer is a test harness, and its harness is
    allowed to panic.
    """
    out: list[tuple[str, Scan]] = []
    for crate in KERNEL_CRATES:
        sources = tree.glob(f"crates/{crate}/src/**/*.rs")
        scans = {rel: scan_rust(tree.read_text(rel) or "") for rel in sources}
        excluded = test_only_files(scans)
        for rel in sorted(scans):
            if rel not in excluded:
                out.append((rel, scans[rel]))
    return out


def rule_lint_walls(tree: Tree) -> list[Violation]:
    """The walls are the enforcement; this proves they have not been removed."""
    violations: list[Violation] = []
    for crate in KERNEL_CRATES:
        rel = f"crates/{crate}/src/lib.rs"
        text = tree.read_text(rel)
        if text is None:
            violations.append(Violation(f"[lint-walls-present] {rel} is missing"))
            continue
        code = scan_rust(text).code
        if "#![forbid(unsafe_code)]" not in code.replace(" ", ""):
            violations.append(
                Violation(
                    f"[lint-walls-present] {rel} no longer carries `#![forbid(unsafe_code)]` "
                    "(docs/03 §5; plan §20 'no unsafe')"
                )
            )
        deny = re.search(r"#!\[deny\((.*?)\)\]", code, re.DOTALL)
        declared = deny.group(1) if deny else ""
        for lint in REQUIRED_DENY_LINTS:
            if lint not in declared:
                violations.append(
                    Violation(
                        f"[lint-walls-present] {rel} no longer denies `{lint}`: the no-panic "
                        "covenant would become a review note instead of a compile error "
                        "(docs/12 §11 release-blocker classes; docs/16 PO-KER-001)"
                    )
                )
        manifest = tree.read_toml(f"crates/{crate}/Cargo.toml") or {}
        if not (manifest.get("lints") or {}).get("workspace"):
            violations.append(
                Violation(
                    f"[lint-walls-present] crates/{crate}/Cargo.toml does not opt into the "
                    "workspace lint table, so `unsafe_code = \"forbid\"` no longer applies"
                )
            )
    return violations


def _pattern_rule(tree: Tree, rule: str, patterns: Iterable[tuple[str, str]], why: str) -> list[Violation]:
    violations: list[Violation] = []
    for rel, scan in kernel_sources(tree):
        for number, line in enumerate(scan.shipped_code().splitlines(), start=1):
            for pattern, label in patterns:
                if re.search(pattern, line):
                    violations.append(Violation(f"[{rule}] {rel}:{number} uses `{label}` — {why}"))
    return violations


def rule_no_async(tree: Tree) -> list[Violation]:
    return _pattern_rule(
        tree,
        "kernel-is-synchronous",
        ASYNC_PATTERNS,
        "the trusted checking base is single-threaded and synchronous by covenant "
        "(plan §20; docs/03 §5)",
    )


def rule_no_unsafe(tree: Tree) -> list[Violation]:
    violations = _pattern_rule(
        tree,
        "no-unsafe",
        ((r"\bunsafe\b", "unsafe"),),
        "plan §20 forbids unsafe in the kernel and the workspace forbids it outright",
    )
    for rel, scan in kernel_sources(tree):
        for number, line in enumerate(scan.shipped_code().splitlines(), start=1):
            if re.search(r"allow\s*\(\s*unsafe_code", line):
                violations.append(
                    Violation(
                        f"[no-unsafe] {rel}:{number} re-allows `unsafe_code`, defeating the "
                        "workspace `forbid`"
                    )
                )
    return violations


def rule_no_panic_escape(tree: Tree) -> list[Violation]:
    violations = _pattern_rule(
        tree,
        "no-panic-escape",
        PANIC_PATTERNS,
        "malformed input has to be a value on every path (docs/12 §11: a malformed-artifact "
        "panic in the kernel is a release blocker)",
    )
    for rel, scan in kernel_sources(tree):
        code = scan.shipped_code()
        for number, line in enumerate(code.splitlines(), start=1):
            allow = re.search(r"allow\s*\(([^)]*)", line)
            if not allow:
                continue
            for lint in REQUIRED_DENY_LINTS:
                if lint in allow.group(1):
                    violations.append(
                        Violation(
                            f"[no-panic-escape] {rel}:{number} allows `{lint}` in shipped code; "
                            "only `#[cfg(test)]` modules may opt back out"
                        )
                    )
    return violations


def rule_dependency_freedom(tree: Tree) -> list[Violation]:
    violations: list[Violation] = []
    for crate in KERNEL_CRATES:
        rel = f"crates/{crate}/Cargo.toml"
        manifest = tree.read_toml(rel)
        if manifest is None:
            violations.append(Violation(f"[dependency-freedom] {rel} is missing or is not TOML"))
            continue
        for section in ("dependencies", "build-dependencies"):
            for name in sorted(manifest.get(section, {})):
                if name not in ADMITTED_KERNEL_DEPENDENCIES:
                    violations.append(
                        Violation(
                            f"[dependency-freedom] {rel} declares [{section}] {name}, which "
                            "plan §20 does not admit for the trusted checking base — the four "
                            "kernel crates depend on nothing, so their decoder, arithmetic and "
                            "state representation are their own (docs/03 §8)"
                        )
                    )
        for target in sorted(manifest.get("target", {})):
            block = manifest["target"][target]
            for section in ("dependencies", "build-dependencies"):
                for name in sorted(block.get(section, {})):
                    violations.append(
                        Violation(
                            f"[dependency-freedom] {rel} declares a target-conditional "
                            f"[{section}] {name} under `{target}`; the covenant admits none"
                        )
                    )
    return violations


def scoped_sources(tree: Tree, crates: Iterable[str]) -> list[tuple[str, Scan]]:
    """`kernel_sources`, over any list of crates."""
    out: list[tuple[str, Scan]] = []
    for crate in crates:
        sources = tree.glob(f"crates/{crate}/src/**/*.rs")
        scans = {rel: scan_rust(tree.read_text(rel) or "") for rel in sources}
        excluded = test_only_files(scans)
        out.extend((rel, scans[rel]) for rel in sorted(scans) if rel not in excluded)
    return out


def module_file_candidates(rel: str, name: str) -> tuple[str, str]:
    """Where rustc looks for `mod name;` declared in `rel`, without `#[path]`."""
    base, file = rel.rsplit("/", 1)
    if file not in ("lib.rs", "main.rs", "mod.rs"):
        base = f"{base}/{file[:-3]}"
    return f"{base}/{name}.rs", f"{base}/{name}/mod.rs"


def rule_no_dynamic_loading(tree: Tree) -> list[Violation]:
    rule = "no-plugins-or-dynamic-loading"
    why = (
        "plan §20 forbids plugins and dynamic loading in the kernel, and a spliced or "
        "redirected source file would also defeat the line count"
    )
    violations: list[Violation] = []
    for rel, scan in scoped_sources(tree, SPLICING_SCOPE):
        for number, line in enumerate(scan.shipped_code().splitlines(), start=1):
            for pattern, label in DYNAMIC_LOADING_PATTERNS:
                if re.search(pattern, line):
                    violations.append(Violation(f"[{rule}] {rel}:{number} uses `{label}` — {why}"))
            for declared in re.finditer(r"\bmod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;", line):
                if not any(tree.exists(c) for c in module_file_candidates(rel, declared.group(1))):
                    violations.append(
                        Violation(
                            f"[{rule}] {rel}:{number} declares `mod {declared.group(1)};` with no file "
                            f"at {' or '.join(module_file_candidates(rel, declared.group(1)))} — {why}"
                        )
                    )
        # A `cfg_attr` that can expand to `path` redirects a module as surely as
        # `#[path]` does, over any number of lines (Codex cr-3i3rst). Banned outright.
        code = scan.shipped_code()
        for attr in re.finditer(r"#\s*!?\[\s*cfg_attr\b[^\]]*?\bpath\b", code):
            number = code.count("\n", 0, attr.start()) + 1
            violations.append(Violation(f"[{rule}] {rel}:{number} uses `cfg_attr(…, path …)` — {why}"))
    return violations


# ============================================================================
# KCOV-08 — the mutation matrix
# ============================================================================


def rust_tests(tree: Tree, crate: str) -> dict[str, str]:
    """Every `#[test]` function of a crate, mapped to the file that defines it."""
    found: dict[str, str] = {}
    patterns = (f"crates/{crate}/src/**/*.rs", f"crates/{crate}/tests/**/*.rs")
    for pattern in patterns:
        for rel in tree.glob(pattern):
            code = scan_rust(tree.read_text(rel) or "").code
            for match in re.finditer(r"#\[test\]\s*(?:#\[[^\]]*\]\s*)*fn\s+([A-Za-z_][A-Za-z0-9_]*)", code):
                found.setdefault(match.group(1), rel)
    return found


@dataclass(frozen=True)
class MatrixReport:
    classes: tuple[str, ...]
    owned: dict[str, dict[str, list[str]]]
    uncovered: tuple[tuple[str, str], ...]
    not_applicable: tuple[tuple[str, str], ...]


def read_matrix(tree: Tree) -> tuple[dict | None, list[Violation]]:
    data = tree.read_toml(MATRIX)
    if data is None:
        return None, [Violation(f"[mutation-matrix] {MATRIX} is missing or is not TOML")]
    return data, []


def rule_mutation_matrix(tree: Tree) -> list[Violation]:
    data, violations = read_matrix(tree)
    if data is None:
        return violations

    columns = list(data.get("crates", []))
    if sorted(columns) != sorted(MATRIX_CRATES):
        violations.append(
            Violation(
                f"[mutation-matrix] the matrix covers {columns}, but the kernel is "
                f"{sorted(MATRIX_CRATES)}"
            )
        )
    tests = {column: rust_tests(tree, crate) for column, crate in MATRIX_CRATES.items()}

    classes = data.get("class", [])
    if not classes:
        return [*violations, Violation("[mutation-matrix] the matrix declares no mutation classes")]

    seen: set[str] = set()
    owned: dict[str, dict[str, list[str]]] = {}
    for entry in classes:
        cid = entry.get("id")
        if not cid:
            violations.append(Violation("[mutation-matrix] a class has no `id`"))
            continue
        if cid in seen:
            violations.append(Violation(f"[mutation-matrix] class `{cid}` is declared twice"))
        seen.add(cid)
        for required in ("mutation", "expected"):
            if not entry.get(required):
                violations.append(
                    Violation(f"[mutation-matrix] class `{cid}` does not state its `{required}`")
                )
        owned[cid] = {}
        for column in MATRIX_CRATES:
            names = entry.get(column, [])
            if not isinstance(names, list):
                violations.append(Violation(f"[mutation-matrix] {cid}.{column} is not a list"))
                continue
            owned[cid][column] = list(names)
            for name in names:
                if name not in tests.get(column, {}):
                    violations.append(
                        Violation(
                            f"[mutation-matrix] {cid} names `{name}` for `{column}`, but "
                            f"{MATRIX_CRATES[column]} has no `#[test] fn {name}` — a renamed or "
                            "deleted test leaves a class unowned"
                        )
                    )

    waived: dict[tuple[str, str], dict] = {}
    for waiver in data.get("waiver", []):
        cid, column = waiver.get("class"), waiver.get("crate")
        if cid not in seen:
            violations.append(Violation(f"[mutation-matrix] waiver names unknown class `{cid}`"))
            continue
        if column not in MATRIX_CRATES:
            violations.append(Violation(f"[mutation-matrix] waiver for `{cid}` names unknown crate `{column}`"))
            continue
        if waiver.get("kind") not in ("not-applicable", "uncovered"):
            violations.append(
                Violation(
                    f"[mutation-matrix] waiver {cid}/{column} has kind "
                    f"{waiver.get('kind')!r}; the vocabulary is not-applicable | uncovered"
                )
            )
        if len((waiver.get("reason") or "").strip()) < 40:
            violations.append(
                Violation(
                    f"[mutation-matrix] waiver {cid}/{column} has no substantive `reason`; a "
                    "waiver without a stated reason is an unrecorded gap"
                )
            )
        if (cid, column) in waived:
            violations.append(Violation(f"[mutation-matrix] {cid}/{column} is waived twice"))
        waived[(cid, column)] = waiver

    for cid in sorted(seen):
        for column in sorted(MATRIX_CRATES):
            has_tests = bool(owned.get(cid, {}).get(column))
            waiver = waived.get((cid, column))
            if has_tests and waiver:
                violations.append(
                    Violation(
                        f"[mutation-matrix] {cid}/{column} is both owned and waived — remove the "
                        "waiver now that a test covers the class"
                    )
                )
            if not has_tests and not waiver:
                violations.append(
                    Violation(
                        f"[mutation-matrix] {cid}/{column} has no owning test and no waiver "
                        "(fail-closed: state a `[[waiver]]` with a reason, or name the test)"
                    )
                )
    return violations


def matrix_report(tree: Tree) -> MatrixReport | None:
    data, problems = read_matrix(tree)
    if data is None or problems:
        return None
    owned: dict[str, dict[str, list[str]]] = {}
    for entry in data.get("class", []):
        owned[entry["id"]] = {c: list(entry.get(c, [])) for c in MATRIX_CRATES}
    uncovered = tuple(
        (w["class"], w["crate"]) for w in data.get("waiver", []) if w.get("kind") == "uncovered"
    )
    not_applicable = tuple(
        (w["class"], w["crate"]) for w in data.get("waiver", []) if w.get("kind") == "not-applicable"
    )
    return MatrixReport(tuple(sorted(owned)), owned, uncovered, not_applicable)


# ============================================================================
# KCOV-09 — receipts the schema admits, naming the epoch they mean
# ============================================================================


def json_type(value: object) -> str:
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, str):
        return "string"
    if isinstance(value, int):
        return "integer"
    if isinstance(value, float):
        return "number"
    if isinstance(value, list):
        return "array"
    if isinstance(value, dict):
        return "object"
    return "null"


def validate(schema: dict, instance: object, path: str = "$") -> list[str]:
    """A JSON Schema subset: the keywords `proof-receipt.schema.json` uses on the
    fragments a kernel receipt exercises — type, const, enum, pattern, required,
    properties, additionalProperties, items, minItems, allOf, if/then.

    Deliberately not a general validator, and it says so: `$ref` and the
    redaction `$defs` are outside what a kernel-emitted receipt contains, so a
    receipt that reached them would be reported rather than silently passed.
    """
    errors: list[str] = []
    if "$ref" in schema:
        return [f"{path}: this checker does not resolve $ref ({schema['$ref']})"]
    if "const" in schema and instance != schema["const"]:
        errors.append(f"{path}: {instance!r} is not the required const {schema['const']!r}")
    if "enum" in schema and instance not in schema["enum"]:
        errors.append(f"{path}: {instance!r} is not one of {schema['enum']}")
    expected = schema.get("type")
    if expected:
        actual = json_type(instance)
        allowed = {expected} if isinstance(expected, str) else set(expected)
        if actual == "integer" and "number" in allowed:
            allowed.add("integer")
        if actual not in allowed:
            errors.append(f"{path}: expected type {expected}, found {actual}")
            return errors
    if "pattern" in schema and isinstance(instance, str):
        if not re.search(schema["pattern"], instance):
            errors.append(f"{path}: {instance!r} does not match {schema['pattern']}")
    if isinstance(instance, dict):
        for name in schema.get("required", []):
            if name not in instance:
                errors.append(f"{path}: required property {name!r} is missing")
        properties = schema.get("properties", {})
        if schema.get("additionalProperties") is False:
            for name in sorted(instance):
                if name not in properties:
                    errors.append(f"{path}: property {name!r} is not admitted by the schema")
        for name, value in sorted(instance.items()):
            if name in properties:
                errors.extend(validate(properties[name], value, f"{path}.{name}"))
    if isinstance(instance, list):
        if "minItems" in schema and len(instance) < schema["minItems"]:
            errors.append(f"{path}: needs at least {schema['minItems']} items, has {len(instance)}")
        item_schema = schema.get("items")
        if isinstance(item_schema, dict):
            for index, value in enumerate(instance):
                errors.extend(validate(item_schema, value, f"{path}[{index}]"))
    for sub in schema.get("allOf", []):
        if "if" in sub:
            if not validate(sub["if"], instance, path):
                errors.extend(validate(sub.get("then", {}), instance, path))
            continue
        errors.extend(validate(sub, instance, path))
    return errors


def wire_constants(tree: Tree, crate: str) -> tuple[str | None, str | None]:
    text = tree.read_text(f"crates/{crate}/src/wire.rs") or ""
    magic = re.search(r'pub const MAGIC:\s*\[u8;\s*8\]\s*=\s*\*b"([^"]+)"', text)
    epoch = re.search(r"pub const WIRE_EPOCH:\s*u16\s*=\s*(\d+)", text)
    return (magic.group(1) if magic else None, epoch.group(1) if epoch else None)


def rule_receipt_conformance(tree: Tree) -> list[Violation]:
    violations: list[Violation] = []
    schema = tree.read_json(RECEIPT_SCHEMA)
    if not isinstance(schema, dict):
        return [Violation(f"[receipt-conformance] {RECEIPT_SCHEMA} is missing or is not JSON")]

    magics: dict[str, str] = {}
    for crate in KERNEL_CRATES:
        magic, epoch = wire_constants(tree, crate)
        if magic is None or epoch is None:
            violations.append(
                Violation(f"[receipt-conformance] crates/{crate}/src/wire.rs declares no MAGIC/WIRE_EPOCH")
            )
            continue
        if magic in magics:
            violations.append(
                Violation(
                    f"[receipt-conformance] {crate} and {magics[magic]} share the magic "
                    f"`{magic}`: their wire epochs would be indistinguishable in a receipt "
                    "(INV-014; RFC 0026 correction 17)"
                )
            )
        magics[magic] = crate

        receipt_rel = f"crates/{crate}/src/receipt.rs"
        receipt_src = tree.read_text(receipt_rel)
        if receipt_src is None:
            violations.append(
                Violation(
                    f"[receipt-conformance] {receipt_rel} is missing: the crate cannot record "
                    "checker identity, build digest or input hashes (INV-014)"
                )
            )
            continue
        expected_id = f"{crate}/{magic}/{epoch}"
        declared = re.search(r'pub const WIRE_EPOCH_ID:\s*&str\s*=\s*"([^"]+)"', receipt_src)
        if declared is None:
            violations.append(
                Violation(f"[receipt-conformance] {receipt_rel} declares no `WIRE_EPOCH_ID`")
            )
        elif declared.group(1) != expected_id:
            violations.append(
                Violation(
                    f"[receipt-conformance] {receipt_rel} names the wire epoch "
                    f"`{declared.group(1)}` but the crate's constants say `{expected_id}` — "
                    "four kernel crates share the ordinal 1, so the qualified spelling is the "
                    "only thing that identifies which contract a receipt means"
                )
            )

        golden_rel = f"crates/{crate}/tests/receipt.golden.json"
        golden = tree.read_json(golden_rel)
        if not isinstance(golden, dict):
            violations.append(
                Violation(f"[receipt-conformance] {golden_rel} is missing or is not a JSON object")
            )
            continue
        for error in validate(schema, golden, "$"):
            violations.append(
                Violation(f"[receipt-conformance] {golden_rel} is not a proof receipt: {error}")
            )
        checker = golden.get("checker", {})
        if checker.get("name") != crate:
            violations.append(
                Violation(
                    f"[receipt-conformance] {golden_rel} names checker "
                    f"{checker.get('name')!r}, not `{crate}`"
                )
            )
        marker = f"+wire.{magic}.{epoch}"
        if marker not in str(checker.get("version", "")):
            violations.append(
                Violation(
                    f"[receipt-conformance] {golden_rel} checker.version "
                    f"{checker.get('version')!r} does not carry `{marker}`: checker identity is "
                    "where a native checker's wire contract belongs (RFC 0026 correction 17)"
                )
            )
        epochs = golden.get("epochs", {})
        extra = sorted(set(epochs) - {"semantic", "proof", "corpus"})
        if extra:
            violations.append(
                Violation(
                    f"[receipt-conformance] {golden_rel} writes {extra} into `epochs`; there is "
                    "no checker epoch (RFC 0026 correction 17)"
                )
            )
        for key, value in sorted(epochs.items()):
            if magic in str(value):
                violations.append(
                    Violation(
                        f"[receipt-conformance] {golden_rel} writes the wire magic into "
                        f"epochs.{key}; the wire contract is checker identity, not an epoch"
                    )
                )
    return violations


# ============================================================================
# Requirements
# ============================================================================


@dataclass(frozen=True)
class Requirement:
    rid: str
    rule: str
    summary: str
    source: str
    fn: object
    boundary: str


REQUIREMENTS: tuple[Requirement, ...] = (
    Requirement(
        "KCOV-01",
        "line-budget",
        "Fewer than 15,000 non-test lines across the four kernel crates.",
        "notes/plan/docs/03_ASSURANCE_AND_TCB.md §5; plan §20; START_HERE PR 9 IMPL-03",
        rule_line_budget,
        "EXACT for the stated method, which the module docstring fixes. The count is a "
        "forcing function, not a proof of trustworthiness — docs/03 says so outright.",
    ),
    Requirement(
        "KCOV-02",
        "lint-walls-present",
        "Every kernel crate still carries the forbid/deny walls that make the covenant a compile error.",
        "docs/03 §5; docs/12 §11; docs/16 PO-KER-001",
        rule_lint_walls,
        "PROXY for the compile-time enforcement, which is `cargo clippy -D warnings` in "
        "`just lint`. This rule proves the walls have not been deleted; clippy proves they hold.",
    ),
    Requirement(
        "KCOV-03",
        "kernel-is-synchronous",
        "No async, await, or future machinery in shipped kernel code.",
        "plan §20 ('no async'); docs/03 §5 ('single-threaded')",
        rule_no_async,
        "Textual over the comment- and string-blanked source, so prose about async does not "
        "trip it. Declared external async runtimes are `tools/check_crate_boundaries.py`.",
    ),
    Requirement(
        "KCOV-04",
        "no-unsafe",
        "No `unsafe`, and no re-allowing of `unsafe_code`, in shipped kernel code.",
        "plan §20 ('no unsafe'); workspace `unsafe_code = \"forbid\"`",
        rule_no_unsafe,
        "Textual. `forbid` already makes this a compile error; the rule catches an attempt to "
        "weaken the attribute in the same change that adds the `unsafe`.",
    ),
    Requirement(
        "KCOV-05",
        "no-panic-escape",
        "No unwind construct in shipped kernel code, and no local escape from the denied lints.",
        "docs/12 §11 (malformed artifact panic in kernel = release blocker); docs/16 PO-KER-001",
        rule_no_panic_escape,
        "Textual, and wider than the lints: `assert!` is not covered by `clippy::panic` but "
        "still turns bad data into an unwind. Test modules are excluded, because their harness "
        "is allowed to panic.",
    ),
    Requirement(
        "KCOV-06",
        "dependency-freedom",
        "The kernel crates declare only the dependencies plan §20 admits — today, none.",
        "plan §20 dependency rules; docs/03 §8 (implementation diversity)",
        rule_dependency_freedom,
        "Manifest-level and exact. Transitive third-party auditing is not needed while the "
        "admitted set is empty; widening it is an edit to this file plus an ADR.",
    ),
    Requirement(
        "KCOV-07",
        "no-plugins-or-dynamic-loading",
        "No FFI, dynamic loading, source splicing, or module-path redirection in the kernel.",
        "plan §20 ('no plugins or dynamic loading')",
        rule_no_dynamic_loading,
        "Textual. `include!` and `#[path]` are here as much for the line budget as for the "
        "covenant: both move code into a crate from somewhere the counter does not look.",
    ),
    Requirement(
        "KCOV-08",
        "mutation-matrix",
        "Every mutation class is owned by a named, existing test in every kernel crate, or explicitly waived.",
        "START_HERE PR 9 exit; docs/19 §4 (mutation testing)",
        rule_mutation_matrix,
        "INDEX, not execution: the sweeps themselves run under `cargo test`. What this proves "
        "is that the map from mutation class to owning test is complete and not stale — a "
        "class nobody owns is a check that might not be load-bearing.",
    ),
    Requirement(
        "KCOV-09",
        "receipt-conformance",
        "Kernel receipts validate against proof-receipt.schema.json and name which crate's wire epoch they mean.",
        "INV-014; RFC 0024; RFC 0026 correction 17; schemas/proof-receipt.schema.json",
        rule_receipt_conformance,
        "The golden receipts are produced by the crates' own `to_json` and asserted byte-for-byte "
        "in their unit tests, so validating the goldens here validates the emitters. The schema "
        "subset covers exactly the keywords a kernel receipt exercises and refuses to guess at "
        "the rest.",
    ),
)

RULES = {req.rule: req for req in REQUIREMENTS}


def run_requirements(tree: Tree) -> dict[str, list[Violation]]:
    return {req.rid: list(req.fn(tree)) for req in REQUIREMENTS}


# ============================================================================
# Fixtures
# ============================================================================


@dataclass(frozen=True)
class Fixture:
    fid: str
    requirement: str
    rule: str
    description: str
    overlay: dict[str, str]
    removed: tuple[str, ...]
    budget: int | None


def load_fixtures(tree: Tree) -> tuple[list[Fixture], list[str]]:
    """Read every fixture description. Fixtures are inert data, never compiled.

    A fixture is a one-anchor `substitute` into a real file wherever possible, so
    that adding a case costs a few lines rather than a copy of a 1,000-line source
    file, and so that a fixture goes stale loudly when its anchor moves.
    """
    fixtures: list[Fixture] = []
    problems: list[str] = []
    for rel in sorted(tree.glob(f"{FIXTURE_DIR}/*/fixture.json")):
        directory = rel.rsplit("/", 1)[0]
        spec = tree.read_json(rel)
        if not isinstance(spec, dict):
            problems.append(f"{rel} is not a JSON object")
            continue
        overlay: dict[str, str] = {}
        ok = True
        for target, payload in sorted((spec.get("overlay") or {}).items()):
            text = tree.read_text(f"{directory}/{payload}")
            if text is None:
                problems.append(f"{rel}: overlay payload {payload!r} is missing")
                ok = False
                continue
            overlay[target] = text
        for target, edits in sorted((spec.get("substitute") or {}).items()):
            base = overlay.get(target, tree.read_text(target))
            if base is None:
                problems.append(f"{rel}: substitute target {target!r} does not exist")
                ok = False
                continue
            for edit in edits:
                find, replace = edit["find"], edit["replace"]
                count = base.count(find)
                if count != 1:
                    problems.append(
                        f"{rel}: substitution anchor for {target!r} matches {count} times, "
                        f"expected 1 — the fixture is stale: {find[:60]!r}"
                    )
                    ok = False
                    break
                base = base.replace(find, replace, 1)
            overlay[target] = base
        for target in spec.get("remove") or []:
            if not tree.exists(target):
                problems.append(f"{rel}: remove target {target!r} does not exist")
                ok = False
        if not ok:
            continue
        fixtures.append(
            Fixture(
                fid=spec.get("id", directory),
                requirement=spec.get("requirement", ""),
                rule=spec.get("rule", ""),
                description=spec.get("description", ""),
                overlay=overlay,
                removed=tuple(spec.get("remove") or ()),
                budget=spec.get("budget"),
            )
        )
    return fixtures, problems


def self_test(tree: Tree) -> tuple[bool, dict[str, object]]:
    """Replay every fixture through the real rules; fail if any goes uncaught."""
    fixtures, problems = load_fixtures(tree)
    failures: list[str] = list(problems)

    baseline = run_requirements(tree)
    dirty = sorted(rid for rid, vs in baseline.items() if vs)
    if dirty:
        failures.append(f"the real repository already violates {dirty}; fixtures prove nothing on it")

    covered: dict[str, list[str]] = {req.rid: [] for req in REQUIREMENTS}
    for fixture in sorted(fixtures, key=lambda f: f.fid):
        req = RULES.get(fixture.rule)
        if req is None:
            failures.append(f"{fixture.fid}: unknown rule {fixture.rule!r}")
            continue
        if req.rid != fixture.requirement:
            failures.append(
                f"{fixture.fid}: rule {fixture.rule!r} belongs to {req.rid}, not {fixture.requirement}"
            )
            continue
        mutated = tree.with_changes(fixture.overlay, fixture.removed)
        found = req.fn(mutated, fixture.budget) if fixture.budget else req.fn(mutated)
        if not found:
            failures.append(f"{fixture.fid}: {req.rid} rule `{req.rule}` did not catch it")
            continue
        covered[req.rid].append(fixture.fid)

    for req in REQUIREMENTS:
        if not covered[req.rid]:
            failures.append(f"{req.rid} has no violating fixture; the rule could pass vacuously")

    return (not failures), {
        "status": "fail" if failures else "pass",
        "fixtures": len(fixtures),
        "fixtures_by_requirement": {k: sorted(v) for k, v in sorted(covered.items())},
        "failures": sorted(failures),
    }


# ============================================================================
# Evidence
# ============================================================================


def build_evidence(tree: Tree, results: dict[str, list[Violation]], st: dict[str, object]) -> dict:
    """A deterministic function of the repository — no clock, no commit id."""
    counts = [count_crate(tree, crate) for crate in KERNEL_CRATES]
    report = matrix_report(tree)
    fixtures_by_req = st.get("fixtures_by_requirement", {})
    receipts = {}
    for crate in KERNEL_CRATES:
        magic, epoch = wire_constants(tree, crate)
        receipts[crate] = {
            "magic": magic,
            "wire_epoch": epoch,
            "wire_epoch_id": f"{crate}/{magic}/{epoch}",
            "golden": f"crates/{crate}/tests/receipt.golden.json",
        }
    return {
        "artifact": "continuum.kernel-covenant.evidence/kcov",
        "produced_by": "tools/check_kernel_covenant.py",
        "reproduce": (
            "python3 tools/check_kernel_covenant.py --evidence "
            f"{COVENANT_DIR}/evidence/kernel-covenant.json"
        ),
        "authority": {
            "code_size_covenant": "notes/plan/docs/03_ASSURANCE_AND_TCB.md §5",
            "dependency_rules": "notes/plan/plan.md §20",
            "pr_deliverables": "notes/plan/notes/START_HERE_IMPLEMENTATION.md PR 9",
            "receipt_shape": RECEIPT_SCHEMA,
            "mutation_matrix": MATRIX,
        },
        "determinism": (
            "This file records no timestamp, commit id, or absolute path: it is a function of "
            "the repository contents alone, so an unchanged tree reproduces it byte-for-byte."
        ),
        "counting_method": {
            "counted": "crates/continuum-kernel-*/src/**/*.rs",
            "excluded_directories": ["tests/", "benches/", "examples/"],
            "excluded_files": sorted(f for c in counts for f in c.excluded_files),
            "excluded_items": "every `#[cfg(test)]` item, brace-matched or `;`-terminated",
            "line_definition": "every surviving physical line, blank and comment lines included",
        },
        "line_budget": {
            "budget": LINE_BUDGET,
            "total": sum(c.total for c in counts),
            "headroom": LINE_BUDGET - sum(c.total for c in counts),
            "by_crate": {
                c.crate: {
                    "total": c.total,
                    "by_file": dict(sorted(c.counted.items())),
                    "non_blank": c.non_blank,
                    "non_blank_non_comment": c.non_blank_non_comment,
                }
                for c in counts
            },
            "context_only": (
                "`non_blank` and `non_blank_non_comment` are reported for comparison and are "
                "not enforced; the enforced number is `total`."
            ),
        },
        "receipts": receipts,
        "mutation_matrix": {
            "classes": list(report.classes) if report else [],
            "cells": (len(report.classes) * len(MATRIX_CRATES)) if report else 0,
            "owning_tests": sum(len(v) for c in (report.owned.values() if report else []) for v in c.values()),
            "not_applicable": [f"{c}/{k}" for c, k in (report.not_applicable if report else ())],
            "uncovered_cells": [f"{c}/{k}" for c, k in (report.uncovered if report else ())],
            "policy": (
                "fail-closed: a (class, crate) cell with no owning test and no waiver fails the "
                "gate. `uncovered` waivers are real gaps, listed above and reported by --strict."
            ),
        },
        "self_test": st,
        "requirements": {
            req.rid: {
                "rule": req.rule,
                "summary": req.summary,
                "source": req.source,
                "boundary": req.boundary,
                "result": "fail" if results[req.rid] else "pass",
                "violations": [str(v) for v in results[req.rid]],
                "fixtures_proven_caught": list(fixtures_by_req.get(req.rid, [])),
            }
            for req in REQUIREMENTS
        },
        "status": "fail" if any(results.values()) else "pass",
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="replay every violating fixture through the rules and fail if any goes uncaught",
    )
    parser.add_argument(
        "--evidence",
        metavar="PATH",
        help="run the self-test and the real check, then write the evidence JSON to PATH",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="promote `uncovered` mutation-matrix waivers to failures",
    )
    parser.add_argument("--quiet", action="store_true", help="print only the status line")
    args = parser.parse_args(argv)

    tree = Tree(ROOT)

    if args.self_test and not args.evidence:
        ok, report = self_test(tree)
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0 if ok else 1

    st: dict[str, object] = {"status": "not-run"}
    st_ok = True
    if args.evidence:
        st_ok, st = self_test(tree)

    results = run_requirements(tree)
    report = matrix_report(tree)
    uncovered = [f"{c}/{k}" for c, k in (report.uncovered if report else ())]
    if args.strict and uncovered:
        results["KCOV-08"].append(
            Violation(f"[mutation-matrix] --strict: {len(uncovered)} uncovered cells: {uncovered}")
        )

    if args.evidence:
        evidence = build_evidence(tree, results, st)
        path = Path(args.evidence)
        if not path.is_absolute():
            path = ROOT / path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    failed = {rid: [str(v) for v in vs] for rid, vs in results.items() if vs}
    counts = [count_crate(tree, crate) for crate in KERNEL_CRATES]
    summary = {
        "requirements": len(REQUIREMENTS),
        "passing": sorted(rid for rid in results if not results[rid]),
        "failing": dict(sorted(failed.items())),
        "non_test_lines": {
            "budget": LINE_BUDGET,
            "total": sum(c.total for c in counts),
            **{c.crate: c.total for c in counts},
        },
        "mutation_matrix": {
            "classes": len(report.classes) if report else 0,
            "uncovered_cells": uncovered,
        },
        "self_test": st.get("status"),
        "evidence": args.evidence,
        "status": "fail" if failed or not st_ok else "pass",
    }
    if args.quiet:
        print(summary["status"])
    else:
        print(json.dumps(summary, indent=2, sort_keys=True))
    return 1 if failed or not st_ok else 0


if __name__ == "__main__":
    raise SystemExit(main())
