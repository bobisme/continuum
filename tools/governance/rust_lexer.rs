// A complete lexer for the Rust token grammar, for the no_std structure rules
// (cr-35ujnx). It is the Rust twin of `rust_lexer.py` beside it: see that module's
// documentation for what it lexes and what it refuses. The INV-015 audit and the three
// pack compiler lanes `include!` this one file, so they share one lexer without a
// dependency edge; `rust_lexer_cases.txt` is the corpus both twins are held to.
//
// Anything it cannot classify is an `Err`: an unterminated literal or comment, an
// unbalanced raw-string hash count, an unknown escape, an unknown or reserved prefix,
// a suffix on a string or character literal, and any non-ASCII character outside a
// literal or comment. The rules built on it fail closed on that error.

/// A complete Rust token lexer (cr-35ujnx), shared by `include!`.
#[allow(dead_code)]
mod rust_lexer {
    /// One token's kind.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Kind {
        /// An identifier or keyword.
        Ident,
        /// `r#ident`.
        RawIdent,
        /// `'a`, `'static`, `'_`, a loop label.
        Lifetime,
        /// A character, byte, string, byte-string, C-string or numeric literal.
        Literal,
        /// A raw string, raw byte-string or raw C-string literal.
        RawLiteral,
        /// One punctuation character.
        Punct,
        /// A (possibly nested) block comment.
        BlockComment,
    }

    /// One token.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Token {
        /// Its kind.
        pub kind: Kind,
        /// Its text.
        pub text: String,
        /// The line it starts on, from 1.
        pub line: usize,
    }

    fn ident_start(c: char) -> bool {
        c.is_ascii_alphabetic() || c == '_'
    }

    fn ident_cont(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_'
    }

    struct Lexer {
        src: Vec<char>,
        line: usize,
    }

    impl Lexer {
        fn at(&self, j: usize) -> Option<char> {
            self.src.get(j).copied()
        }

        fn starts(&self, j: usize, s: &str) -> bool {
            s.chars().enumerate().all(|(k, c)| self.at(j + k) == Some(c))
        }

        fn fail<T>(&self, why: &str) -> Result<T, String> {
            Err(format!("line {}: {why}", self.line))
        }

        /// Validate the escape at `src[j] == '\\'`; the index after it.
        fn escape(&self, j: usize, kind: &str) -> Result<usize, String> {
            let Some(c) = self.at(j + 1) else {
                return self.fail("unterminated escape");
            };
            let string_like = matches!(kind, "string" | "byte-string" | "c-string");
            match c {
                'n' | 'r' | 't' | '\\' | '0' | '\'' | '"' => Ok(j + 2),
                'x' => {
                    let digits: Vec<char> = (j + 2..j + 4).filter_map(|k| self.at(k)).collect();
                    if digits.len() != 2 || !digits.iter().all(char::is_ascii_hexdigit) {
                        return self.fail("malformed \\x escape");
                    }
                    let value = digits
                        .iter()
                        .fold(0, |acc, d| acc * 16 + d.to_digit(16).unwrap_or(0));
                    if matches!(kind, "char" | "string") && value > 0x7F {
                        return self.fail("\\x escape above 0x7F in a character or string literal");
                    }
                    Ok(j + 4)
                }
                'u' if matches!(kind, "char" | "string" | "c-string") => {
                    if self.at(j + 2) != Some('{') {
                        return self.fail("malformed \\u escape");
                    }
                    let mut k = j + 3;
                    while self.at(k).is_some_and(|c| c != '}') {
                        k += 1;
                    }
                    let body: String = self.src[(j + 3).min(k)..k].iter().collect();
                    let digits: String = body.chars().filter(|c| *c != '_').collect();
                    if self.at(k) != Some('}')
                        || digits.is_empty()
                        || digits.len() > 6
                        || !digits.chars().all(|c| c.is_ascii_hexdigit())
                        || body.starts_with('_')
                    {
                        return self.fail("malformed \\u escape");
                    }
                    Ok(k + 1)
                }
                '\n' if string_like => Ok(j + 2),
                '\r' if string_like && self.at(j + 2) == Some('\n') => Ok(j + 3),
                other => self.fail(&format!("unknown escape \\{other:?}")),
            }
        }

        /// `src[j]` is the opening `"`; the index after the closing one.
        fn quoted(&mut self, mut j: usize, kind: &str) -> Result<usize, String> {
            j += 1;
            while let Some(c) = self.at(j) {
                if c == '"' {
                    return Ok(j + 1);
                }
                if c == '\\' {
                    if self.at(j + 1) == Some('\n') {
                        self.line += 1;
                    }
                    j = self.escape(j, kind)?;
                    continue;
                }
                if kind == "byte-string" && !c.is_ascii() {
                    return self.fail("non-ASCII character in a byte string");
                }
                if c == '\r' && self.at(j + 1) != Some('\n') {
                    return self.fail("bare carriage return in a string");
                }
                if c == '\n' {
                    self.line += 1;
                }
                j += 1;
            }
            self.fail("unterminated string literal")
        }

        /// `src[j]` is the first `#` or the `"` after a raw prefix; the index after the
        /// closing quote and hashes.
        fn raw(&mut self, mut j: usize) -> Result<usize, String> {
            let mut hashes = 0;
            while self.at(j) == Some('#') {
                hashes += 1;
                j += 1;
            }
            if hashes > 255 {
                return self.fail("more than 255 raw-string hashes");
            }
            if self.at(j) != Some('"') {
                return self.fail("raw-string prefix without an opening quote");
            }
            let close: String = std::iter::once('"')
                .chain(std::iter::repeat_n('#', hashes))
                .collect();
            let mut k = j + 1;
            while k < self.src.len() {
                if self.starts(k, &close) {
                    self.line += self.src[j..k].iter().filter(|c| **c == '\n').count();
                    return Ok(k + close.chars().count());
                }
                k += 1;
            }
            self.fail("unterminated raw string, or unbalanced raw-string hashes")
        }

        /// `src[j]` is the opening `'`; the index after the closing one.
        fn char_literal(&self, mut j: usize, kind: &str) -> Result<usize, String> {
            j += 1;
            match self.at(j) {
                None => return self.fail("unterminated character literal"),
                Some('\\') => j = self.escape(j, kind)?,
                Some('\'' | '\n' | '\r' | '\t') => {
                    return self.fail("empty or unescaped character literal");
                }
                Some(c) => {
                    if kind == "byte" && !c.is_ascii() {
                        return self.fail("non-ASCII byte literal");
                    }
                    j += 1;
                }
            }
            if self.at(j) != Some('\'') {
                return self.fail("unterminated character literal");
            }
            Ok(j + 1)
        }

        fn no_suffix(&self, j: usize) -> Result<(), String> {
            if self.at(j).is_some_and(ident_cont) {
                return self.fail("a suffix on a string or character literal");
            }
            Ok(())
        }
    }

    /// Every token of `src`, comments dropped except that a block comment leaves one
    /// `BlockComment` token. An `Err` for anything it cannot classify.
    #[allow(clippy::too_many_lines)]
    pub fn lex(src: &str) -> Result<Vec<Token>, String> {
        let mut lx = Lexer {
            src: src.chars().collect(),
            line: 1,
        };
        let n = lx.src.len();
        let mut out = Vec::new();
        let mut i = 0;
        let text = |lx: &Lexer, a: usize, b: usize| -> String { lx.src[a..b].iter().collect() };
        while i < n {
            let c = lx.src[i];
            let line = lx.line;
            let mut push = |kind: Kind, text: String| out.push(Token { kind, text, line });
            if matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}') {
                if c == '\n' {
                    lx.line += 1;
                }
                i += 1;
            } else if lx.starts(i, "//") {
                while i < n && lx.src[i] != '\n' {
                    i += 1;
                }
            } else if lx.starts(i, "/*") {
                let (mut depth, mut j) = (1, i + 2);
                while j < n && depth > 0 {
                    if lx.starts(j, "/*") {
                        depth += 1;
                        j += 2;
                    } else if lx.starts(j, "*/") {
                        depth -= 1;
                        j += 2;
                    } else {
                        if lx.src[j] == '\n' {
                            lx.line += 1;
                        }
                        j += 1;
                    }
                }
                if depth > 0 {
                    return lx.fail("unterminated block comment");
                }
                push(Kind::BlockComment, text(&lx, i, j));
                i = j;
            } else if ident_start(c) {
                let mut j = i;
                while j < n && ident_cont(lx.src[j]) {
                    j += 1;
                }
                let word = text(&lx, i, j);
                let nxt = lx.at(j);
                let raw_prefix = matches!(word.as_str(), "r" | "br" | "cr");
                if raw_prefix
                    && (nxt == Some('"')
                        || (nxt == Some('#') && matches!(lx.at(j + 1), Some('"' | '#'))))
                {
                    let end = lx.raw(j)?;
                    lx.no_suffix(end)?;
                    push(Kind::RawLiteral, text(&lx, i, end));
                    i = end;
                } else if word == "r" && nxt == Some('#') && lx.at(j + 1).is_some_and(ident_start) {
                    let mut k = j + 1;
                    while k < n && ident_cont(lx.src[k]) {
                        k += 1;
                    }
                    push(Kind::RawIdent, text(&lx, i, k));
                    i = k;
                } else if matches!(word.as_str(), "b" | "c") && nxt == Some('"') {
                    let kind = if word == "b" { "byte-string" } else { "c-string" };
                    let end = lx.quoted(j, kind)?;
                    lx.no_suffix(end)?;
                    push(Kind::Literal, text(&lx, i, end));
                    i = end;
                } else if word == "b" && nxt == Some('\'') {
                    let end = lx.char_literal(j, "byte")?;
                    lx.no_suffix(end)?;
                    push(Kind::Literal, text(&lx, i, end));
                    i = end;
                } else if matches!(nxt, Some('"' | '\'' | '#')) {
                    return lx.fail(&format!(
                        "unknown or reserved prefix `{word}{}`",
                        nxt.unwrap_or(' ')
                    ));
                } else {
                    push(Kind::Ident, word);
                    i = j;
                }
            } else if c == '"' {
                let end = lx.quoted(i, "string")?;
                lx.no_suffix(end)?;
                push(Kind::Literal, text(&lx, i, end));
                i = end;
            } else if c == '\'' {
                if lx.at(i + 1) == Some('\\') || lx.at(i + 2) == Some('\'') {
                    let end = lx.char_literal(i, "char")?;
                    lx.no_suffix(end)?;
                    push(Kind::Literal, text(&lx, i, end));
                    i = end;
                } else if lx.at(i + 1).is_some_and(ident_start) {
                    let mut j = i + 1;
                    if lx.starts(j, "r#") && lx.at(j + 2).is_some_and(ident_start) {
                        j += 2;
                    }
                    while j < n && ident_cont(lx.src[j]) {
                        j += 1;
                    }
                    if lx.at(j) == Some('\'') {
                        return lx.fail("a lifetime followed by a quote");
                    }
                    push(Kind::Lifetime, text(&lx, i, j));
                    i = j;
                } else {
                    return lx.fail("a quote that opens neither a character literal nor a lifetime");
                }
            } else if c.is_ascii_digit() {
                let mut j = i + 1;
                let hex = lx.starts(i, "0x") || lx.starts(i, "0X");
                while j < n {
                    let d = lx.src[j];
                    if ident_cont(d) {
                        if matches!(d, 'e' | 'E') && matches!(lx.at(j + 1), Some('+' | '-')) && !hex {
                            j += 2;
                            continue;
                        }
                        j += 1;
                    } else if d == '.' && lx.at(j + 1).is_some_and(|c| c.is_ascii_digit()) {
                        j += 1;
                    } else {
                        break;
                    }
                }
                if lx.at(j) == Some('.')
                    && lx.at(j + 1) != Some('.')
                    && !lx.at(j + 1).is_some_and(ident_start)
                {
                    j += 1;
                }
                push(Kind::Literal, text(&lx, i, j));
                i = j;
            } else if c == '#' && matches!(lx.at(i + 1), Some('"' | '#')) {
                return lx.fail("a reserved guarded-string or `##` token");
            } else if "!#$%&()*+,-./:;<=>?@[]^{|}~".contains(c) {
                push(Kind::Punct, c.to_string());
                i += 1;
            } else {
                return lx.fail(&format!("character {c:?} outside a literal or comment"));
            }
        }
        Ok(out)
    }

    /// Every compile-time input a pack could read from its build environment rather
    /// than from its own source (cr-35ujnx round 4): environment variables (`env!`,
    /// `option_env!`), other files (`include!`, `include_str!`, `include_bytes!`), the
    /// invocation path (`file!`), and the build configuration (`cfg`, `cfg_attr`). A
    /// build script and `links` are refused by the manifest rule, and proc macros need
    /// a dependency, which it also refuses. The identifier is refused wherever it is a
    /// token; inside a literal or comment it is not one. The compiler lanes add the
    /// compiler's witness: rustc's dep-info lists no `# env-dep:` line.
    pub const AMBIENT: [&str; 8] = [
        "env",
        "option_env",
        "include",
        "include_str",
        "include_bytes",
        "file",
        "cfg",
        "cfg_attr",
    ];

    /// Identifiers that would let a build escape the described structure: macro
    /// definitions, a module loaded from another path, and inline assembly.
    pub const BANNED: [&str; 4] = ["macro_rules", "path", "asm", "global_asm"];

    /// The no_std structure rules over one crate's `src/` files, token by token: the
    /// crate root opens with `#![no_std]`; no file has a block comment, a raw literal
    /// or raw identifier, an ambient compile-time input ([`AMBIENT`]) or a banned
    /// identifier ([`BANNED`]); and the token `extern` occurs exactly once, in
    /// `lib.rs`, as `extern crate alloc;`. A file the lexer cannot read fails: "source
    /// the scan cannot lex".
    pub fn structure<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> Result<(), String> {
        let mut externs = 0;
        let mut saw_lib = false;
        for (name, text) in files {
            let toks = lex(text).map_err(|why| format!("{name}: source the scan cannot lex: {why}"))?;
            let is_lib = name.ends_with("lib.rs");
            if is_lib {
                saw_lib = true;
                let head: Vec<&str> = toks.iter().take(5).map(|t| t.text.as_str()).collect();
                if head != ["#", "!", "[", "no_std", "]"] {
                    return Err("`#![no_std]` is not the crate root's first item".to_owned());
                }
            }
            for (at, tok) in toks.iter().enumerate() {
                match tok.kind {
                    Kind::BlockComment => {
                        return Err(format!("{name} has a block comment on line {}", tok.line));
                    }
                    Kind::RawLiteral | Kind::RawIdent => {
                        return Err(format!(
                            "{name} uses a raw identifier or string on line {}",
                            tok.line
                        ));
                    }
                    Kind::Ident if AMBIENT.contains(&tok.text.as_str()) => {
                        return Err(format!(
                            "{name} reads the build environment at compile time with `{}` on \
                             line {}",
                            tok.text, tok.line
                        ));
                    }
                    Kind::Ident if BANNED.contains(&tok.text.as_str()) => {
                        return Err(format!("{name} uses `{}` on line {}", tok.text, tok.line));
                    }
                    Kind::Ident if tok.text == "extern" => {
                        externs += 1;
                        let next: Vec<&str> =
                            toks[at + 1..].iter().take(3).map(|t| t.text.as_str()).collect();
                        if !is_lib || next != ["crate", "alloc", ";"] {
                            return Err(format!(
                                "{name} has an `extern` other than `extern crate alloc;` on line {}: \
                                 extern {}",
                                tok.line,
                                next.join(" ")
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }
        if !saw_lib {
            return Err("no lib.rs".to_owned());
        }
        if externs != 1 {
            return Err(format!(
                "{externs} extern declarations, not exactly `extern crate alloc;`"
            ));
        }
        Ok(())
    }

    /// The `extern` rule alone, token by token, for the literal-kind corpus: literal
    /// contents are never tokens, so an `extern crate std` inside any literal is not
    /// a declaration, and one beside it is.
    pub fn extern_problem(text: &str) -> Result<(), String> {
        let toks = lex(text).map_err(|why| format!("source the scan cannot lex: {why}"))?;
        for (at, tok) in toks.iter().enumerate() {
            if tok.kind == Kind::Ident && tok.text == "extern" {
                let next: Vec<&str> =
                    toks[at + 1..].iter().take(3).map(|t| t.text.as_str()).collect();
                if next != ["crate", "alloc", ";"] {
                    return Err(format!("extern {}", next.join(" ")));
                }
            }
        }
        Ok(())
    }
}

/// Cargo's resolved view of a pack package (cr-35ujnx round 5), shared by `include!`
/// with the lexer above. No manifest text scan carries the no-build-script claim: a
/// quoted, dotted or inline-table `build` or `links` key, and a `build.rs` that Cargo
/// finds with no key at all, are all visible here only as what Cargo resolved them to.
#[allow(dead_code)]
mod cargo_view {
    use std::path::Path;
    use std::process::Command;

    /// A JSON value, as much of JSON as `cargo metadata` prints.
    #[derive(Debug, Clone, PartialEq)]
    pub enum Json {
        /// `null`.
        Null,
        /// `true` or `false`.
        Bool(bool),
        /// A number, as written.
        Num(String),
        /// A string.
        Str(String),
        /// An array.
        Arr(Vec<Json>),
        /// An object, in order.
        Obj(Vec<(String, Json)>),
    }

    impl Json {
        /// The member `key` of an object.
        pub fn get(&self, key: &str) -> Option<&Json> {
            match self {
                Json::Obj(members) => members.iter().find(|(k, _)| k == key).map(|(_, v)| v),
                _ => None,
            }
        }

        /// The string, if this is one.
        pub fn str(&self) -> Option<&str> {
            match self {
                Json::Str(s) => Some(s),
                _ => None,
            }
        }

        /// The elements, if this is an array.
        pub fn arr(&self) -> Option<&[Json]> {
            match self {
                Json::Arr(items) => Some(items),
                _ => None,
            }
        }
    }

    /// Parse one JSON document; an `Err` for anything that is not one.
    pub fn parse(text: &str) -> Result<Json, String> {
        let chars: Vec<char> = text.chars().collect();
        let mut at = 0;
        let value = value(&chars, &mut at)?;
        skip_ws(&chars, &mut at);
        if at != chars.len() {
            return Err(format!("trailing input at {at}"));
        }
        Ok(value)
    }

    fn skip_ws(c: &[char], at: &mut usize) {
        while *at < c.len() && c[*at].is_whitespace() {
            *at += 1;
        }
    }

    fn value(c: &[char], at: &mut usize) -> Result<Json, String> {
        skip_ws(c, at);
        match c.get(*at) {
            Some('n') => literal(c, at, "null", Json::Null),
            Some('t') => literal(c, at, "true", Json::Bool(true)),
            Some('f') => literal(c, at, "false", Json::Bool(false)),
            Some('"') => string(c, at).map(Json::Str),
            Some('[') => {
                *at += 1;
                let mut items = Vec::new();
                skip_ws(c, at);
                if c.get(*at) == Some(&']') {
                    *at += 1;
                    return Ok(Json::Arr(items));
                }
                loop {
                    items.push(value(c, at)?);
                    skip_ws(c, at);
                    match c.get(*at) {
                        Some(',') => *at += 1,
                        Some(']') => {
                            *at += 1;
                            return Ok(Json::Arr(items));
                        }
                        _ => return Err(format!("bad array at {at}")),
                    }
                }
            }
            Some('{') => {
                *at += 1;
                let mut members = Vec::new();
                skip_ws(c, at);
                if c.get(*at) == Some(&'}') {
                    *at += 1;
                    return Ok(Json::Obj(members));
                }
                loop {
                    skip_ws(c, at);
                    let key = string(c, at)?;
                    skip_ws(c, at);
                    if c.get(*at) != Some(&':') {
                        return Err(format!("expected `:` at {at}"));
                    }
                    *at += 1;
                    members.push((key, value(c, at)?));
                    skip_ws(c, at);
                    match c.get(*at) {
                        Some(',') => *at += 1,
                        Some('}') => {
                            *at += 1;
                            return Ok(Json::Obj(members));
                        }
                        _ => return Err(format!("bad object at {at}")),
                    }
                }
            }
            Some(d) if *d == '-' || d.is_ascii_digit() => {
                let start = *at;
                while *at < c.len() && (c[*at].is_ascii_digit() || "+-.eE".contains(c[*at])) {
                    *at += 1;
                }
                Ok(Json::Num(c[start..*at].iter().collect()))
            }
            _ => Err(format!("unexpected input at {at}")),
        }
    }

    fn literal(c: &[char], at: &mut usize, word: &str, v: Json) -> Result<Json, String> {
        let end = *at + word.len();
        if end <= c.len() && c[*at..end].iter().copied().eq(word.chars()) {
            *at = end;
            Ok(v)
        } else {
            Err(format!("bad literal at {at}"))
        }
    }

    fn string(c: &[char], at: &mut usize) -> Result<String, String> {
        if c.get(*at) != Some(&'"') {
            return Err(format!("expected a string at {at}"));
        }
        *at += 1;
        let mut out = String::new();
        while let Some(&ch) = c.get(*at) {
            *at += 1;
            match ch {
                '"' => return Ok(out),
                '\\' => {
                    let esc = *c.get(*at).ok_or("unterminated escape")?;
                    *at += 1;
                    match esc {
                        '"' | '\\' | '/' => out.push(esc),
                        'n' => out.push('\n'),
                        't' => out.push('\t'),
                        'r' => out.push('\r'),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'u' => {
                            let hex: String = c.get(*at..*at + 4).ok_or("short \\u")?.iter().collect();
                            *at += 4;
                            let unit = u32::from_str_radix(&hex, 16).map_err(|e| e.to_string())?;
                            out.push(char::from_u32(unit).unwrap_or('\u{fffd}'));
                        }
                        other => return Err(format!("bad escape {other:?}")),
                    }
                }
                other => out.push(other),
            }
        }
        Err("unterminated string".to_owned())
    }

    /// `cargo metadata --format-version 1 --no-deps --offline` for a manifest, with
    /// `--locked` when asked. A Cargo error is an `Err`: a manifest Cargo refuses is
    /// refused here too, never skipped.
    pub fn metadata(manifest: &Path, locked: bool) -> Result<Json, String> {
        let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()));
        command
            .args(["metadata", "--format-version", "1", "--no-deps", "--offline"])
            .arg("--manifest-path")
            .arg(manifest);
        if locked {
            command.arg("--locked");
        }
        let output = command.output().map_err(|e| format!("cargo metadata did not run: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "cargo metadata refused the manifest: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        parse(&String::from_utf8_lossy(&output.stdout))
    }

    /// Why Cargo's resolved view of `package` lets a build escape the pack's own
    /// `src/`, or `Ok`: a `custom-build` target (a build script, declared or found), a
    /// `proc-macro` target, a `links` value, any dependency, any feature, or a library
    /// whose source is not `<pack_dir>/src/lib.rs`.
    pub fn pack_problem(meta: &Json, package: &str, pack_dir: &Path) -> Result<(), String> {
        let packages = meta.get("packages").and_then(Json::arr).ok_or("no packages")?;
        let pkg = packages
            .iter()
            .find(|p| p.get("name").and_then(Json::str) == Some(package))
            .ok_or_else(|| format!("cargo metadata has no package {package}"))?;
        if pkg.get("links") != Some(&Json::Null) {
            return Err(format!("{package} sets `links`: {:?}", pkg.get("links")));
        }
        if pkg.get("dependencies").and_then(Json::arr).is_none_or(|d| !d.is_empty()) {
            return Err(format!("{package} has dependencies"));
        }
        if pkg.get("features") != Some(&Json::Obj(Vec::new())) {
            return Err(format!("{package} declares features"));
        }
        let lib = pack_dir.join("src").join("lib.rs");
        let lib = lib.canonicalize().unwrap_or(lib);
        let mut libs = 0;
        for target in pkg.get("targets").and_then(Json::arr).ok_or("no targets")? {
            let kinds: Vec<&str> = target
                .get("kind")
                .and_then(Json::arr)
                .ok_or("a target with no kind")?
                .iter()
                .filter_map(Json::str)
                .collect();
            for bad in ["custom-build", "proc-macro"] {
                if kinds.contains(&bad) {
                    return Err(format!("{package} has a `{bad}` target"));
                }
            }
            if kinds.contains(&"lib") {
                libs += 1;
                let src = target.get("src_path").and_then(Json::str).ok_or("a lib with no src_path")?;
                let src = Path::new(src);
                let src = src.canonicalize().unwrap_or_else(|_| src.to_path_buf());
                if src != lib {
                    return Err(format!("{package}'s library is {}, not its src/lib.rs", src.display()));
                }
            } else if !kinds.iter().all(|k| *k == "test") {
                return Err(format!("{package} has a target of kind {kinds:?}"));
            }
        }
        if libs != 1 {
            return Err(format!("{package} has {libs} library targets"));
        }
        Ok(())
    }
}
