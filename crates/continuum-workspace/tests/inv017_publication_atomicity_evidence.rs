//! INV-017 as an architectural guard (bone `bn-svf6`).
//!
//! > **INV-017 — Semantic atomicity of publication.** Artifacts become visible only after
//! > their content, provenance, and references are durably committed.
//! >
//! > — `notes/plan/plan.md` §2
//!
//! # What this file adds, and what it does not repeat
//!
//! The behaviour of publication is already evidenced, at three grains:
//!
//! | Evidence | Grain | Question it answers |
//! |---|---|---|
//! | `tests/dx13_falsification.rs`, `tests/dx13_mutation_campaign.rs`, `tests/publication_schedule_matrix.rs` | [`ReferenceStore`](continuum_workspace::publication::ReferenceStore) | Does *this* store publish atomically under contention, faults and every weaving? |
//! | `crates/continuumd/tests/gate_g1_06_acceptance.rs` (bn-2vbqm, bn-12plt) | `Daemon::dispatch` | Does every daemon operation that publishes today leave the namespace intact on abort? |
//! | `crates/continuumd/tests/g1_crash_recovery_evidence.rs` | process death | Does the durable image recover to a consistent namespace? |
//!
//! Every row is a probe of code that exists. None of them fails when **future** code makes
//! an artifact visible by a path that does not go through the committed publication path:
//! a second index writer inside the store, a receipt minted by a new constructor, a read
//! that serves content the index does not name, or a daemon helper that holds the
//! content-committed witness while it does other work. That is the gap this file closes. It
//! is a guard over the *shape of the program*, so it runs against the source tree, and a
//! guard over a shape is only evidence when a mutant of that shape is shown to fail it.
//!
//! G1-06's census (`the_publication_seam_census_is_closed_over_the_daemon_source`) scans
//! `continuumd/src` for four receiver-spelled calls (`store.publish(` and so on) and
//! adjudicates each *operation*. This file does not re-adjudicate operations. It differs in
//! three ways: it reads every crate's source, not only the daemon's; it matches
//! receiver-independent token sequences, so `s.publish(` on a renamed binding is still a
//! site; and it attributes each site to its enclosing item (`impl` type and `fn`), which is
//! what the rules below are stated over.
//!
//! # The choice: a type-level guard, plus a census that keeps it closed
//!
//! Two designs were on the table.
//!
//! 1. **A type-level guard.** Visibility requires a value only the commit path can mint.
//!    `continuum-workspace` already has it: the index is written only by
//!    `CommittedContent::commit_index`, a method on the witness that
//!    `StagedPublication::commit_content` alone returns, and the receipt is appended in
//!    the same critical section. The compiler enforces this *across* crates, because the
//!    fields of `StoreState`, `CommittedContent` and `PublicationReceipt` are private. Two
//!    `compile_fail` doctests on `PublicationReceipt` and `CommittedContent` (added with
//!    this file) pin that a caller outside the crate cannot forge either one.
//! 2. **A census.** Scan for every visibility write and every publication call and require
//!    each to be a sanctioned site.
//!
//! Neither is sufficient alone. The type-level guard has three holes the compiler cannot
//! see. First, privacy is per *module*, so code inside `publication.rs` can write
//! `state.index` from anywhere. Second, the witness is only as strong as its constructor
//! set, and a new `impl` can add one. Third, a consumer can hold the witness across
//! unrelated work, so that other state becomes visible between the two commits. The
//! census alone is spelling-bound. It cannot say *why* a site is safe. It can only say
//! that the site was reviewed.
//!
//! So this file uses **both**, each for what it can prove. The type-level guard carries
//! the ordering, and the census keeps its premises true. The rules are:
//!
//! | Rule | Scope | Premise of the type-level guard it keeps true |
//! |---|---|---|
//! | [`Violation::StateNotPrivate`] | `publication.rs` | `StoreState` and its fields are private, and the module has no non-test child, so every access to the maps is in this one file and the census over it is complete |
//! | [`Violation::VisibilityWriteOutsideWitness`] | `publication.rs` | the index (the only thing a read consults) is written only in `CommittedContent::commit_index` |
//! | [`Violation::ProvenanceWriteOutsideWitness`] | `publication.rs` | the receipt ledger is written only there too |
//! | [`Violation::VisibilityAndProvenanceNotOneCriticalSection`] | `publication.rs` | no lock release separates the index write from the ledger write, so no reader sees one without the other |
//! | [`Violation::ContentWriteOutsideContentCommit`] | `publication.rs` | content is written only by the content commit, and removed only by garbage collection |
//! | [`Violation::ContentServedWithoutIndex`] | `publication.rs` | a read that returns content has consulted the index first |
//! | [`Violation::WitnessMintedOutsideCommitPath`] | every crate | `StagedPublication`, `CommittedContent`, `PublicationReceipt` and `StoreState` are constructed (or destructured) only at their one sanctioned site |
//! | [`Violation::ForbiddenDerive`], [`Violation::UnexpectedWitnessImpl`], [`Violation::WitnessFieldNotPrivate`] | `publication.rs` | the witnesses stay linear (no `Clone`, no `Default`) and gain no constructor through a new `impl` |
//! | [`Violation::WitnessHeldAcrossStatements`] | every crate | every `.commit_content()` is chained directly into `?.commit_index()`, so no consumer does other work between the two commits |
//! | [`Violation::UnadjudicatedPublicationSite`], [`Violation::StalePublicationSite`] | every crate | the set of items that reach the write surface is closed ([`PUBLICATION_SITES`]) |
//!
//! # The mutants
//!
//! Each mutant is built **in this test**, as an in-memory edit of the real source text.
//! No file under `src/` is changed. Each edit's anchor is asserted to exist, so a mutant
//! cannot pass by editing nothing. The central one is the brief's own. It is a content
//! commit that also writes the index, so the artifact becomes visible before its receipt
//! (provenance) and its index commit. See
//! [`mutant_eager_visibility_at_the_content_commit_is_caught`]. The other mutants each
//! break one premise in the table above, and each must fail only the rule for that
//! premise.
//!
//! # Absences (INV-007)
//!
//! 1. **The daemon's volatile records are outside this guard.** An evidence node, a task
//!    record, or a workspace record in `DaemonState` can name a published handle. The type
//!    system does not tie that record to a `PublicationReceipt`. The daemon computes
//!    the name before it publishes, and the order of the two statements is a matter of
//!    discipline. G1-06 probes this behaviourally for every seam that exists today: an
//!    aborted publication leaves the namespace and the task record byte-identical. It also
//!    states that the daemon-side reference record is volatile (G1-06 scope statement 4).
//!    A type-level tie (a `Published<H>` newtype mintable only from a receipt) would touch
//!    the record types of five daemon families and their test fixtures. It is not done
//!    here.
//! 2. **Spelling, not types.** The census reads tokens, not resolved types. A write
//!    surface reached through a trait object, a macro, or a re-export under another name
//!    would escape it. None exists. `ReferenceStore` implements no trait with a write
//!    method, and the crate defines no macros.
//! 3. **No disk.** `ReferenceStore` is in memory. "Durably committed" is read as committed
//!    to the store that survives `Daemon::crash`, as G1-06 absence 4 reads it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

// =========================================================================================
// §1. A lexer that keeps structure and drops prose
// =========================================================================================

