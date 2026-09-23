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
//! | [`Violation::UnadjudicatedBareHandleField`], [`Violation::StaleHandleFieldAdjudication`], [`Violation::ReceiptTiedFieldLost`], [`Violation::DaemonRecordMissing`] | the daemon's records ([`DAEMON_RECORDS`]) and every daemon type they hold at any depth ([`walk`]) | a record that names a published artifact holds `Published<H>`, which only a receipt mints ([`RECEIPT_TIED_FIELDS`]), and every bare handle field — struct, tuple or enum variant, nested or direct — is adjudicated as not claiming a publication ([`BARE_HANDLE_FIELDS`]); the walk stops only at `Published<…>` and at the members [`OPAQUE_FIELDS`] names (bn-283p6, cr-2n0xng) |
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
//! 1. **Closed by bn-283p6: the daemon's volatile records are tied to a receipt.** A
//!    record that says an artifact is published holds `Published<H>`, which
//!    `Published::attest` and `Published::of` mint only from a `PublicationReceipt`:
//!    `WorkspaceRecord::seal` (was `sealed: bool`), `EvidenceNode::publication`,
//!    `Publication::commitment`, the continuation record's durable revision, and every
//!    identity a startup resolution (`ResolvedTask`) claims, restores from, or emits
//!    (cr-2n0xng). The record census keeps the set closed: it walks every daemon type the
//!    listed records hold at any depth, enum variants and tuple fields included, not only
//!    their direct fields. The walk reads only `continuumd`'s own types: a type from another
//!    crate (`continuum_task`'s ledger, `continuum_context`'s items) is a leaf. What remains
//!    is narrower. A `Published<H>` proves that
//!    *a* store committed the name, not that this daemon's store did: a value carried from
//!    another store type-checks (G1-08's orphan plant does exactly this). The
//!    `PublishedName` impls that compare a daemon spelling with a store handle are
//!    reviewed code, not census-checked. The records stay volatile (G1-06 scope
//!    statement 4).
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
    /// A daemon record field of bare handle type that [`BARE_HANDLE_FIELDS`] does not
    /// adjudicate. A record that names a published artifact holds `Published<H>`.
    UnadjudicatedBareHandleField {
        file: String,
        record: String,
        field: String,
        line: usize,
    },
    /// A [`BARE_HANDLE_FIELDS`] row that no longer names a bare handle field.
    StaleHandleFieldAdjudication {
        file: String,
        record: String,
        field: String,
    },
    /// A [`RECEIPT_TIED_FIELDS`] row whose field is gone, or no longer holds `Published<H>`.
    ReceiptTiedFieldLost {
        file: String,
        record: String,
        field: String,
    },
    /// A [`DAEMON_RECORDS`] row whose record type is not in its file.
    DaemonRecordMissing { file: String, record: String },
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

/// Each witness type, and the items that may construct it.
///
/// `Published` (bn-283p6) has two: `attest` checks a name against a receipt, and `of` takes
/// the receipt's own handle. Both take a `&PublicationReceipt`, so both are downstream of
/// `CommittedContent::commit_index`.
const WITNESS_MINTS: &[(&str, &[&str])] = &[
    ("StagedPublication", &["ReferenceStore::stage"]),
    ("CommittedContent", &["StagedPublication::commit_content"]),
    ("PublicationReceipt", &["CommittedContent::commit_index"]),
    ("StoreState", &["ReferenceStoreBuilder::build"]),
    ("Published", &["Published::attest", "Published::of"]),
];

