"""A complete lexer for the Rust token grammar, for the no_std structure rules.

cr-35ujnx: the no_std evidence layers (the GOV-4-13 not-applicable form here, the
INV-015 audit and each pack's compiler lane) read pack sources as tokens. A scan that
does not know every literal kind can be made to read code as a literal, or a literal
as code, and so miss an `extern crate std`. This module lexes every token kind of the
Rust Reference ("Tokens", "Comments"), edition 2024:

- whitespace, line comments (`//`, `///`, `//!`) and nested block comments
  (`/* /* */ */`, `/** */`, `/*! */`);
- identifiers, keywords and raw identifiers (`r#ident`);
- lifetimes and loop labels (`'a`, `'static`, `'_`, `'r#a`), which are not
  character literals;
- character and byte literals (`'x'`, `b'x'`) with every escape the Reference allows;
- string, byte-string and C-string literals (`"…"`, `b"…"`, `c"…"`);
- raw string, raw byte-string and raw C-string literals with any number of `#`
  (`r"…"`, `r#"…"#`, `br##"…"##`, `cr"…"`);
- numeric literals, and punctuation.

Anything it cannot classify fails: an unterminated literal or comment, an unbalanced
raw-string hash count, an unknown escape, an unknown or reserved prefix (`x"…"`,
`foo#`, and 2024's `#"…"` and `##`), a suffix on a string or character literal, and
any non-ASCII character outside a literal or comment. The rules built on it then fail
closed with "source the scan cannot lex"; they never skip what they cannot read.

`rust_lexer.rs` beside this file is the same lexer in Rust, used by
the INV-015 audit and by the three compiler lanes. `rust_lexer_cases.txt` beside this
file is the corpus both are held to (INV-015 runs the Rust half; the GOV-4-13
fixtures generated from it run this half).
"""

from __future__ import annotations

import sys
from dataclasses import dataclass

IDENT_START = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_")
IDENT_CONT = IDENT_START | set("0123456789")
HEX = set("0123456789abcdefABCDEF")
PUNCT = set("!#$%&()*+,-./:;<=>?@[]^{|}~")
WHITESPACE = set(" \t\n\r\x0b\x0c")
# Prefixes a literal may carry, by what follows them.
QUOTE_PREFIXES = {"b": "byte-string", "c": "c-string"}
RAW_PREFIXES = {"r": "raw-string", "br": "raw-byte-string", "cr": "raw-c-string"}


class LexError(Exception):
    """The source is not in the Rust token grammar this lexer knows."""


@dataclass(frozen=True)
class Token:
    kind: str  # Ident, RawIdent, Lifetime, Literal, RawLiteral, Punct, BlockComment
    text: str
    line: int