/// One token of Rust source. String and char literals collapse to one opaque token, and
/// comments do not survive, so prose and data can never be read as code.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    text: String,
    line: usize,
    kind: TokenKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Ident,
    Punct,
    Literal,
    Lifetime,
}

fn lex(source: &str) -> Vec<Token> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut at = 0;
    let mut line = 1;
    let peek = |index: usize| chars.get(index).copied();

    while at < chars.len() {
        let character = chars[at];
        let start_line = line;

        if character == '\n' {
            line += 1;
            at += 1;
            continue;
        }
        if character.is_whitespace() {
            at += 1;
            continue;
        }
        // Line comment, doc comments included.
        if character == '/' && peek(at + 1) == Some('/') {
            while at < chars.len() && chars[at] != '\n' {
                at += 1;
            }
            continue;
        }
        // Block comment, nested.
        if character == '/' && peek(at + 1) == Some('*') {
            let mut depth = 0usize;
            while at < chars.len() {
                if chars[at] == '/' && peek(at + 1) == Some('*') {
                    depth += 1;
                    at += 2;
                } else if chars[at] == '*' && peek(at + 1) == Some('/') {
                    depth -= 1;
                    at += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    if chars[at] == '\n' {
                        line += 1;
                    }
                    at += 1;
                }
            }
            continue;
        }
        if character.is_alphabetic() || character == '_' {
            let begin = at;
            while at < chars.len() && (chars[at].is_alphanumeric() || chars[at] == '_') {
                at += 1;
            }
            let word: String = chars[begin..at].iter().collect();
            let next = peek(at);
            // Raw strings: r"…", r#"…"#, br"…", cr#"…"#.
            if matches!(word.as_str(), "r" | "br" | "cr")
                && (next == Some('"') || (next == Some('#') && raw_string_follows(&chars, at)))
            {
                let mut hashes = 0;
                while peek(at) == Some('#') {
                    hashes += 1;
                    at += 1;
                }
                at += 1; // the opening quote
                loop {
                    match peek(at) {
                        None => break,
                        Some('"') if (1..=hashes).all(|k| peek(at + k) == Some('#')) => {
                            at += 1 + hashes;
                            break;
                        }
                        Some(c) => {
                            if c == '\n' {
                                line += 1;
                            }
                            at += 1;
                        }
                    }
                }
                tokens.push(literal(start_line));
                continue;
            }
            // Raw identifier: r#match.
            if word == "r" && next == Some('#') {
                at += 1;
                let begin = at;
                while at < chars.len() && (chars[at].is_alphanumeric() || chars[at] == '_') {
                    at += 1;
                }
                tokens.push(Token {
                    text: chars[begin..at].iter().collect(),
                    line: start_line,
                    kind: TokenKind::Ident,
                });
                continue;
            }
            // Byte and C strings, byte chars: b"…", c"…", b'…'.
            if matches!(word.as_str(), "b" | "c") && next == Some('"') {
                at = skip_quoted(&chars, at, '"', &mut line);
                tokens.push(literal(start_line));
                continue;
            }
            if word == "b" && next == Some('\'') {
                at = skip_quoted(&chars, at, '\'', &mut line);
                tokens.push(literal(start_line));
                continue;
            }
            tokens.push(Token {
                text: word,
                line: start_line,
                kind: TokenKind::Ident,
            });
            continue;
        }
        if character.is_ascii_digit() {
            while at < chars.len()
                && (chars[at].is_alphanumeric()
                    || chars[at] == '_'
                    || (chars[at] == '.' && peek(at + 1).is_some_and(|c| c.is_ascii_digit())))
            {
                at += 1;
            }
            tokens.push(literal(start_line));
            continue;
        }
        if character == '"' {
            at = skip_quoted(&chars, at, '"', &mut line);
            tokens.push(literal(start_line));
            continue;
        }
        if character == '\'' {
            // A char literal is '\…' or 'x'; anything else is a lifetime or a label.
            if peek(at + 1) == Some('\\') || peek(at + 2) == Some('\'') {
                at = skip_quoted(&chars, at, '\'', &mut line);
                tokens.push(literal(start_line));
            } else {
                at += 1;
                let begin = at;
                while at < chars.len() && (chars[at].is_alphanumeric() || chars[at] == '_') {
                    at += 1;
                }
                tokens.push(Token {
                    text: format!("'{}", chars[begin..at].iter().collect::<String>()),
                    line: start_line,
                    kind: TokenKind::Lifetime,
                });
            }
            continue;
        }
        tokens.push(Token {
            text: character.to_string(),
            line: start_line,
            kind: TokenKind::Punct,
        });
        at += 1;
    }
    tokens
}

fn raw_string_follows(chars: &[char], mut at: usize) -> bool {
    while chars.get(at) == Some(&'#') {
        at += 1;
    }
    chars.get(at) == Some(&'"')
}

/// Skip a quoted literal that opens at `at` (or one character later, for a prefix), and
/// return the index just past its closing quote.
fn skip_quoted(chars: &[char], mut at: usize, quote: char, line: &mut usize) -> usize {
    while chars.get(at) != Some(&quote) {
        at += 1;
    }
    at += 1;
    while let Some(&c) = chars.get(at) {
        match c {
            '\\' => at += 2,
            '\n' => {
                *line += 1;
                at += 1;
            }
            c if c == quote => return at + 1,
            _ => at += 1,
        }
    }
    at
}

const fn literal(line: usize) -> Token {
    Token {
        text: String::new(),
        line,
        kind: TokenKind::Literal,
    }
}

// =========================================================================================
// §2. Item attribution: which `impl` and which `fn` each token belongs to
// =========================================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
enum ItemKind {
    Impl {
        self_type: String,
        trait_name: Option<String>,
    },
    Fn(String),
    Mod(String),
    Struct(String),
    Other,
}

#[derive(Debug, Clone)]
struct Frame {
    kind: ItemKind,
    depth: usize,
    test: bool,
}

/// A token with the items that enclose it.
#[derive(Debug, Clone)]
struct Placed {
    token: Token,
    /// `Impl::fn` style path of the named enclosing items, e.g.
    /// `CommittedContent::commit_index`. Empty at module level.
    item: String,
    /// Inside a `#[cfg(test)]` item: unit tests, not a production path.
    test: bool,
    /// Inside a `fn`/`impl`/`struct` header, before its body opens.
    in_header: bool,
    /// The innermost `impl` self type, if any.
    impl_type: Option<String>,
    /// The innermost `struct` whose body this token is directly in.
    struct_body: Option<String>,
}

/// Everything the rules need to know about one file.
#[derive(Debug, Default)]
struct Parsed {
    tokens: Vec<Placed>,
    /// `struct` name → derives written on it.
    derives: BTreeMap<String, Vec<String>>,
    /// `struct` name → whether the definition is `pub` (any form).
    struct_is_pub: BTreeMap<String, bool>,
    /// Every `impl` header: (self type, trait, is test).
    impls: Vec<(String, Option<String>, bool)>,
    /// Every non-test `mod` with a body.
    child_modules: Vec<String>,
}