/// The closed set of `impl` blocks each witness type may have. A new inherent method is
/// still inherent, and the mint rule catches any new constructor. A trait `impl` such as
/// `Default` or `From` is a constructor by another name, so it is not in the set.
const WITNESS_IMPLS: &[(&str, &[Option<&str>])] = &[
    ("StagedPublication", &[None, Some("Debug"), Some("Drop")]),
    ("CommittedContent", &[None, Some("Debug"), Some("Drop")]),
    ("PublicationReceipt", &[None]),
    ("StoreState", &[]),
    ("Published", &[None]),
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
    ("Published", &["Default", "Deserialize"]),
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
                "Published",
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
        let Some((_, mints)) = WITNESS_MINTS.iter().find(|(w, _)| *w == witness) else {
            continue;
        };
        if !(file == PUBLICATION_RS && mints.contains(&placed.item.as_str())) {
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

// -----------------------------------------------------------------------------------------
// The daemon's records: a name of a published artifact is a `Published<H>` (bn-283p6)
// -----------------------------------------------------------------------------------------

const STATE_RS: &str = "continuumd/src/daemon/state.rs";
const BUDGET_RS: &str = "continuumd/src/daemon/budget.rs";
const TASK_RS: &str = "continuumd/src/daemon/task.rs";
const CONTINUATION_RS: &str = "continuumd/src/daemon/continuation.rs";
const TERMINAL_RS: &str = "continuumd/src/daemon/terminal.rs";
const RECOVERY_RS: &str = "continuumd/src/daemon/recovery.rs";
const CONTEXT_RS: &str = "continuumd/src/daemon/context.rs";
const VERIFICATION_RS: &str = "continuumd/src/daemon/verification.rs";
const ENVELOPE_RS: &str = "continuumd/src/protocol/envelope.rs";
const WIRE_TASK_RS: &str = "continuumd/src/protocol/task.rs";

/// The crate whose types the record walk descends into.
const DAEMON_CRATE: &str = "continuumd/src/";

/// The roots of the record walk ([`walk`]): the daemon record types of the five families
/// that publish — workspace, task (campaign records), continuation, terminal, and evidence
/// (`observe.ingest`, `evidence.link`) — the state and table that hold them, the value a
/// restore hands the table, and the startup report. Every member of these types, and of
/// every daemon type a member names at any depth, is read (cr-2n0xng). Each root must
/// exist.
const DAEMON_RECORDS: &[(&str, &str)] = &[
    (STATE_RS, "DaemonState"),
    (STATE_RS, "WorkspaceRecord"),
    (STATE_RS, "EvidenceNode"),
    (STATE_RS, "EvidenceEdge"),
    (BUDGET_RS, "Publications"),
    (BUDGET_RS, "Publication"),
    (TASK_RS, "TaskTable"),
    (TASK_RS, "TaskEntry"),
    (TASK_RS, "Continuation"),
    (TASK_RS, "DurableRevision"),
    (CONTINUATION_RS, "ContinuationRecord"),
    (CONTINUATION_RS, "Restored"),
    (TERMINAL_RS, "TerminalRecord"),
    (RECOVERY_RS, "TaskResolution"),
    (RECOVERY_RS, "ResolvedTask"),
];

/// Members the record walk does not descend through, each with the reason its nested
/// types cannot hold a claim that an artifact is published. The member's own bare handles
/// are still judged. A row whose member is gone is stale.
const OPAQUE_FIELDS: &[(&str, &str, &str, &str)] = &[
    (
        STATE_RS,
        "DaemonState",
        "idempotency",
        "the replay ledger: answers already sent, replayed byte for byte; each answer's \
         artifacts were bound from receipt-tied records when it was built",
    ),
    (
        STATE_RS,
        "DaemonState",
        "capabilities",
        "the capability registry: wire descriptors of authority, not content",
    ),
    (
        STATE_RS,
        "DaemonState",
        "components",
        "registered component sets, wire values held as registered and never published by \
         registration",
    ),
];

/// The fields that say "this artifact is published", each holding `Published<H>`.
const RECEIPT_TIED_FIELDS: &[(&str, &str, &str)] = &[
    // `sealed: bool` until bn-283p6.
    (STATE_RS, "WorkspaceRecord", "seal"),
    // The content `observe.ingest` and `evidence.link` publish before they append.
    (STATE_RS, "EvidenceNode", "publication"),
    // A campaign record the task committed.
    (BUDGET_RS, "Publication", "commitment"),
    // The last durable continuation record, live and restored.
    (TASK_RS, "DurableRevision", "record"),
    (CONTINUATION_RS, "Restored", "record"),
    // What a startup resolution claims, restores from, and emits (cr-2n0xng).
    (RECOVERY_RS, "ResolvedTask", "records"),
    (RECOVERY_RS, "ResolvedTask", "continuations"),
    (RECOVERY_RS, "ResolvedTask", "continuation_identity"),
    (RECOVERY_RS, "ResolvedTask", "terminals"),
    (RECOVERY_RS, "ResolvedTask", "terminal_identity"),
];

/// The closed set of bare handle fields in [`DAEMON_RECORDS`], each with the reason it
/// does not claim a publication. A new bare handle field is a violation until it is
/// either tied to a receipt or adjudicated here.
const BARE_HANDLE_FIELDS: &[(&str, &str, &str, &str)] = &[
    // Lookup keys. The claim, where there is one, is in the value record.
    (
        STATE_RS,
        "DaemonState",
        "capabilities",
        "key: a capability, which is not content",
    ),
    (
        STATE_RS,
        "DaemonState",
        "workspaces",
        "key: the seal is `WorkspaceRecord::seal`",
    ),
    (
        STATE_RS,
        "DaemonState",
        "content",
        "key: staged content, never published by staging",
    ),
    (
        STATE_RS,
        "DaemonState",
        "components",
        "key: a registered component set, not published",
    ),
    (
        STATE_RS,
        "DaemonState",
        "intents",
        "key: the intent registry is volatile, not the store",
    ),
    (
        STATE_RS,
        "DaemonState",
        "evidence",
        "key: a graph node identity; see `EvidenceNode::publication`",
    ),
    (
        STATE_RS,
        "DaemonState",
        "edges",
        "key: a graph edge identity, never published",
    ),
    (
        STATE_RS,
        "DaemonState",
        "context_packs",
        "key: a context pack, held and not published",
    ),
    (
        STATE_RS,
        "DaemonState",
        "receipt_signatures",
        "key: a receipt node identity; the publication is `EvidenceNode::publication`",
    ),
    (
        TASK_RS,
        "TaskTable",
        "tasks",
        "key: a task identity, not a store artifact",
    ),
    // A refused run's key (cr-3hcpn4): what was refused, for whom. A refused run
    // published nothing, so none of these claims a publication.
    (
        TASK_RS,
        "RefusedRun",
        "capability",
        "key: the capability a refused run is charged to, not content",
    ),
    (
        TASK_RS,
        "RefusedRun",
        "task",
        "key: the task a refused run was for; a refused run publishes nothing",
    ),
    (
        TASK_RS,
        "RefusedRun",
        "from",
        "key: the continuation a refused resume started from, not a claim",
    ),
    (
        TASK_RS,
        "TaskTable",
        "continuations",
        "key: a continuation's pins identity",
    ),
    (
        TASK_RS,
        "TaskTable",
        "resolved",
        "key: a task identity, not a store artifact",
    ),
    (
        TASK_RS,
        "TaskTable",
        "pending",
        "key: a continuation's pins identity",
    ),
    (
        TASK_RS,
        "TaskTable",
        "revisions",
        "key: the durable record is `DurableRevision::record`",
    ),
    // References to what a record is about, which the record does not claim is published.
    (
        STATE_RS,
        "WorkspaceRecord",
        "intent",
        "the governing contract, by identity (plan §4.2)",
    ),
    (
        STATE_RS,
        "EvidenceNode",
        "artifact",
        "staged content by commitment; the claim is `publication`",
    ),
    (
        STATE_RS,
        "EvidenceEdge",
        "from",
        "a graph node identity, never published",
    ),
    (
        STATE_RS,
        "EvidenceEdge",
        "to",
        "a graph node identity, never published",
    ),
    (
        BUDGET_RS,
        "Publications",
        "staged",
        "staged, not yet published: the typestate's open half",
    ),
    (TASK_RS, "TaskEntry", "handle", "the task's own identity"),
    (
        TASK_RS,
        "TaskEntry",
        "snapshot",
        "the workspace the run is over; its seal is on its record",
    ),
    (
        TASK_RS,
        "TaskEntry",
        "intent",
        "the governing contract, by identity",
    ),
    (
        TASK_RS,
        "TaskEntry",
        "model",
        "the model source identity, catalogued and not published",
    ),
    (
        TASK_RS,
        "TaskEntry",
        "committed_evidence",
        "graph node identities, never published",
    ),
    (
        TASK_RS,
        "TaskEntry",
        "continuation",
        "the pins identity; the durable record is `DurableRevision::record`",
    ),
    (
        TASK_RS,
        "Continuation",
        "handle",
        "the pins identity, not a store handle",
    ),
    (TASK_RS, "Continuation", "task", "the task it resumes"),
    (
        TASK_RS,
        "Continuation",
        "snapshot",
        "the pinned workspace, by identity",
    ),
    (
        TASK_RS,
        "Continuation",
        "intent",
        "the pinned contract, by identity",
    ),
    // Durable encodings: the bytes the store holds, projected from the live task and
    // decoded on restart. A restore ties them to the ledger (`restore(&audit, …)`).
    (
        CONTINUATION_RS,
        "ContinuationRecord",
        "handle",
        "durable encoding",
    ),
    (
        CONTINUATION_RS,
        "ContinuationRecord",
        "task",
        "durable encoding",
    ),
    (
        CONTINUATION_RS,
        "ContinuationRecord",
        "snapshot",
        "durable encoding",
    ),
    (
        CONTINUATION_RS,
        "ContinuationRecord",
        "intent",
        "durable encoding",
    ),
    (
        CONTINUATION_RS,
        "ContinuationRecord",
        "model",
        "durable encoding",
    ),
    (
        CONTINUATION_RS,
        "ContinuationRecord",
        "publications",
        "durable encoding; restore ties each to its receipt",
    ),
    (
        CONTINUATION_RS,
        "ContinuationRecord",
        "committed_evidence",
        "durable encoding",
    ),
    (TERMINAL_RS, "TerminalRecord", "task", "durable encoding"),
    (
        TERMINAL_RS,
        "TerminalRecord",
        "snapshot",
        "durable encoding",
    ),
    (TERMINAL_RS, "TerminalRecord", "intent", "durable encoding"),
    (TERMINAL_RS, "TerminalRecord", "model", "durable encoding"),
    (
        TERMINAL_RS,
        "TerminalRecord",
        "publications",
        "durable encoding; restore ties each to its receipt",
    ),
    (
        TERMINAL_RS,
        "TerminalRecord",
        "committed_evidence",
        "durable encoding",
    ),
    (
        TERMINAL_RS,
        "TerminalRecord",
        "continuation",
        "durable encoding",
    ),
    // Reached only by the walk (cr-2n0xng): nested in the records above.
    (
        RECOVERY_RS,
        "ResolvedTask",
        "task",
        "the task's own identity, not a store artifact",
    ),
    (
        RECOVERY_RS,
        "TaskResolution",
        "unattributed",
        "report: an index entry that does not decode, claimed by no task",
    ),
    (
        RECOVERY_RS,
        "TaskResolution",
        "unclaimed",
        "report: a record no task claims, reported and not used",
    ),
    (
        RECOVERY_RS,
        "TaskResolution",
        "unreceipted",
        "report: an identity with no receipt, which no `ResolvedTask` can hold",
    ),
    (
        STATE_RS,
        "AdmissionRecord",
        "capability",
        "the admitting capability, which is not content",
    ),
    (
        STATE_RS,
        "IntentRecord",
        "supersedes",
        "the intent registry is volatile, not the store",
    ),
    (
        STATE_RS,
        "IntentRecord",
        "superseded_by",
        "the intent registry is volatile, not the store",
    ),
    (
        CONTEXT_RS,
        "ContextPackRecord",
        "snapshot",
        "a registered pack's workspace, by identity; registration publishes nothing",
    ),
    (
        CONTEXT_RS,
        "ContextCompileSource",
        "snapshot",
        "a registered answer header, provisioned out of band (IDL section 7)",
    ),
    (
        CONTEXT_RS,
        "ContextCompileSource",
        "intent",
        "a registered answer header, provisioned out of band (IDL section 7)",
    ),
    (
        CONTEXT_RS,
        "ContextCompileSource",
        "evidence",
        "a registered answer header, provisioned out of band (IDL section 7)",
    ),
    (
        VERIFICATION_RS,
        "ModelCatalog",
        "models",
        "key: a model source commitment, catalogued and not published",
    ),
    (
        ENVELOPE_RS,
        "Redacted",
        "commitment",
        "the commitment of redacted content, stated and not published",
    ),
    (
        WIRE_TASK_RS,
        "EvidenceEvent",
        "node",
        "a graph node identity, never published",
    ),
    (
        WIRE_TASK_RS,
        "EvidenceEvent",
        "edge",
        "a graph edge identity, never published",
    ),
    (WIRE_TASK_RS, "TaskEvent", "task", "the task's own identity"),
];

/// One named member of a type: a struct field, a tuple field (`0`, `1`, …), an enum
/// variant's field (`Variant.0`, `Variant.field`), or a type alias's right side (`type`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Field {
    name: String,
    ty: Vec<String>,
    line: usize,
}

/// The types the record walk reached, by `(file, name)`, with their members.
type Reached = BTreeMap<(String, String), Vec<Field>>;

/// One type the daemon crate defines, with its members.
#[derive(Debug, Clone)]
struct TypeDef {
    file: String,
    members: Vec<Field>,
}

/// A handle type, by spelling: every `*Handle`, and `Commitment`.
fn is_handle_type(ident: &str) -> bool {
    ident == "Commitment" || (ident.len() > "Handle".len() && ident.ends_with("Handle"))
}

/// The handle types in `ty` that no enclosing `Published<…>` wraps.
fn bare_handles(ty: &[String]) -> Vec<String> {
    let mut bare = Vec::new();
    for (index, ident) in unwrapped_idents(ty) {
        if is_handle_type(&ty[index]) {
            bare.push(ident);
        }
    }
    bare
}

/// Every identifier in `ty` that no enclosing `Published<…>` wraps, with its index.
fn unwrapped_idents(ty: &[String]) -> Vec<(usize, String)> {
    let mut owners: Vec<String> = Vec::new();
    let mut last_ident: Option<String> = None;
    let mut found = Vec::new();
    for (index, token) in ty.iter().enumerate() {
        match token.as_str() {
            "<" => owners.push(last_ident.take().unwrap_or_default()),
            ">" => {
                owners.pop();
            }
            ident if ident.chars().next().is_some_and(char::is_alphabetic) => {
                if !owners.iter().any(|owner| owner == "Published") {
                    found.push((index, ident.to_owned()));
                }
                last_ident = Some(ident.to_owned());
            }
            _ => last_ident = None,
        }
    }
    found
}

/// The types `ty` names that the walk descends into: every type name no `Published<…>`
/// wraps, except the handle types themselves, each with the module that qualifies it
/// (`super::task::TaskTable` → `task`), if any.
fn nested_types(ty: &[String]) -> Vec<(String, Option<String>)> {
    unwrapped_idents(ty)
        .into_iter()
        .filter(|(_, ident)| {
            ident.chars().next().is_some_and(char::is_uppercase) && !is_handle_type(ident)
        })
        .map(|(index, ident)| {
            let qualifier = (index >= 3 && ty[index - 1] == ":" && ty[index - 2] == ":")
                .then(|| ty[index - 3].clone())
                .filter(|module| !matches!(module.as_str(), "super" | "crate" | "self"));
            (ident, qualifier)
        })
        .collect()
}

/// `tokens` without attributes (`#[…]`).
fn without_attributes<'a>(tokens: &[&'a Token]) -> Vec<&'a Token> {
    let mut kept = Vec::new();
    let mut at = 0;
    while at < tokens.len() {
        if tokens[at].text == "#" && tokens.get(at + 1).is_some_and(|t| t.text == "[") {
            let mut brackets = 0usize;
            while at < tokens.len() {
                match tokens[at].text.as_str() {
                    "[" => brackets += 1,
                    "]" => {
                        brackets -= 1;
                        if brackets == 0 {
                            at += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                at += 1;
            }
            continue;
        }
        kept.push(tokens[at]);
        at += 1;
    }
    kept
}

/// `tokens` split at top-level `,` and `;` (a `protocol_struct!` body ends fields with
/// `;`), attributes dropped, empty segments skipped.
fn split_members<'a>(tokens: &[&'a Token]) -> Vec<Vec<&'a Token>> {
    let tokens = without_attributes(tokens);
    let mut segments = Vec::new();
    let mut segment: Vec<&Token> = Vec::new();
    let mut depth = 0usize;
    for token in tokens {
        match token.text.as_str() {
            "<" | "(" | "[" | "{" => depth += 1,
            ">" | ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "," | ";" if depth == 0 => {
                if !segment.is_empty() {
                    segments.push(std::mem::take(&mut segment));
                }
                continue;
            }
            _ => {}
        }
        segment.push(token);
    }
    if !segment.is_empty() {
        segments.push(segment);
    }
    segments
}