def lex(src: str) -> list[Token]:
    """Every token of `src`, comments dropped except that a block comment leaves one
    `BlockComment` token. Raises `LexError` for anything it cannot classify."""
    out: list[Token] = []
    i, n, line = 0, len(src), 1

    def fail(why: str) -> None:
        raise LexError(f"line {line}: {why}")

    def escape(j: int, kind: str) -> int:
        """Validate the escape at `src[j] == '\\'`; return the index after it."""
        if j + 1 >= n:
            fail("unterminated escape")
        c = src[j + 1]
        if c in "nrt\\0'\"":
            return j + 2
        if c == "x":
            digits = src[j + 2 : j + 4]
            if len(digits) != 2 or not set(digits) <= HEX:
                fail("malformed \\x escape")
            if kind in ("char", "string") and int(digits, 16) > 0x7F:
                fail("\\x escape above 0x7F in a character or string literal")
            return j + 4
        if c == "u" and kind in ("char", "string", "c-string"):
            if j + 2 >= n or src[j + 2] != "{":
                fail("malformed \\u escape")
            close = src.find("}", j + 3)
            body = src[j + 3 : close] if close != -1 else ""
            digits = body.replace("_", "")
            if close == -1 or not digits or len(digits) > 6 or not set(digits) <= HEX or body[0] == "_":
                fail("malformed \\u escape")
            return close + 1
        if c == "\n" and kind in ("string", "byte-string", "c-string"):
            return j + 2
        if c == "\r" and j + 2 < n and src[j + 2] == "\n" and kind in ("string", "byte-string", "c-string"):
            return j + 3
        fail(f"unknown escape \\{c!r}")
        return j

    def quoted(j: int, kind: str) -> int:
        """`src[j]` is the opening `"`; return the index after the closing one."""
        nonlocal line
        j += 1
        while j < n:
            c = src[j]
            if c == '"':
                return j + 1
            if c == "\\":
                if src[j + 1 : j + 2] == "\n":
                    line += 1
                j = escape(j, kind)
                continue
            if kind == "byte-string" and ord(c) > 0x7F:
                fail("non-ASCII character in a byte string")
            if c == "\r" and src[j + 1 : j + 2] != "\n":
                fail("bare carriage return in a string")
            if c == "\n":
                line += 1
            j += 1
        fail("unterminated string literal")
        return j

    def raw(j: int) -> int:
        """`src[j]` is the first `#` or the `"` after a raw prefix; return the index
        after the closing quote and hashes."""
        nonlocal line
        hashes = 0
        while j < n and src[j] == "#":
            hashes += 1
            j += 1
        if hashes > 255:
            fail("more than 255 raw-string hashes")
        if j >= n or src[j] != '"':
            fail("raw-string prefix without an opening quote")
        close = '"' + "#" * hashes
        end = src.find(close, j + 1)
        if end == -1:
            fail("unterminated raw string, or unbalanced raw-string hashes")
        line += src.count("\n", j, end)
        return end + len(close)

    def char_literal(j: int, kind: str) -> int:
        """`src[j]` is the opening `'`; return the index after the closing one."""
        j += 1
        if j >= n:
            fail("unterminated character literal")
        if src[j] == "\\":
            j = escape(j, kind)
        elif src[j] in "'\n\r\t":
            fail("empty or unescaped character literal")
        else:
            if kind == "byte" and ord(src[j]) > 0x7F:
                fail("non-ASCII byte literal")
            j += 1
        if j >= n or src[j] != "'":
            fail("unterminated character literal")
        return j + 1

    def no_suffix(j: int) -> None:
        if j < n and src[j] in IDENT_CONT:
            fail("a suffix on a string or character literal")

    while i < n:
        c = src[i]
        start_line = line
        if c in WHITESPACE:
            if c == "\n":
                line += 1
            i += 1
        elif src.startswith("//", i):
            end = src.find("\n", i)
            i = n if end == -1 else end
        elif src.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif src.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    if src[j] == "\n":
                        line += 1
                    j += 1
            if depth:
                fail("unterminated block comment")
            out.append(Token("BlockComment", src[i:j], start_line))
            i = j
        elif c in IDENT_START:
            j = i
            while j < n and src[j] in IDENT_CONT:
                j += 1
            word = src[i:j]
            nxt = src[j] if j < n else ""
            if word in RAW_PREFIXES and (nxt == '"' or (nxt == "#" and src[j + 1 : j + 2] in ('"', "#"))):
                end = raw(j)
                no_suffix(end)
                out.append(Token("RawLiteral", src[i:end], start_line))
                i = end
            elif word == "r" and nxt == "#" and src[j + 1 : j + 2] in IDENT_START:
                k = j + 1
                while k < n and src[k] in IDENT_CONT:
                    k += 1
                out.append(Token("RawIdent", src[i:k], start_line))
                i = k
            elif word in QUOTE_PREFIXES and nxt == '"':
                end = quoted(j, QUOTE_PREFIXES[word])
                no_suffix(end)
                out.append(Token("Literal", src[i:end], start_line))
                i = end
            elif word == "b" and nxt == "'":
                end = char_literal(j, "byte")
                no_suffix(end)
                out.append(Token("Literal", src[i:end], start_line))
                i = end
            elif nxt in ('"', "'", "#"):
                fail(f"unknown or reserved prefix `{word}{nxt}`")
            else:
                out.append(Token("Ident", word, start_line))
                i = j
        elif c == '"':
            end = quoted(i, "string")
            no_suffix(end)
            out.append(Token("Literal", src[i:end], start_line))
            i = end
        elif c == "'":
            # A character literal, or a lifetime or label.
            if src[i + 1 : i + 2] == "\\" or (i + 2 < n and src[i + 2] == "'"):
                end = char_literal(i, "char")
                no_suffix(end)
                out.append(Token("Literal", src[i:end], start_line))
                i = end
            elif src[i + 1 : i + 2] in IDENT_START:
                j = i + 1
                if src.startswith("r#", j) and src[j + 2 : j + 3] in IDENT_START:
                    j += 2
                while j < n and src[j] in IDENT_CONT:
                    j += 1
                if j < n and src[j] == "'":
                    fail("a lifetime followed by a quote")
                out.append(Token("Lifetime", src[i:j], start_line))
                i = j
            else:
                fail("a quote that opens neither a character literal nor a lifetime")
        elif c.isdigit():
            j = i + 1
            while j < n:
                if src[j] in IDENT_CONT:
                    if src[j] in "eE" and src[j + 1 : j + 2] in ("+", "-") and not src[i:j].lower().startswith("0x"):
                        j += 2
                        continue
                    j += 1
                elif src[j] == "." and src[j + 1 : j + 2].isdigit():
                    j += 1
                else:
                    break
            if j < n and src[j] == "." and src[j + 1 : j + 2] not in (".",) and not (
                src[j + 1 : j + 2] in IDENT_START
            ):
                j += 1  # `1.` is a float literal
            out.append(Token("Literal", src[i:j], start_line))
            i = j
        elif c == "#" and src[i + 1 : i + 2] in ('"', "#"):
            fail("a reserved guarded-string or `##` token")
        elif c in PUNCT:
            out.append(Token("Punct", c, start_line))
            i += 1
        else:
            fail(f"character {c!r} outside a literal or comment")
    return out