fn parse(source: &str) -> Parsed {
    let tokens = lex(source);
    let mut parsed = Parsed::default();
    let mut frames: Vec<Frame> = Vec::new();
    let mut pending: Option<ItemKind> = None;
    let mut pending_test = false;
    let mut derives: Vec<String> = Vec::new();
    let mut nesting = 0usize; // ( and [ depth, for `;` inside a header
    let mut depth = 0usize;
    let mut index = 0;

    while index < tokens.len() {
        let token = &tokens[index];
        let text = token.text.as_str();
        let next = tokens.get(index + 1);

        // Attributes: record `#[cfg(test)]` and `#[derive(…)]`, and skip the rest.
        if text == "#"
            && (next.is_some_and(|n| n.text == "[")
                || (next.is_some_and(|n| n.text == "!")
                    && tokens.get(index + 2).is_some_and(|n| n.text == "[")))
        {
            let open = if next.is_some_and(|n| n.text == "!") {
                index + 2
            } else {
                index + 1
            };
            let mut close = open;
            let mut brackets = 0usize;
            while close < tokens.len() {
                match tokens[close].text.as_str() {
                    "[" => brackets += 1,
                    "]" => {
                        brackets -= 1;
                        if brackets == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                close += 1;
            }
            let body: Vec<&str> = tokens[open + 1..close]
                .iter()
                .map(|t| t.text.as_str())
                .collect();
            if body == ["cfg", "(", "test", ")"] {
                pending_test = true;
            }
            if body.first() == Some(&"derive") {
                derives.extend(
                    body.iter()
                        .filter(|t| t.chars().next().is_some_and(char::is_alphabetic))
                        .skip(1)
                        .map(|t| (*t).to_owned()),
                );
            }
            for t in &tokens[index..=close.min(tokens.len() - 1)] {
                parsed.tokens.push(place(t, &frames, pending.is_some()));
            }
            index = close + 1;
            continue;
        }

        match text {
            "fn" if pending.is_none() && next.is_some_and(|n| n.kind == TokenKind::Ident) => {
                pending = Some(ItemKind::Fn(
                    next.map(|n| n.text.clone()).unwrap_or_default(),
                ));
            }
            "mod" if next.is_some_and(|n| n.kind == TokenKind::Ident) => {
                pending = Some(ItemKind::Mod(
                    next.map(|n| n.text.clone()).unwrap_or_default(),
                ));
            }
            "struct" | "enum" | "union" | "trait"
                if pending.is_none() && next.is_some_and(|n| n.kind == TokenKind::Ident) =>
            {
                let name = next.map(|n| n.text.clone()).unwrap_or_default();
                let previous = index.checked_sub(1).map(|p| tokens[p].text.as_str());
                if text == "struct" {
                    parsed
                        .derives
                        .insert(name.clone(), std::mem::take(&mut derives));
                    parsed
                        .struct_is_pub
                        .insert(name.clone(), matches!(previous, Some("pub" | ")")));
                    pending = Some(ItemKind::Struct(name));
                } else {
                    pending = Some(ItemKind::Other);
                }
            }
            "impl" if pending.is_none() => {
                let (self_type, trait_name) = impl_header(&tokens, index);
                pending = Some(ItemKind::Impl {
                    self_type,
                    trait_name,
                });
            }
            _ => {}
        }

        match text {
            "(" | "[" => nesting += 1,
            ")" | "]" => nesting = nesting.saturating_sub(1),
            _ => {}
        }

        let in_header = pending.is_some();
        parsed.tokens.push(place(token, &frames, in_header));

        match text {
            "{" => {
                depth += 1;
                let parent_test = frames.last().is_some_and(|frame| frame.test);
                let kind = pending.take().unwrap_or(ItemKind::Other);
                let test = parent_test || (pending_test && kind != ItemKind::Other);
                if let ItemKind::Impl {
                    self_type,
                    trait_name,
                } = &kind
                {
                    parsed
                        .impls
                        .push((self_type.clone(), trait_name.clone(), test));
                }
                if let ItemKind::Mod(name) = &kind
                    && !test
                {
                    parsed.child_modules.push(name.clone());
                }
                if kind != ItemKind::Other {
                    pending_test = false;
                    derives.clear();
                }
                frames.push(Frame { kind, depth, test });
            }
            "}" => {
                if frames.last().is_some_and(|frame| frame.depth == depth) {
                    frames.pop();
                }
                depth = depth.saturating_sub(1);
            }
            ";" if nesting == 0 => {
                pending = None;
                pending_test = false;
                derives.clear();
            }
            _ => {}
        }
        index += 1;
    }
    parsed
}

fn place(token: &Token, frames: &[Frame], in_header: bool) -> Placed {
    let item = frames
        .iter()
        .filter_map(|frame| match &frame.kind {
            ItemKind::Impl { self_type, .. } => Some(self_type.clone()),
            ItemKind::Fn(name) | ItemKind::Mod(name) => Some(name.clone()),
            ItemKind::Struct(_) | ItemKind::Other => None,
        })
        .collect::<Vec<_>>()
        .join("::");
    let impl_type = frames.iter().rev().find_map(|frame| match &frame.kind {
        ItemKind::Impl { self_type, .. } => Some(self_type.clone()),
        _ => None,
    });
    let struct_body = frames.last().and_then(|frame| match &frame.kind {
        ItemKind::Struct(name) => Some(name.clone()),
        _ => None,
    });
    Placed {
        token: token.clone(),
        item,
        test: frames.iter().any(|frame| frame.test),
        in_header,
        impl_type,
        struct_body,
    }
}

/// The self type and trait of the `impl` header starting at `at`.
fn impl_header(tokens: &[Token], at: usize) -> (String, Option<String>) {
    let mut cursor = at + 1;
    // Skip the header's own generics.
    if tokens.get(cursor).is_some_and(|t| t.text == "<") {
        let mut angles = 0usize;
        while let Some(token) = tokens.get(cursor) {
            match token.text.as_str() {
                "<" => angles += 1,
                ">" => {
                    angles -= 1;
                    if angles == 0 {
                        cursor += 1;
                        break;
                    }
                }
                _ => {}
            }
            cursor += 1;
        }
    }
    let header_end = (cursor..tokens.len())
        .find(|&i| matches!(tokens[i].text.as_str(), "{" | ";" | "where"))
        .unwrap_or(tokens.len());
    let mut angles = 0usize;
    let mut for_at = None;
    for (i, token) in tokens.iter().enumerate().take(header_end).skip(cursor) {
        match token.text.as_str() {
            "<" => angles += 1,
            ">" => angles = angles.saturating_sub(1),
            "for" if angles == 0 => for_at = Some(i),
            _ => {}
        }
    }
    let path_last = |from: usize| {
        let mut last = String::new();
        for token in &tokens[from..header_end] {
            match token.kind {
                TokenKind::Ident if token.text == "for" => break,
                TokenKind::Ident => last.clone_from(&token.text),
                _ if token.text == ":" => {}
                _ => break,
            }
        }
        last
    };
    match for_at {
        Some(for_at) => (path_last(for_at + 1), Some(path_last(cursor))),
        None => (path_last(cursor), None),
    }
}

// =========================================================================================
// §3. The rules
// =========================================================================================

/// One broken premise of the type-level guard. Each variant names the rule, where it broke,
/// and what was found. It is never a bare boolean (INV-008).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Violation {
    /// `StoreState`, its fields, or `ReferenceStore::state` became reachable from outside
    /// `publication.rs`, or the module grew a non-test child module that can see them.
    StateNotPrivate { what: String },
    /// The index was written outside `CommittedContent::commit_index`.
    VisibilityWriteOutsideWitness { item: String, line: usize },
    /// The receipt ledger was written outside `CommittedContent::commit_index`.
    ProvenanceWriteOutsideWitness { item: String, line: usize },
    /// The index write and the ledger write are not one critical section.
    VisibilityAndProvenanceNotOneCriticalSection { item: String, detail: String },
    /// Content was written outside the content commit (or removed outside collection).
    ContentWriteOutsideContentCommit {
        item: String,
        op: String,
        line: usize,
    },
    /// A content read that serves bytes was not preceded by an index lookup.
    ContentServedWithoutIndex { item: String, line: usize },
    /// A witness type was constructed or destructured outside its sanctioned site.
    WitnessMintedOutsideCommitPath {
        file: String,
        witness: String,
        item: String,
        line: usize,
    },
    /// A witness type derives a trait that would let it be forged or duplicated.
    ForbiddenDerive { witness: String, derive: String },
    /// A witness type gained an `impl` outside the closed set.
    UnexpectedWitnessImpl {
        witness: String,
        trait_name: Option<String>,
    },
    /// A witness type has a public field.
    WitnessFieldNotPrivate { witness: String, line: usize },
    /// `.commit_content()` is not chained directly into `?.commit_index()`.
    WitnessHeldAcrossStatements {
        file: String,
        item: String,
        line: usize,
    },
    /// A publication call at an item [`PUBLICATION_SITES`] does not list.
    UnadjudicatedPublicationSite {
        file: String,
        item: String,
        call: &'static str,
        line: usize,
    },
    /// A [`PUBLICATION_SITES`] row whose call no longer exists.
    StalePublicationSite {
        file: String,
        item: String,
        call: &'static str,
    },
}

/// The file the store's state machine lives in, relative to `crates/`.
const PUBLICATION_RS: &str = "continuum-workspace/src/publication.rs";

/// The store's three state maps whose writes decide visibility, provenance, and content.
const INDEX: &str = "index";
const LEDGER: &str = "ledger";
const CONTENT: &str = "content";

/// The sanctioned writers.
const INDEX_WRITER: &str = "CommittedContent::commit_index";
const CONTENT_WRITER: &str = "StagedPublication::commit_content";
const CONTENT_COLLECTOR: &str = "ReferenceStore::collect_garbage";

/// Methods on a `BTreeMap` that do not change it. Any other method counts as a write, so an
/// unfamiliar method fails closed.
const READ_METHODS: &[&str] = &[
    "get",
    "values",
    "keys",
    "len",
    "is_empty",
    "iter",
    "clone",
    "contains_key",
    "first_key_value",
    "last_key_value",
    "range",
];

/// Each witness type, and the one item that may construct it.
const WITNESS_MINTS: &[(&str, &str)] = &[
    ("StagedPublication", "ReferenceStore::stage"),
    ("CommittedContent", "StagedPublication::commit_content"),
    ("PublicationReceipt", "CommittedContent::commit_index"),
    ("StoreState", "ReferenceStoreBuilder::build"),
];

/// The closed set of `impl` blocks each witness type may have. A new inherent method is
/// still inherent, and the mint rule catches any new constructor. A trait `impl` such as
/// `Default` or `From` is a constructor by another name, so it is not in the set.
const WITNESS_IMPLS: &[(&str, &[Option<&str>])] = &[
    ("StagedPublication", &[None, Some("Debug"), Some("Drop")]),
    ("CommittedContent", &[None, Some("Debug"), Some("Drop")]),
    ("PublicationReceipt", &[None]),
    ("StoreState", &[]),
];

/// Derives that would forge a witness (`Default`, deserialization) or duplicate a linear
/// one (`Clone`/`Copy` on the two state-machine states).
const FORBIDDEN_DERIVES: &[(&str, &[&str])] = &[
    (
        "StagedPublication",
        &["Clone", "Copy", "Default", "Deserialize"],
    ),
    (
        "CommittedContent",
        &["Clone", "Copy", "Default", "Deserialize"],
    ),
    ("PublicationReceipt", &["Default", "Deserialize"]),
    ("StoreState", &["Clone", "Copy", "Deserialize"]),
];

/// The receiver-independent spellings of the store's write surface, as token sequences.
///
/// `ReferenceStore` has two write entry points, `stage` and `publish`. A staged publication
/// has no effect until `.commit_content()`, so that call and `.publish(` together cover the
/// store. `SealedWorkspace::seal` and `seal_current` are the crate's two public wrappers
/// over `publish`.
const PUBLICATION_CALLS: &[(&str, &[&str])] = &[
    (".commit_content(", &[".", "commit_content", "("]),
    (".publish(", &[".", "publish", "("]),
    (
        "SealedWorkspace::seal(",
        &["SealedWorkspace", ":", ":", "seal", "("],
    ),
    ("seal_current(", &["seal_current", "("]),
];

/// The closed set of items, across every crate's `src`, that reach the store's write
/// surface. Each row is `(file relative to crates/, enclosing item, call)`.
///
/// G1-06 adjudicates the daemon's rows per *operation*. This table adjudicates every
/// crate's rows per *item*, and it is what makes a new publisher fail here even outside
/// the daemon.
const PUBLICATION_SITES: &[(&str, &str, &str)] = &[
    // The store's own convenience spelling of the three steps.
    (
        PUBLICATION_RS,
        "ReferenceStore::publish",
        ".commit_content(",
    ),
    // The crate's two wrappers: one record per `publish`, children before parents.
    (
        "continuum-workspace/src/seal.rs",
        "SealedWorkspace::seal",
        ".publish(",
    ),
    (
        "continuum-workspace/src/staleness.rs",
        "seal_current",
        "SealedWorkspace::seal(",
    ),
    // The daemon: campaign and terminal records, the continuation record, the two evidence
    // appends, and the two workspace seals.
    (
        "continuumd/src/daemon/verification.rs",
        "publish_record",
        ".commit_content(",
    ),
    (
        "continuumd/src/daemon/continuation.rs",
        "publish",
        ".commit_content(",
    ),
    ("continuumd/src/daemon/observe.rs", "ingest", ".publish("),
    ("continuumd/src/daemon/evidence.rs", "link", ".publish("),
    (
        "continuumd/src/daemon/workspace.rs",
        "seal",
        "seal_current(",
    ),
    (
        "continuumd/src/daemon/workspace.rs",
        "publish",
        "SealedWorkspace::seal(",
    ),
];

/// Where a field access sits, and whether it writes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Access {
    item: String,
    field: &'static str,
    op: String,
    write: bool,
    line: usize,
    position: usize,
}