/// A member segment without its visibility (`pub`, `pub(crate)`, …).
fn without_visibility<'a, 'b>(segment: &'b [&'a Token]) -> &'b [&'a Token] {
    let mut rest = segment;
    if rest.first().is_some_and(|t| t.text == "pub") {
        rest = &rest[1..];
        if rest.first().is_some_and(|t| t.text == "(") {
            let close = rest.iter().position(|t| t.text == ")").unwrap_or(0);
            rest = &rest[close + 1..];
        }
    }
    rest
}

/// Named fields (`name: Type`), each prefixed with `prefix`.
fn named_members(tokens: &[&Token], prefix: &str) -> Vec<Field> {
    split_members(tokens)
        .iter()
        .filter_map(|segment| {
            let rest = without_visibility(segment);
            (rest.len() > 2 && rest[1].text == ":" && rest[2].text != ":").then(|| Field {
                name: format!("{prefix}{}", rest[0].text),
                ty: rest[2..].iter().map(|t| t.text.clone()).collect(),
                line: rest[0].line,
            })
        })
        .collect()
}

/// Positional fields, named `{prefix}0`, `{prefix}1`, ….
fn tuple_members(tokens: &[&Token], prefix: &str) -> Vec<Field> {
    split_members(tokens)
        .iter()
        .enumerate()
        .filter_map(|(index, segment)| {
            let rest = without_visibility(segment);
            (!rest.is_empty()).then(|| Field {
                name: format!("{prefix}{index}"),
                ty: rest.iter().map(|t| t.text.clone()).collect(),
                line: rest[0].line,
            })
        })
        .collect()
}