# Every compile-time input a pack could read from its build environment rather than
# from its own source (cr-35ujnx round 4): environment variables (`env!`,
# `option_env!`), other files (`include!`, `include_str!`, `include_bytes!`), the
# invocation path (`file!`), and the build configuration (`cfg`, `cfg_attr`: target,
# features and `--cfg` flags). A build script and `links` are refused by the manifest
# rule, and proc macros need a dependency, which the manifest rule also refuses. The
# identifier is refused wherever it occurs as a token, so a path such as
# `core::option_env!` or a space before the `!` changes nothing; inside a literal or a
# comment it is not a token and does not count. The compiler lanes add the compiler's
# own witness: rustc's dep-info for each pack crate lists no `# env-dep:` line.
AMBIENT = frozenset({"env", "option_env", "include", "include_str", "include_bytes", "file", "cfg", "cfg_attr"})
# Identifiers that would let a build escape the structure the rules describe:
# macro definitions, a module loaded from another path, and inline assembly.
BANNED = frozenset({"macro_rules", "path", "asm", "global_asm"})


def structure_problem(files: list[tuple[str, str]]) -> str | None:
    """The no_std structure rules over one crate's `src/` files, token by token, or
    `None` when they hold. The same rules as `rust_lexer.rs`'s `structure`: the crate
    root opens with `#![no_std]`; no file has a block comment, a raw literal or raw
    identifier, an ambient compile-time input (`AMBIENT`) or a banned identifier; and
    the token `extern` occurs exactly once, in
    `lib.rs`, as `extern crate alloc;`. A file the lexer cannot read fails."""
    externs, saw_lib = 0, False
    for name, text in files:
        try:
            toks = lex(text)
        except LexError as why:
            return f"{name}: source the scan cannot lex: {why}"
        is_lib = name.endswith("lib.rs")
        if is_lib:
            saw_lib = True
            if [t.text for t in toks[:5]] != ["#", "!", "[", "no_std", "]"]:
                return f"{name} does not open with `#![no_std]` as the crate root's first item"
        for at, tok in enumerate(toks):
            if tok.kind == "BlockComment":
                return f"{name} has a block comment on line {tok.line}"
            if tok.kind in ("RawLiteral", "RawIdent"):
                return f"{name} uses a raw identifier or string on line {tok.line}"
            if tok.kind == "Ident" and tok.text in AMBIENT:
                return f"{name} reads the build environment at compile time with `{tok.text}` on line {tok.line}"
            if tok.kind == "Ident" and tok.text in BANNED:
                return f"{name} uses `{tok.text}` on line {tok.line}"
            if tok.kind == "Ident" and tok.text == "extern":
                externs += 1
                following = [t.text for t in toks[at + 1 : at + 4]]
                if not is_lib or following != ["crate", "alloc", ";"]:
                    return (
                        f"{name} has an `extern` other than `extern crate alloc;` on line {tok.line}: "
                        f"extern {' '.join(following)}"
                    )
    if not saw_lib:
        return "no lib.rs"
    if externs != 1:
        return f"{externs} extern declarations, not exactly `extern crate alloc;`"
    return None


def main(argv: list[str]) -> int:
    """`--tokens`: lex standard input and print its tokens as JSON, or the error.
    `--rules`: print the ambient and banned identifier sets. The Rust twin's
    differential tests (INV-015) drive both."""
    import json

    if argv == ["--rules"]:
        print(json.dumps({"ambient": sorted(AMBIENT), "banned": sorted(BANNED)}))
        return 0
    if argv != ["--tokens"]:
        print("usage: rust_lexer.py --tokens < source.rs | --rules", file=sys.stderr)
        return 2
    src = sys.stdin.read()
    try:
        print(json.dumps({"tokens": [[t.kind, t.text, t.line] for t in lex(src)]}))
    except LexError as why:
        print(json.dumps({"error": str(why)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