/// Every non-test `.index`, `.ledger`, and `.content` access in `publication.rs`.
fn accesses(parsed: &Parsed) -> Vec<Access> {
    let tokens = &parsed.tokens;
    let mut found = Vec::new();
    for (position, placed) in tokens.iter().enumerate() {
        if placed.test || placed.token.text != "." {
            continue;
        }
        let Some(field_token) = tokens.get(position + 1) else {
            continue;
        };
        let field = match field_token.token.text.as_str() {
            "index" => INDEX,
            "ledger" => LEDGER,
            "content" => CONTENT,
            _ => continue,
        };
        // `x.content(` is a method named `content`, not the field.
        let after = |k: usize| tokens.get(position + k).map(|t| t.token.text.as_str());
        if after(2) == Some("(") {
            continue;
        }
        let borrowed_mut = position >= 3
            && tokens[position - 3].token.text == "&"
            && tokens[position - 2].token.text == "mut";
        let (op, write) = match (after(2), after(3), after(4)) {
            (Some("."), Some(method), Some("(")) => {
                (method.to_owned(), !READ_METHODS.contains(&method))
            }
            (Some("="), next, _) if next != Some("=") => ("=".to_owned(), true),
            _ if borrowed_mut => ("&mut".to_owned(), true),
            _ => ("borrow".to_owned(), false),
        };
        found.push(Access {
            item: placed.item.clone(),
            field,
            op,
            write,
            line: placed.token.line,
            position,
        });
    }
    found
}