/// The index of the delimiter that closes the one at `open`.
fn closing(tokens: &[&Token], open: usize) -> usize {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        match token.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                depth -= 1;
                if depth == 0 {
                    return index;
                }
            }
            _ => {}
        }
    }
    tokens.len()
}

/// Every non-test `struct`, `enum`, and module-level `type` alias in `parsed`, by name, with
/// its members. Tuple structs, enum variants and `protocol_struct!` bodies included.
fn type_definitions(parsed: &Parsed) -> Vec<(String, Vec<Field>)> {
    let placed: Vec<&Placed> = parsed.tokens.iter().filter(|p| !p.test).collect();
    let tokens: Vec<&Token> = placed.iter().map(|p| &p.token).collect();
    let mut definitions = Vec::new();
    for at in 0..tokens.len() {
        let keyword = tokens[at].text.as_str();
        if !matches!(keyword, "struct" | "enum" | "type") {
            continue;
        }
        let Some(name) = tokens.get(at + 1).filter(|t| t.kind == TokenKind::Ident) else {
            continue;
        };
        if keyword == "type" && placed[at].impl_type.is_some() {
            continue;
        }
        let mut cursor = at + 2;
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
        if tokens.get(cursor).is_some_and(|t| t.text == "where") {
            while tokens
                .get(cursor)
                .is_some_and(|t| !matches!(t.text.as_str(), "{" | ";"))
            {
                cursor += 1;
            }
        }
        let Some(open) = tokens.get(cursor) else {
            continue;
        };
        let members = match (keyword, open.text.as_str()) {
            ("type", "=") => {
                let end = (cursor..tokens.len())
                    .find(|&i| tokens[i].text == ";")
                    .unwrap_or(tokens.len());
                vec![Field {
                    name: "type".to_owned(),
                    ty: tokens[cursor + 1..end]
                        .iter()
                        .map(|t| t.text.clone())
                        .collect(),
                    line: name.line,
                }]
            }
            ("struct", "{") => named_members(&tokens[cursor + 1..closing(&tokens, cursor)], ""),
            ("struct", "(") => tuple_members(&tokens[cursor + 1..closing(&tokens, cursor)], ""),
            ("struct", ";") => Vec::new(),
            ("enum", "{") => split_members(&tokens[cursor + 1..closing(&tokens, cursor)])
                .iter()
                .flat_map(|variant| {
                    let Some(head) = variant.first() else {
                        return Vec::new();
                    };
                    let prefix = format!("{}.", head.text);
                    match variant.get(1).map(|t| t.text.as_str()) {
                        Some("(") => tuple_members(
                            &variant[2..closing(variant, 1).min(variant.len())],
                            &prefix,
                        ),
                        Some("{") => named_members(
                            &variant[2..closing(variant, 1).min(variant.len())],
                            &prefix,
                        ),
                        _ => Vec::new(),
                    }
                })
                .collect(),
            _ => continue,
        };
        definitions.push((name.text.clone(), members));
    }
    definitions
}