/// The store-internal rules, over `publication.rs` alone.
fn check_store(parsed: &Parsed) -> Vec<Violation> {
    let mut violations = Vec::new();

    // Premise: the maps are private to this file, so this census is complete.
    if parsed.struct_is_pub.get("StoreState").copied() != Some(false) {
        violations.push(Violation::StateNotPrivate {
            what: "`StoreState` is public, or not found".to_owned(),
        });
    }
    for placed in &parsed.tokens {
        if placed.token.text == "pub"
            && matches!(
                placed.struct_body.as_deref(),
                Some("StoreState" | "ReferenceStore")
            )
        {
            violations.push(Violation::StateNotPrivate {
                what: format!(
                    "a public field in `{}` at line {}",
                    placed.struct_body.as_deref().unwrap_or_default(),
                    placed.token.line
                ),
            });
        }
    }
    for module in &parsed.child_modules {
        violations.push(Violation::StateNotPrivate {
            what: format!("non-test child module `{module}` can see private fields"),
        });
    }

    let all = accesses(parsed);
    for access in all.iter().filter(|access| access.write) {
        match access.field {
            f if f == INDEX && access.item != INDEX_WRITER => {
                violations.push(Violation::VisibilityWriteOutsideWitness {
                    item: access.item.clone(),
                    line: access.line,
                });
            }
            f if f == LEDGER && access.item != INDEX_WRITER => {
                violations.push(Violation::ProvenanceWriteOutsideWitness {
                    item: access.item.clone(),
                    line: access.line,
                });
            }
            f if f == CONTENT => {
                let sanctioned = (access.item == CONTENT_WRITER
                    && matches!(access.op.as_str(), "insert" | "take"))
                    || (access.item == CONTENT_COLLECTOR && access.op == "remove");
                if !sanctioned {
                    violations.push(Violation::ContentWriteOutsideContentCommit {
                        item: access.item.clone(),
                        op: access.op.clone(),
                        line: access.line,
                    });
                }
            }
            _ => {}
        }
    }

    // One critical section: between the index write and the ledger write in the witness's
    // method, nothing releases or re-takes the store lock.
    let index_write = all
        .iter()
        .find(|a| a.write && a.field == INDEX && a.item == INDEX_WRITER);
    let ledger_write = all
        .iter()
        .find(|a| a.write && a.field == LEDGER && a.item == INDEX_WRITER);
    match (index_write, ledger_write) {
        (Some(index), Some(ledger)) => {
            let (from, to) = if index.position < ledger.position {
                (index.position, ledger.position)
            } else {
                (ledger.position, index.position)
            };
            let between = &parsed.tokens[from..to];
            let releases = between.iter().any(|t| t.token.text == "drop");
            let retakes = between.windows(3).any(|w| {
                w[0].token.text == "." && w[1].token.text == "state" && w[2].token.text == "("
            });
            if releases || retakes {
                violations.push(Violation::VisibilityAndProvenanceNotOneCriticalSection {
                    item: INDEX_WRITER.to_owned(),
                    detail: format!(
                        "lines {}–{}: the store lock is {} between the two writes",
                        index.line.min(ledger.line),
                        index.line.max(ledger.line),
                        if releases { "released" } else { "re-taken" }
                    ),
                });
            }
        }
        _ => violations.push(Violation::VisibilityAndProvenanceNotOneCriticalSection {
            item: INDEX_WRITER.to_owned(),
            detail: "the witness's method does not write both the index and the ledger".to_owned(),
        }),
    }

    // A read that serves content bytes has consulted the index first, in the same item.
    for access in all
        .iter()
        .filter(|a| a.field == CONTENT && a.op == "get" && a.item != CONTENT_WRITER)
    {
        let gated = all.iter().any(|a| {
            a.field == INDEX
                && a.op == "get"
                && a.item == access.item
                && a.position < access.position
        });
        if !gated {
            violations.push(Violation::ContentServedWithoutIndex {
                item: access.item.clone(),
                line: access.line,
            });
        }
    }

    // Witness shape: derives, impls, field privacy.
    for (witness, forbidden) in FORBIDDEN_DERIVES {
        for derive in parsed.derives.get(*witness).into_iter().flatten() {
            if forbidden.contains(&derive.as_str()) {
                violations.push(Violation::ForbiddenDerive {
                    witness: (*witness).to_owned(),
                    derive: derive.clone(),
                });
            }
        }
    }
    for (self_type, trait_name, test) in &parsed.impls {
        if *test {
            continue;
        }
        if let Some((_, allowed)) = WITNESS_IMPLS.iter().find(|(w, _)| w == self_type)
            && !allowed.contains(&trait_name.as_deref())
        {
            violations.push(Violation::UnexpectedWitnessImpl {
                witness: self_type.clone(),
                trait_name: trait_name.clone(),
            });
        }
    }
    for placed in &parsed.tokens {
        if placed.token.text == "pub"
            && let Some(witness) = placed.struct_body.as_deref()
            && [
                "StagedPublication",
                "CommittedContent",
                "PublicationReceipt",
            ]
            .contains(&witness)
        {
            violations.push(Violation::WitnessFieldNotPrivate {
                witness: witness.to_owned(),
                line: placed.token.line,
            });
        }
    }

    violations.sort();
    violations
}

/// Construction or destructuring of a witness: `Name {` (or `Self {` inside its `impl`)
/// outside a header.
fn witness_literals(file: &str, parsed: &Parsed) -> Vec<Violation> {
    let tokens = &parsed.tokens;
    let mut violations = Vec::new();
    for (position, placed) in tokens.iter().enumerate() {
        if placed.test || placed.in_header {
            continue;
        }
        if tokens.get(position + 1).map(|t| t.token.text.as_str()) != Some("{") {
            continue;
        }
        let named = placed.token.text.as_str();
        let witness = if named == "Self" {
            placed.impl_type.as_deref()
        } else {
            Some(named)
        };
        let Some(witness) = witness else { continue };
        let Some((_, mint)) = WITNESS_MINTS.iter().find(|(w, _)| *w == witness) else {
            continue;
        };
        if !(file == PUBLICATION_RS && placed.item == *mint) {
            violations.push(Violation::WitnessMintedOutsideCommitPath {
                file: file.to_owned(),
                witness: witness.to_owned(),
                item: placed.item.clone(),
                line: placed.token.line,
            });
        }
    }
    violations
}

/// A publication call found in a file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Site {
    file: String,
    item: String,
    call: &'static str,
    line: usize,
}

fn publication_sites(file: &str, parsed: &Parsed) -> (Vec<Site>, Vec<Violation>) {
    let tokens = &parsed.tokens;
    let mut sites = Vec::new();
    let mut violations = Vec::new();
    for position in 0..tokens.len() {
        if tokens[position].test {
            continue;
        }
        for (call, sequence) in PUBLICATION_CALLS {
            let matches = sequence.iter().enumerate().all(|(k, text)| {
                tokens
                    .get(position + k)
                    .is_some_and(|t| t.token.text == *text)
            });
            if !matches {
                continue;
            }
            // `fn seal_current(` is the definition, not a call.
            if position > 0 && tokens[position - 1].token.text == "fn" {
                continue;
            }
            sites.push(Site {
                file: file.to_owned(),
                item: tokens[position].item.clone(),
                call,
                line: tokens[position].token.line,
            });
            if *call == ".commit_content(" {
                let chain = [")", "?", ".", "commit_index", "("];
                let chained = chain.iter().enumerate().all(|(k, text)| {
                    tokens
                        .get(position + sequence.len() + k)
                        .is_some_and(|t| t.token.text == *text)
                });
                if !chained {
                    violations.push(Violation::WitnessHeldAcrossStatements {
                        file: file.to_owned(),
                        item: tokens[position].item.clone(),
                        line: tokens[position].token.line,
                    });
                }
            }
        }
    }
    (sites, violations)
}

/// Run every rule over a source tree given as `crates/`-relative path → text.
fn check_tree(tree: &BTreeMap<String, String>) -> (Vec<Site>, Vec<Violation>) {
    let mut sites = Vec::new();
    let mut violations = Vec::new();
    for (file, text) in tree {
        let parsed = parse(text);
        if file == PUBLICATION_RS {
            violations.extend(check_store(&parsed));
        }
        violations.extend(witness_literals(file, &parsed));
        let (found, held) = publication_sites(file, &parsed);
        sites.extend(found);
        violations.extend(held);
    }
    if !tree.contains_key(PUBLICATION_RS) {
        violations.push(Violation::StateNotPrivate {
            what: format!("`{PUBLICATION_RS}` is missing from the tree"),
        });
    }

    let adjudicated: BTreeSet<(&str, &str, &str)> = PUBLICATION_SITES.iter().copied().collect();
    for site in &sites {
        if !adjudicated.contains(&(site.file.as_str(), site.item.as_str(), site.call)) {
            violations.push(Violation::UnadjudicatedPublicationSite {
                file: site.file.clone(),
                item: site.item.clone(),
                call: site.call,
                line: site.line,
            });
        }
    }
    let found: BTreeSet<(&str, &str, &str)> = sites
        .iter()
        .map(|s| (s.file.as_str(), s.item.as_str(), s.call))
        .collect();
    for &(file, item, call) in PUBLICATION_SITES {
        if !found.contains(&(file, item, call)) {
            // Only report rows whose file is in the tree, so a partial tree under test is
            // judged on what it contains.
            if tree.contains_key(file) {
                let call = PUBLICATION_CALLS
                    .iter()
                    .map(|(c, _)| *c)
                    .find(|c| *c == call)
                    .unwrap_or("?");
                violations.push(Violation::StalePublicationSite {
                    file: file.to_owned(),
                    item: item.to_owned(),
                    call,
                });
            }
        }
    }

    sites.sort();
    violations.sort();
    (sites, violations)
}

// =========================================================================================
// §4. The real tree
// =========================================================================================

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crate lives under crates/")
        .to_path_buf()
}

/// Every `crates/*/src/**/*.rs` file, keyed by its path relative to `crates/`.
fn source_tree() -> BTreeMap<String, String> {
    let root = crates_dir();
    let mut tree = BTreeMap::new();
    let mut crates: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("crates/ is readable")
        .map(|entry| entry.expect("a readable entry").path().join("src"))
        .filter(|src| src.is_dir())
        .collect();
    crates.sort();
    let mut stack = crates;
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(&directory).expect("a readable source directory") {
            let path = entry.expect("a readable entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(std::ffi::OsStr::to_str) == Some("rs") {
                let relative = path
                    .strip_prefix(&root)
                    .expect("under crates/")
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = std::fs::read_to_string(&path).expect("a readable source file");
                tree.insert(relative, text);
            }
        }
    }
    tree
}

/// **Positive evidence.** The real tree meets every rule, and the census is not vacuous.
#[test]
fn the_source_tree_routes_every_visibility_through_the_committed_publication_path() {
    let tree = source_tree();
    assert!(
        tree.len() > 100,
        "the scan must see the whole workspace, not a fragment ({} files)",
        tree.len()
    );
    let (sites, violations) = check_tree(&tree);
    assert!(
        violations.is_empty(),
        "INV-017's architectural guard is broken. A new path makes an artifact visible \
         without the committed publication path, or a premise of the type-level guard \
         changed. Route it through `CommittedContent::commit_index`, or adjudicate it in \
         `PUBLICATION_SITES` with a reason:\n{violations:#?}"
    );
    assert_eq!(
        sites.len(),
        PUBLICATION_SITES.len(),
        "each adjudicated item reaches the write surface exactly once: {sites:#?}"
    );
    let crates: BTreeSet<&str> = sites
        .iter()
        .filter_map(|site| site.file.split('/').next())
        .collect();
    assert_eq!(
        crates,
        BTreeSet::from(["continuum-workspace", "continuumd"]),
        "only the store's own crate and the daemon publish"
    );
}

/// The store-internal accesses the rules are stated over, read off the source. This pins
/// that the rules see the real writes, so their silence on the real tree is not blindness.
#[test]
fn the_store_rules_see_the_real_writes() {
    let tree = source_tree();
    let parsed = parse(&tree[PUBLICATION_RS]);
    let writes: BTreeSet<(String, &str, String)> = accesses(&parsed)
        .into_iter()
        .filter(|access| access.write)
        .map(|access| (access.item, access.field, access.op))
        .collect();
    let expected: BTreeSet<(String, &str, String)> = [
        (INDEX_WRITER, INDEX, "entry"),
        (INDEX_WRITER, LEDGER, "entry"),
        (CONTENT_WRITER, CONTENT, "take"),
        (CONTENT_WRITER, CONTENT, "insert"),
        (CONTENT_COLLECTOR, CONTENT, "remove"),
    ]
    .into_iter()
    .map(|(item, field, op)| (item.to_owned(), field, op.to_owned()))
    .collect();
    assert_eq!(
        writes, expected,
        "the store writes its three maps at exactly these sites"
    );

    let reads: BTreeSet<(String, &str, String)> = accesses(&parsed)
        .into_iter()
        .filter(|access| access.item == "ReferenceStore::read")
        .map(|access| (access.item, access.field, access.op))
        .collect();
    assert_eq!(
        reads,
        [
            ("ReferenceStore::read".to_owned(), INDEX, "get".to_owned()),
            ("ReferenceStore::read".to_owned(), CONTENT, "get".to_owned()),
        ]
        .into_iter()
        .collect(),
        "the read path consults the index, then the content"
    );

    let impls: BTreeSet<(String, Option<String>)> = parsed
        .impls
        .iter()
        .filter(|(_, _, test)| !test)
        .filter(|(self_type, _, _)| WITNESS_IMPLS.iter().any(|(w, _)| w == self_type))
        .map(|(self_type, trait_name, _)| (self_type.clone(), trait_name.clone()))
        .collect();
    assert_eq!(
        impls.len(),
        7,
        "three impls on each state and one on the receipt: {impls:#?}"
    );
}