/// Every type the daemon crate (`continuumd/src`) defines, by name. A name can have more
/// than one definition, in different modules.
fn daemon_types(tree: &BTreeMap<String, String>) -> BTreeMap<String, Vec<TypeDef>> {
    let mut types: BTreeMap<String, Vec<TypeDef>> = BTreeMap::new();
    for (file, text) in tree {
        if !file.starts_with(DAEMON_CRATE) {
            continue;
        }
        for (name, members) in type_definitions(&parse(text)) {
            types.entry(name).or_default().push(TypeDef {
                file: file.clone(),
                members,
            });
        }
    }
    types
}

/// The definitions `ident` names from `file`: the module that qualifies it if there is
/// one, else one in the same file, else every definition by that name (conservative).
fn resolve<'t>(
    types: &'t BTreeMap<String, Vec<TypeDef>>,
    ident: &str,
    qualifier: Option<&str>,
    file: &str,
) -> Vec<&'t TypeDef> {
    let Some(candidates) = types.get(ident) else {
        return Vec::new();
    };
    if let Some(module) = qualifier {
        let in_module: Vec<&TypeDef> = candidates
            .iter()
            .filter(|def| {
                def.file.ends_with(&format!("/{module}.rs"))
                    || def.file.ends_with(&format!("/{module}/mod.rs"))
            })
            .collect();
        if !in_module.is_empty() {
            return in_module;
        }
    }
    let local: Vec<&TypeDef> = candidates.iter().filter(|def| def.file == file).collect();
    if !local.is_empty() {
        return local;
    }
    candidates.iter().collect()
}