// =========================================================================================
// §5. Mutants: each built in-test from the real source, each caught by its own rule
// =========================================================================================

/// The real tree with one in-memory edit. The anchor must occur exactly once, so a mutant
/// cannot "pass" because its edit silently matched nothing.
fn mutate(file: &str, anchor: &str, replacement: &str) -> BTreeMap<String, String> {
    let mut tree = source_tree();
    let text = tree.get_mut(file).expect("the mutated file is in the tree");
    assert_eq!(
        text.matches(anchor).count(),
        1,
        "the mutant's anchor must occur exactly once in {file}"
    );
    *text = text.replacen(anchor, replacement, 1);
    tree
}

fn violations_of(tree: &BTreeMap<String, String>) -> Vec<Violation> {
    check_tree(tree).1
}

/// **The brief's mutant.** A content commit that also writes the index. The artifact is
/// readable the moment its bytes land, before `commit_index` appends its receipt. So it
/// is visible without provenance. If the index commit is then refused, it stays visible
/// with no receipt at all. That is exactly INV-017's violation. The guard names the item
/// and nothing else.
#[test]
fn mutant_eager_visibility_at_the_content_commit_is_caught() {
    let tree = mutate(
        PUBLICATION_RS,
        "                    state.content.insert(self.handle.clone(), content);\n",
        "                    state.content.insert(self.handle.clone(), content);\n\
         \x20                   if let Ok(path) = ArtifactPath::for_handle(&self.handle) {\n\
         \x20                       state.index.insert(path, self.handle.clone());\n\
         \x20                   }\n",
    );
    let violations = violations_of(&tree);
    assert_eq!(violations.len(), 1, "{violations:#?}");
    assert!(
        matches!(
            &violations[0],
            Violation::VisibilityWriteOutsideWitness { item, .. } if item == CONTENT_WRITER
        ),
        "{violations:#?}"
    );
}

/// A second publisher inside the store: a new entry point that writes the index and the
/// content directly, without the witness. It is caught twice: once for the index write,
/// and once for the content write.
#[test]
fn mutant_a_second_publish_entry_point_inside_the_store_is_caught() {
    let tree = mutate(
        PUBLICATION_RS,
        "    /// Stage, commit content, commit index.\n",
        "    /// A shortcut.\n\
         \x20   pub fn publish_eagerly(&self, handle: ArtifactHandle, content: Vec<u8>) {\n\
         \x20       let mut state = self.state();\n\
         \x20       if let Ok(path) = ArtifactPath::for_handle(&handle) {\n\
         \x20           state.index.insert(path, handle.clone());\n\
         \x20       }\n\
         \x20       state.content.insert(handle, content);\n\
         \x20   }\n\
         \n\
         \x20   /// Stage, commit content, commit index.\n",
    );
    let violations = violations_of(&tree);
    let item = "ReferenceStore::publish_eagerly".to_owned();
    assert!(
        violations.iter().any(|v| matches!(
            v,
            Violation::VisibilityWriteOutsideWitness { item: i, .. } if *i == item
        )),
        "{violations:#?}"
    );
    assert!(
        violations.iter().any(|v| matches!(
            v,
            Violation::ContentWriteOutsideContentCommit { item: i, op, .. }
                if *i == item && op == "insert"
        )),
        "{violations:#?}"
    );
    assert_eq!(violations.len(), 2, "{violations:#?}");
}

/// The index and the receipt ledger written under two lock acquisitions. A reader between
/// them sees an indexed artifact with no receipt.
#[test]
fn mutant_visibility_and_provenance_in_two_critical_sections_is_caught() {
    let tree = mutate(
        PUBLICATION_RS,
        "            .or_insert_with(|| self.handle.clone());\n        state\n            .ledger\n",
        "            .or_insert_with(|| self.handle.clone());\n        drop(state);\n        \
         let mut state = store.state();\n        state\n            .ledger\n",
    );
    let violations = violations_of(&tree);
    assert_eq!(violations.len(), 1, "{violations:#?}");
    assert!(
        matches!(
            &violations[0],
            Violation::VisibilityAndProvenanceNotOneCriticalSection { item, .. }
                if item == INDEX_WRITER
        ),
        "{violations:#?}"
    );
}

/// A read that serves content without consulting the index. Content that a crash left
/// unreachable, or that is still between its two commits, becomes visible.
#[test]
fn mutant_a_read_that_bypasses_the_index_is_caught() {
    let tree = mutate(
        PUBLICATION_RS,
        "        match state.index.get(&path) {\n            Some(indexed) if indexed == handle => {\n                state.content.get(handle).cloned().ok_or(CapabilityDenied)\n            }\n            _ => Err(CapabilityDenied),\n        }\n",
        "        let _ = path;\n        state.content.get(handle).cloned().ok_or(CapabilityDenied)\n",
    );
    let violations = violations_of(&tree);
    assert_eq!(violations.len(), 1, "{violations:#?}");
    assert!(
        matches!(
            &violations[0],
            Violation::ContentServedWithoutIndex { item, .. } if item == "ReferenceStore::read"
        ),
        "{violations:#?}"
    );
}

/// A receipt minted by a new constructor, spelled `Self { … }` so the rule must resolve
/// `Self` through the enclosing `impl`.
#[test]
fn mutant_a_forged_receipt_constructor_is_caught() {
    let tree = mutate(
        PUBLICATION_RS,
        "impl PublicationReceipt {\n",
        "impl PublicationReceipt {\n\
         \x20   /// Forge.\n\
         \x20   pub fn forge(handle: ArtifactHandle, actor: ActorId, capability: CapabilityToken) -> Self {\n\
         \x20       Self { handle, actor, capability, cost: PublicationCost::for_content(&[]) }\n\
         \x20   }\n",
    );
    let violations = violations_of(&tree);
    assert_eq!(violations.len(), 1, "{violations:#?}");
    assert!(
        matches!(
            &violations[0],
            Violation::WitnessMintedOutsideCommitPath { witness, item, .. }
                if witness == "PublicationReceipt" && item == "PublicationReceipt::forge"
        ),
        "{violations:#?}"
    );
}

/// The content-committed witness made duplicable, and given a `Default` constructor.
#[test]
fn mutant_a_duplicable_or_defaultable_witness_is_caught() {
    let tree = mutate(
        PUBLICATION_RS,
        "#[must_use = \"dropping committed content leaves unreachable content and no receipt\"]\npub struct CommittedContent<'store> {\n",
        "#[must_use = \"dropping committed content leaves unreachable content and no receipt\"]\n#[derive(Clone)]\npub struct CommittedContent<'store> {\n",
    );
    assert_eq!(
        violations_of(&tree),
        vec![Violation::ForbiddenDerive {
            witness: "CommittedContent".to_owned(),
            derive: "Clone".to_owned(),
        }]
    );

    let tree = mutate(
        PUBLICATION_RS,
        "impl PublicationReceipt {\n",
        "impl Default for PublicationReceipt {\n    fn default() -> Self { todo!() }\n}\n\nimpl PublicationReceipt {\n",
    );
    assert_eq!(
        violations_of(&tree),
        vec![Violation::UnexpectedWitnessImpl {
            witness: "PublicationReceipt".to_owned(),
            trait_name: Some("Default".to_owned()),
        }]
    );
}