/// Every type reachable from [`DAEMON_RECORDS`] through the members of the types it reaches,
/// keyed by `(file, name)`, with its members (bn-283p6, cr-2n0xng).
///
/// A member's type is descended into when it names a type the daemon crate defines, at any
/// depth of generic nesting (`BTreeMap<K, Vec<Option<T>>>` reaches `T`), except inside
/// `Published<…>`: a receipt-tied value is tied as a whole, and its `PublishedName` impl
/// is what compares its identity with the receipt's. [`OPAQUE_FIELDS`] names the members
/// the walk does not descend through, each with its reason.
fn walk(
    tree: &BTreeMap<String, String>,
    types: &BTreeMap<String, Vec<TypeDef>>,
) -> (Reached, Vec<Violation>) {
    walk_from(tree, types, DAEMON_RECORDS)
}

/// [`walk`] from `roots`.
fn walk_from(
    tree: &BTreeMap<String, String>,
    types: &BTreeMap<String, Vec<TypeDef>>,
    roots: &[(&str, &str)],
) -> (Reached, Vec<Violation>) {
    let mut violations = Vec::new();
    let mut queue: Vec<(String, String)> = Vec::new();
    for &(file, record) in roots {
        if !tree.contains_key(file) {
            continue;
        }
        if types
            .get(record)
            .is_some_and(|defs| defs.iter().any(|def| def.file == file))
        {
            queue.push((file.to_owned(), record.to_owned()));
        } else {
            violations.push(Violation::DaemonRecordMissing {
                file: file.to_owned(),
                record: record.to_owned(),
            });
        }
    }
    let mut reached: Reached = BTreeMap::new();
    while let Some((file, record)) = queue.pop() {
        if reached.contains_key(&(file.clone(), record.clone())) {
            continue;
        }
        let Some(def) = types
            .get(&record)
            .and_then(|defs| defs.iter().find(|def| def.file == file))
        else {
            continue;
        };
        for field in &def.members {
            let opaque = OPAQUE_FIELDS
                .iter()
                .any(|(f, r, n, _)| *f == file && *r == record && *n == field.name);
            if opaque {
                continue;
            }
            for (ident, qualifier) in nested_types(&field.ty) {
                for next in resolve(types, &ident, qualifier.as_deref(), &file) {
                    queue.push((next.file.clone(), ident.clone()));
                }
            }
        }
        reached.insert((file, record), def.members.clone());
    }
    (reached, violations)
}

/// The record rules, over every type [`walk`] reaches from the daemon's records.
fn check_records(tree: &BTreeMap<String, String>) -> Vec<Violation> {
    let types = daemon_types(tree);
    let (reached, mut violations) = walk(tree, &types);
    let mut bare_found: BTreeSet<(String, String, String)> = BTreeSet::new();
    for ((file, record), fields) in &reached {
        for field in fields {
            if bare_handles(&field.ty).is_empty() {
                continue;
            }
            bare_found.insert((file.clone(), record.clone(), field.name.clone()));
            let adjudicated = BARE_HANDLE_FIELDS
                .iter()
                .any(|(f, r, n, _)| f == file && r == record && *n == field.name);
            if !adjudicated {
                violations.push(Violation::UnadjudicatedBareHandleField {
                    file: file.clone(),
                    record: record.clone(),
                    field: field.name.clone(),
                    line: field.line,
                });
            }
        }
    }
    // Only a tree that holds the daemon's records judges the rows: a partial tree under
    // test is judged on what it contains.
    let judged = |file: &str| tree.contains_key(file);
    for &(file, record, field, _) in BARE_HANDLE_FIELDS {
        if judged(file)
            && !bare_found.contains(&(file.to_owned(), record.to_owned(), field.to_owned()))
        {
            violations.push(Violation::StaleHandleFieldAdjudication {
                file: file.to_owned(),
                record: record.to_owned(),
                field: field.to_owned(),
            });
        }
    }
    for &(file, record, field, _) in OPAQUE_FIELDS {
        let present = types.get(record).is_some_and(|defs| {
            defs.iter()
                .any(|def| def.file == file && def.members.iter().any(|m| m.name == field))
        });
        if judged(file) && !present {
            violations.push(Violation::StaleHandleFieldAdjudication {
                file: file.to_owned(),
                record: record.to_owned(),
                field: field.to_owned(),
            });
        }
    }
    for &(file, record, field) in RECEIPT_TIED_FIELDS {
        if !judged(file) {
            continue;
        }
        let tied = reached
            .get(&(file.to_owned(), record.to_owned()))
            .is_some_and(|fields| {
                fields.iter().any(|f| {
                    f.name == field
                        && f.ty.iter().any(|t| t == "Published")
                        && bare_handles(&f.ty).is_empty()
                })
            });
        if !tied {
            violations.push(Violation::ReceiptTiedFieldLost {
                file: file.to_owned(),
                record: record.to_owned(),
                field: field.to_owned(),
            });
        }
    }
    violations
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
    violations.extend(check_records(tree));
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
        8,
        "three impls on each state, and inherent ones on the receipt and on `Published`: \
         {impls:#?}"
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
        "    Published::attest(&receipt, named).map_err(|_| aborted().not_retryable())\n",
        "    let _forged = PublicationReceipt { handle: named.clone() };\n    \
         Published::attest(&receipt, named).map_err(|_| aborted().not_retryable())\n",
    );
    let violations = violations_of(&tree);
    assert_eq!(violations.len(), 1, "{violations:#?}");
    assert!(matches!(
        &violations[0],
        Violation::WitnessMintedOutsideCommitPath { file: f, witness, .. }
            if f == file && witness == "PublicationReceipt"
    ));
}

/// **bn-283p6's mutant.** A daemon record gains a field that names a published artifact by
/// a bare handle. Nothing ties the name to a receipt, so the record could be written before
/// the publication commits. The census names the field and nothing else.
#[test]
fn mutant_a_bare_published_handle_on_a_daemon_record_is_caught() {
    let tree = mutate(
        STATE_RS,
        "    pub seal: Option<Published<WorkspaceHandle>>,\n",
        "    pub seal: Option<Published<WorkspaceHandle>>,\n    \
         pub descriptor_published_as: Option<continuum_workspace::artifact_path::ArtifactHandle>,\n",
    );
    let violations = violations_of(&tree);
    assert_eq!(violations.len(), 1, "{violations:#?}");
    assert!(
        matches!(
            &violations[0],
            Violation::UnadjudicatedBareHandleField { file, record, field, .. }
                if file == STATE_RS
                    && record == "WorkspaceRecord"
                    && field == "descriptor_published_as"
        ),
        "{violations:#?}"
    );
}

/// A tied field reverted to its bare handle: the task names a campaign record by a
/// commitment nothing ties to a receipt. Caught twice, once as a new bare field and once as
/// a lost tie.
#[test]
fn mutant_a_receipt_tied_field_reverted_to_a_bare_handle_is_caught() {
    let tree = mutate(
        BUDGET_RS,
        "    commitment: Published<Commitment>,\n",
        "    commitment: Commitment,\n",
    );
    let mut violations = violations_of(&tree);
    violations.sort();
    let mut expected = vec![
        Violation::UnadjudicatedBareHandleField {
            file: BUDGET_RS.to_owned(),
            record: "Publication".to_owned(),
            field: "commitment".to_owned(),
            line: violations
                .iter()
                .find_map(|v| match v {
                    Violation::UnadjudicatedBareHandleField { line, .. } => Some(*line),
                    _ => None,
                })
                .unwrap_or(0),
        },
        Violation::ReceiptTiedFieldLost {
            file: BUDGET_RS.to_owned(),
            record: "Publication".to_owned(),
            field: "commitment".to_owned(),
        },
    ];
    expected.sort();
    assert_eq!(violations, expected);
}

/// `Published` forged from a bare name through a trait `impl`: a constructor by another
/// name. Caught as an unexpected `impl` and as a mint outside the two sanctioned items.
#[test]
fn mutant_a_published_name_forged_without_a_receipt_is_caught() {
    let tree = mutate(
        PUBLICATION_RS,
        "impl Published<ArtifactHandle> {\n",
        "impl<H> From<H> for Published<H> {\n    fn from(name: H) -> Self {\n        \
         Self { name }\n    }\n}\n\nimpl Published<ArtifactHandle> {\n",
    );
    let violations = violations_of(&tree);
    assert!(
        violations.contains(&Violation::UnexpectedWitnessImpl {
            witness: "Published".to_owned(),
            trait_name: Some("From".to_owned()),
        }),
        "{violations:#?}"
    );
    assert!(
        violations.iter().any(|v| matches!(
            v,
            Violation::WitnessMintedOutsideCommitPath { witness, item, .. }
                if witness == "Published" && item == "Published::from"
        )),
        "{violations:#?}"
    );
    assert_eq!(violations.len(), 2, "{violations:#?}");
}

/// **cr-2n0xng's mutant: a nested record.** A record the task table stores holds a new
/// record type, and that type names a published artifact by a bare handle. The field on
/// `ResolvedTask` holds no handle type itself, so a direct-field census passes it. The walk
/// descends into the nested type and names its field.
#[test]
fn mutant_a_bare_handle_nested_in_a_record_the_task_table_stores_is_caught() {
    let tree = mutate(
        RECOVERY_RS,
        "pub struct ResolvedTask {\n",
        "pub struct Echo(pub ArtifactHandle);\n\npub struct ResolvedTask {\n    pub echoes: Vec<Echo>,\n",
    );
    assert!(
        bare_handles(&["Vec", "<", "Echo", ">"].map(str::to_owned)).is_empty(),
        "the direct-field rule alone does not see it"
    );
    let violations = violations_of(&tree);
    assert_eq!(violations.len(), 1, "{violations:#?}");
    assert!(
        matches!(
            &violations[0],
            Violation::UnadjudicatedBareHandleField { file, record, field, .. }
                if file == RECOVERY_RS && record == "Echo" && field == "0"
        ),
        "{violations:#?}"
    );
}

/// **cr-2n0xng's mutant: a resolution that holds its campaign records untied.** The
/// finding's own shape: `ResolvedTask::records` back to bare `CampaignRecord`s, whose
/// identities `claims()` and `verification.start` would emit with no receipt. Caught as a
/// lost tie, and as the nested record's bare identity.
#[test]
fn mutant_a_resolution_holding_campaign_records_untied_is_caught() {
    let tree = mutate(
        RECOVERY_RS,
        "    pub records: Vec<Published<CampaignRecord>>,\n",
        "    pub records: Vec<CampaignRecord>,\n",
    );
    let violations = violations_of(&tree);
    assert!(
        violations.contains(&Violation::ReceiptTiedFieldLost {
            file: RECOVERY_RS.to_owned(),
            record: "ResolvedTask".to_owned(),
            field: "records".to_owned(),
        }),
        "{violations:#?}"
    );
    assert!(
        violations.iter().any(|v| matches!(
            v,
            Violation::UnadjudicatedBareHandleField { file, record, field, .. }
                if file == RECOVERY_RS && record == "CampaignRecord" && field == "identity"
        )),
        "{violations:#?}"
    );
    assert!(
        violations.iter().all(|v| match v {
            Violation::ReceiptTiedFieldLost { record, .. } => record == "ResolvedTask",
            Violation::UnadjudicatedBareHandleField { record, .. } => record == "CampaignRecord",
            _ => false,
        }),
        "only the reverted field and the record it exposes: {violations:#?}"
    );
}