/// The store's state made reachable from the rest of the crate. The census over
/// `publication.rs` would stop being complete, so the premise fails.
#[test]
fn mutant_leaking_the_store_state_is_caught() {
    let tree = mutate(
        PUBLICATION_RS,
        "#[derive(Debug, Default)]\nstruct StoreState {\n",
        "#[derive(Debug, Default)]\npub(crate) struct StoreState {\n",
    );
    let violations = violations_of(&tree);
    assert_eq!(violations.len(), 1, "{violations:#?}");
    assert!(matches!(&violations[0], Violation::StateNotPrivate { .. }));
}

/// A consumer that holds the witness across a statement. Between the two commits, any
/// work the daemon does can publish state that names content the index does not yet name.
#[test]
fn mutant_a_daemon_helper_holding_the_witness_is_caught() {
    let file = "continuumd/src/daemon/continuation.rs";
    let tree = mutate(
        file,
        ".and_then(|staged| Ok(staged.commit_content()?.commit_index()?))",
        ".and_then(|staged| {\n            let committed = staged.commit_content()?;\n            \
         Ok(committed.commit_index()?)\n        })",
    );
    let violations = violations_of(&tree);
    assert_eq!(violations.len(), 1, "{violations:#?}");
    assert!(
        matches!(
            &violations[0],
            Violation::WitnessHeldAcrossStatements { file: f, item, .. }
                if f == file && item == "publish"
        ),
        "{violations:#?}"
    );
}

/// A new publisher in a crate that has never published, on a binding not named `store`.
/// G1-06's census reads only the daemon, and only for `store.`-prefixed calls. This one
/// reads every crate, and it matches the call whatever the receiver is called.
#[test]
fn mutant_an_unadjudicated_publisher_in_another_crate_is_caught() {
    let mut tree = source_tree();
    tree.insert(
        "continuum-cli/src/sneak.rs".to_owned(),
        "pub fn sneak(s: &ReferenceStore, t: &CapabilityToken) {\n    \
         let _ = s\n        .publish(ArtifactClass::Evidence, b\"x\".to_vec(), t);\n}\n"
            .to_owned(),
    );
    assert_eq!(
        violations_of(&tree),
        vec![Violation::UnadjudicatedPublicationSite {
            file: "continuum-cli/src/sneak.rs".to_owned(),
            item: "sneak".to_owned(),
            call: ".publish(",
            line: 3,
        }]
    );
}

/// A consumer that forges the witness outside the store's crate. The compiler already
/// refuses this, because the fields are private. The `compile_fail` doctests on
/// `PublicationReceipt` and `CommittedContent` pin that. The census still reports it, so a
/// future `pub` field cannot turn it into a silent path.
#[test]
fn mutant_a_receipt_literal_in_the_daemon_is_caught() {
    let file = "continuumd/src/daemon/continuation.rs";
    let tree = mutate(
        file,
        "    if receipt.handle() != &named {\n",
        "    let _forged = PublicationReceipt { handle: named.clone() };\n    \
         if receipt.handle() != &named {\n",
    );
    let violations = violations_of(&tree);
    assert_eq!(violations.len(), 1, "{violations:#?}");
    assert!(matches!(
        &violations[0],
        Violation::WitnessMintedOutsideCommitPath { file: f, witness, .. }
            if f == file && witness == "PublicationReceipt"
    ));
}

// =========================================================================================
// §6. Controls: the instrument reads code, not prose, and survives formatting
// =========================================================================================

/// Comments, doc comments, strings, raw strings and char literals that spell every sink
/// are not sites. Lifetimes do not open char literals.
#[test]
fn control_prose_and_literals_are_not_code() {
    let source = r##"
        // state.index.insert(path, handle);
        /// PublicationReceipt { handle }
        /* nested /* state.ledger.entry(x) */ still a comment: s.publish( */
        fn quiet<'store>(x: &'store str) -> char {
            let _ = "state.index.insert(path, handle); s.publish(";
            let _ = r#"PublicationReceipt { "quoted" } .commit_content()"#;
            let _ = b"seal_current(";
            let _ = '{';
            let _ = '\'';
            'x'
        }
    "##;
    let parsed = parse(source);
    assert!(accesses(&parsed).is_empty());
    assert!(witness_literals("x.rs", &parsed).is_empty());
    let (sites, violations) = publication_sites("x.rs", &parsed);
    assert!(
        sites.is_empty() && violations.is_empty(),
        "{sites:?} {violations:?}"
    );
    assert_eq!(
        parsed.tokens.last().map(|t| t.token.text.as_str()),
        Some("}"),
        "the braces balance, so no literal swallowed the closing brace"
    );
}

/// **Boundary.** `rustfmt` splits a chain across lines, and the chain rule must still see
/// it as one expression. A chain that is split by a statement is not one.
#[test]
fn boundary_a_formatted_chain_is_one_expression_and_a_split_one_is_not() {
    let formatted = "fn f() { let r = staged\n    .commit_content()?\n    .commit_index()?; }";
    let (sites, held) = publication_sites("f.rs", &parse(formatted));
    assert_eq!(sites.len(), 1);
    assert!(held.is_empty(), "{held:?}");

    let split = "fn f() { let c = staged.commit_content()?; c.commit_index() }";
    let (_, held) = publication_sites("f.rs", &parse(split));
    assert_eq!(held.len(), 1);

    // An unfinished chain: content committed, the witness returned to the caller.
    let returned = "fn f() -> R { staged.commit_content() }";
    let (_, held) = publication_sites("f.rs", &parse(returned));
    assert_eq!(held.len(), 1);
}

/// **Boundary.** Items attribute through generic `impl` headers, trait `impl`s, and
/// `#[cfg(test)]` modules, which are excluded.
#[test]
fn boundary_item_attribution_reads_impl_headers_and_skips_unit_tests() {
    let source = "
        impl<'store> StagedPublication<'store> { fn commit_content(self) { x.index.insert(1); } }
        impl Drop for CommittedContent<'_> { fn drop(&mut self) { y.ledger.clear(); } }
        #[cfg(test)]
        mod tests { fn t() { z.index.insert(2); } }
        fn free(v: [u8; 4]) { w.content.remove(3); }
    ";
    let parsed = parse(source);
    let found: Vec<(String, &str, String)> = accesses(&parsed)
        .into_iter()
        .map(|a| (a.item, a.field, a.op))
        .collect();
    assert_eq!(
        found,
        vec![
            (
                "StagedPublication::commit_content".to_owned(),
                INDEX,
                "insert".to_owned()
            ),
            (
                "CommittedContent::drop".to_owned(),
                LEDGER,
                "clear".to_owned()
            ),
            ("free".to_owned(), CONTENT, "remove".to_owned()),
        ]
    );
    assert!(
        parsed.child_modules.is_empty(),
        "the test module is not a child"
    );
    assert!(parsed.impls.contains(&(
        "CommittedContent".to_owned(),
        Some("Drop".to_owned()),
        false
    )));
}