/// **cr-2n0xng's mutant: an enum variant.** A resolution's typed failure reason gains a
/// payload that names an artifact by a bare handle. The walk reads enum variants too,
/// reached through `ResolvedTask::resolution`.
#[test]
fn mutant_a_bare_handle_in_a_failure_reason_variant_is_caught() {
    let tree = mutate(
        RECOVERY_RS,
        "    /// restored, and the task is not reported as `Restored` or `Terminal` either.\n    Unreceipted,\n",
        "    /// restored, and the task is not reported as `Restored` or `Terminal` either.\n    Unreceipted(ArtifactHandle),\n",
    );
    let violations = violations_of(&tree);
    assert_eq!(violations.len(), 1, "{violations:#?}");
    assert!(
        matches!(
            &violations[0],
            Violation::UnadjudicatedBareHandleField { file, record, field, .. }
                if file == RECOVERY_RS && record == "FailureReason" && field == "Unreceipted.0"
        ),
        "{violations:#?}"
    );
}

/// An adjudication that stops the walk cannot outlive its field.
#[test]
fn mutant_a_stale_opaque_adjudication_is_caught() {
    let tree = mutate(
        STATE_RS,
        "    components: BTreeMap<Commitment, SnapshotComponents>,\n",
        "    registered_components: BTreeMap<Commitment, SnapshotComponents>,\n",
    );
    let violations = violations_of(&tree);
    assert!(
        violations.contains(&Violation::StaleHandleFieldAdjudication {
            file: STATE_RS.to_owned(),
            record: "DaemonState".to_owned(),
            field: "components".to_owned(),
        }),
        "{violations:#?}"
    );
}

/// **Positive, and not vacuous.** The record census walks every listed record and every
/// daemon type they hold at any depth, finds every adjudicated bare field and every tied
/// field, and each tied field holds `Published`.
#[test]
fn the_daemon_records_that_name_a_published_artifact_hold_a_receipt_tied_name() {
    let tree = source_tree();
    assert!(
        check_records(&tree).is_empty(),
        "{:#?}",
        check_records(&tree)
    );
    let types = daemon_types(&tree);
    let (reached, missing) = walk(&tree, &types);
    assert!(missing.is_empty(), "{missing:#?}");
    for &(file, record) in DAEMON_RECORDS {
        let fields = reached
            .get(&(file.to_owned(), record.to_owned()))
            .unwrap_or_else(|| panic!("{file} defines {record}"));
        assert!(!fields.is_empty(), "{record} has fields the census read");
    }
    assert!(
        reached.len() > DAEMON_RECORDS.len(),
        "the walk descends past the listed records ({} types)",
        reached.len()
    );
    let tied = RECEIPT_TIED_FIELDS
        .iter()
        .filter(|(file, record, field)| {
            reached
                .get(&((*file).to_owned(), (*record).to_owned()))
                .is_some_and(|fields| fields.iter().any(|f| f.name == *field))
        })
        .count();
    assert_eq!(tied, RECEIPT_TIED_FIELDS.len(), "every tied field is found");

    // cr-2n0xng: what the task table stores is walked, not only its direct fields. From the
    // table alone the walk reaches the resolution records a restart files, the records a
    // resolution restores from, and the enums its outcome is spelled in.
    let (from_table, _) = walk_from(&tree, &types, &[(TASK_RS, "TaskTable")]);
    for (file, record) in [
        (RECOVERY_RS, "ResolvedTask"),
        (RECOVERY_RS, "Resolution"),
        (RECOVERY_RS, "FailureReason"),
        (CONTINUATION_RS, "ContinuationRecord"),
        (TERMINAL_RS, "TerminalRecord"),
        (TASK_RS, "TaskEntry"),
        (TASK_RS, "Continuation"),
        (TASK_RS, "DurableRevision"),
    ] {
        assert!(
            from_table.contains_key(&(file.to_owned(), record.to_owned())),
            "the task table reaches {record}: {:#?}",
            from_table.keys().collect::<Vec<_>>()
        );
    }
    assert!(
        !reached.contains_key(&(RECOVERY_RS.to_owned(), "CampaignRecord".to_owned())),
        "a campaign record is held only as `Published<CampaignRecord>`, so the walk does \
         not descend into it"
    );
    assert!(
        !reached.contains_key(&(RECOVERY_RS.to_owned(), "CandidateTask".to_owned())),
        "the untied pass output is stored by no daemon record"
    );
    assert_eq!(
        bare_handles(&["Option", "<", "Published", "<", "Commitment", ">", ">"].map(str::to_owned)),
        Vec::<String>::new(),
        "a handle inside `Published<…>` is tied"
    );
    assert_eq!(
        bare_handles(
            &[
                "BTreeMap",
                "<",
                "TaskHandle",
                ",",
                "Published",
                "<",
                "Commitment",
                ">",
                ">"
            ]
            .map(str::to_owned)
        ),
        vec!["TaskHandle".to_owned()],
        "a sibling of `Published<…>` is not"
    );
    assert_eq!(
        nested_types(
            &[
                "BTreeMap",
                "<",
                "TaskHandle",
                ",",
                "super",
                ":",
                ":",
                "recovery",
                ":",
                ":",
                "ResolvedTask",
                ">"
            ]
            .map(str::to_owned)
        ),
        vec![
            ("BTreeMap".to_owned(), None),
            ("ResolvedTask".to_owned(), Some("recovery".to_owned()))
        ],
        "a nested record is found at any depth, with its module"
    );
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
