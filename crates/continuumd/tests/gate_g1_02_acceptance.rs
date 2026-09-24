//! G1-02 acceptance — **independent re-derivation** of "explicit handles across native
//! API" (bn-3j01v).
//!
//! > explicit handles across native API
//! >
//! > — `notes/plan/docs/52_RELEASE_GATES_REV3.md:22`, gate G1, criterion **G1-02**
//!
//! The constitutional backdrop is INV-002 ("Every stateful workflow uses explicit handles.
//! A dropped connection or restarted client does not change meaning", `plan.md:311`),
//! INV-015, and the plan's own headline — "typed operations over explicit handles, not
//! terminal prose".
//!
//! # What this file is, and what it deliberately is not
//!
//! It is **not** a re-run of the delivering suites. `notes/plan/notes/PHASE_A_EXIT_PACKAGE.md`
//! §7.2 grades G1-02 "evidenced — the registry's typed handle vocabulary;
//! `idl_conformance.rs` + `registry_agreement.rs` hold the transcription to the IDL and to
//! RFC 0027's independent table". Both of those checkers answer a **transcription** question:
//! *do the committed Rust types say what the IDL says?* Neither asks the criterion's own
//! question, which is a question about the IDL's *content*: *does every operation name every
//! resource it touches through an explicit typed handle?* A perfectly faithful transcription
//! of a protocol full of raw paths would pass both of them.
//!
//! Three method differences carry the independence:
//!
//! 1. **A second reader, by a different strategy.** [`reader`] is written from the IDL
//!    header's own EBNF but parses by *region extraction* — comment/rule elision, brace-
//!    balanced declaration slicing, then statement splitting — where `idl_conformance.rs`
//!    builds a token stream and runs recursive descent over it. Neither reader shares a line
//!    with the other, and this one never reads `continuumd::protocol::registry`: the sweep's
//!    subject is the normative file, not the crate's copy of it.
//! 2. **A semantic classification, not an equality check.** Every leaf of every
//!    request and response body — and of both envelopes — is walked transitively through
//!    named structs and unions and given a [`Class`] from its declared type, and every leaf
//!    that is neither a handle nor a plain scalar is then adjudicated against a **closed,
//!    exhaustive table** ([`STRING_SITES`], [`INLINE_SITES`]). The tables are keyed by
//!    declaration site, and [`every_string_and_inline_site_is_adjudicated`] fails on a site
//!    the tables do not name, so a field added to the IDL cannot enter the protocol without
//!    an adjudication.
//! 3. **A third, independent completeness anchor.** The sweep's operation set is
//!    cross-checked against **plan §10.2**, which neither landed checker reads
//!    (`idl_conformance.rs` reads the IDL; `registry_agreement.rs` reads RFC 0027's
//!    authority table and RFC 0026's verdict table). [`the_sweep_is_exhaustive_over_the_registry`]
//!    parses §10.2's own text block and requires the two lists to agree name for name and in
//!    order.
//!
//! The live half shares no fixture with `inv002_no_hidden_state_evidence.rs` — that file
//! transplants `DaemonState` between two `Daemon` values to simulate a reconnection, and
//! asks whether a *parked campaign* survives it. This file never reconnects anything. It
//! asks a different question with a different instrument: whether two principals' requests
//! against two distinct resources, **interleaved in both orders on freshly built daemons**,
//! produce answers that are a function of the handle alone.
//!
//! # The sweep, in numbers
//!
//! | | |
//! |---|---|
//! | operations swept | 83, and asserted equal to plan §10.2's registry |
//! | bodies swept | 168 — a request and a response per operation, plus both envelopes; one is declared empty |
//! | leaf fields classified | 702 ([`LEAVES`]) |
//! | distinct declaration sites | 446 ([`SITES`]) |
//! | explicit typed handles (19 declared classes) | 210 ([`HANDLE_LEAVES`]) |
//! | signer names (`SignerHandle`, a content identity, protocol 3.8) | 11 ([`SIGNER_NAME_LEAVES`]) |
//! | class-agnostic `ArtifactHandle`s | 12 ([`ARTIFACT_HANDLE_LEAVES`]) |
//! | content commitments | 21 ([`COMMITMENT_LEAVES`]) |
//! | **resource-naming leaves, all of them handles or content identities** | **254** |
//! | `String` leaves — the class every finding lives in | 119 ([`STRING_LEAVES`]) |
//! | `Opaque`/`Bytes` leaves | 48 ([`INLINE_LEAVES`]) |
//! | adjudicated `String` sites | 66 |
//! | adjudicated `Opaque`/`Bytes` sites | 46 |
//! | `String` sites whose domain a named IDL rule declares | 5 sites, 6 leaves ([`DECLARED_DOMAINS`]) |
//! | **findings** | **0** — the 6 leaves over 5 sites pinned before bn-ah1k8 are paid by it |
//!
//! # Verdict
//!
//! **SATISFIED-AT-NARROWER-SCOPE.** Every daemon-held resource that any of the 83
//! operations consumes or returns is named by an explicit typed handle or by a content
//! commitment, or by a string whose grammar, meaning, and refusal a named IDL rule declares,
//! with a stated boundary on inbound content. The scope statements are not softened:
//!
//! | # | Scope statement | Test |
//! |---|---|---|
//! | 1 | 19 declared handle classes; every one is a prefix-checked newtype whose constructor is the parser, so a wrong-class handle is unconstructable rather than merely unlikely | [`a_handle_of_the_wrong_class_cannot_be_constructed_at_all`] |
//! | 2 | Every `@mutation` operation returns at least one handle or commitment. The five operations whose response names none are all `@readonly` | [`the_response_direction_returns_handles_wherever_it_mints_or_moves_a_resource`] |
//! | 3 | Every response-position inline blob travels in a call that also names a handle — content beside a name, never instead of one | [`inline_content_never_travels_without_a_handle_in_the_same_call`] |
//! | 4 | **FINDING, paid (bn-ah1k8)**: `benchmark.run` named its benchmark task by `task_id: String` and its graders by `graders: list<String>`, with no handle class and no declared domain; `Target.id` under `benchmark_task` had the same gap. `rule benchmark.task_identity` now fixes the task name's grammar, meaning, and refusal, and states why a content handle is refused (it would be a function of the bundle's hidden half, an existence oracle). `rule benchmark.graders` closes the grader vocabulary to RFC 0034's seven grader-order stages | [`no_leaf_names_a_resource_without_a_handle_or_a_declared_domain`], [`every_declared_domain_is_a_rule_that_cites_its_site`] |
//! | 5 | **FINDING, paid (bn-ah1k8)**: `query.explain_invalidation`'s `edges: list<String>` named nothing. `rule query.invalidation_edges` spells each edge `<handle>:<reason>` over a sibling `ArtifactHandle`, so the artifact is named by its handle, and states why an index record has no class of its own | same |
//! | 6 | **FINDING (weak), paid (bn-ah1k8)**: `EvidenceQuery.claim_id` had two readings and `QueryExplainReuseResponse.reasons` had no doc. `rule evidence.claim_identity` fixes the first reading (a claim identity, never a property identifier); `rule query.reuse_reasons` keys `reasons` by sibling handles and closes its values | same |
//! | 7 | 12 request-position leaves carry an inbound document by value (8 until protocol 3.8 added a bundle, a pack, and a signed artifact with its signature) (`Opaque`/`Bytes`); three of them name documents that have a `schemas/` artifact shape and no handle class (`whiteboard.compile.note`, `forge.create.sketch`, `intent.propose_revision.changes.changes`) | [`inbound_documents_travel_by_value_and_this_is_the_whole_list`] |
//! | 8 | The live probes reach **38 of 83** operations (30 of 75 until protocol 3.8): the other 45 have no `Arguments` variant in this build and are refused by the codec before a payload is read | [`the_live_surface_is_thirty_of_the_seventy_five_and_this_is_which`] |
//! | 9 | `ResultEnvelope.next_operations` — the typed discovery surface — is declared and **never populated** by this daemon | [`the_typed_discovery_surface_is_declared_and_unpopulated`] |
//!
//! Findings 4, 5 and 6 were **not** failures of the criterion as this file reads it: none of
//! them was ambient state, an implicit "current" context, or prose that must be parsed to
//! recover a name. They were places where a resource was named by an untyped string instead
//! of a typed handle. bn-ah1k8 pays them in the form `rule versioning.breaking_change` allows
//! inside major 3: the fields keep type `String`, and a rule declares each domain. Retyping
//! them is a major and stays owed (RFC 0026 F23). The pin below is now a regression guard: a
//! new raw-string resource, or a declared-domain rule that is removed or stops citing its
//! site, turns it red.
//!
//! # Negative controls
//!
//! Eight, each a real assertion rather than a comment. Seven weaken the sweep and require it
//! to report; one weakens the live comparison and requires it to disagree.
//!
//! | Control | What it doctors | What must happen |
//! |---|---|---|
//! | [`the_sweep_flags_a_raw_path_parameter`] | appends a synthetic `workspace.read_file` taking `snapshot_path: String` and answering `contents: Bytes` | both are reported unadjudicated |
//! | [`the_sweep_flags_a_handle_downgraded_to_a_string`] | `workspace.seal`'s `snapshot: WorkspaceHandle` → `String` | reported, and the handle census drops by one |
//! | [`the_sweep_flags_an_inline_blob_with_no_handle_in_its_call`] | removes `observe.classify`'s only handle | the call's inline response is left nameless |
//! | [`the_sweep_flags_a_mutation_that_returns_no_handle`] | `workspace.seal`'s response loses its handle | detected on a `@mutation` |
//! | [`a_removed_operation_breaks_the_registry_cross_check`] | deletes an operation from the IDL | both counting methods drop to 74 |
//! | [`the_adjudication_table_is_not_a_wildcard`] | — | the tables answer `None` for real fields they do not name |
//! | [`a_removed_domain_rule_is_reported`] | renames `rule benchmark.graders`, and separately removes `rule query.invalidation_edges`'s citation of its site | each declared-domain site loses its rule and is reported |
//! | [`the_order_independence_comparison_detects_an_ambient_resolution`] | seals "the most recently created" snapshot instead of the one named | the two permutations **disagree** |
//!
//! # What the live probes observed
//!
//! Across the real byte boundary, over 11 operations and 6 declared handle prefixes plus the
//! class-agnostic alias, each handle presented three ways:
//!
//! | Spelling | Outcome | Reading |
//! |---|---|---|
//! | valid handle of the declared class, nothing holds it | `CapabilityDenied` on 9 of 11, `UnsupportedSemanticFeature` on `observe.classify` and `context.compile` | decoded, then refused *semantically* — RFC 0027 X2 forbids a distinguishable not-found, and `rule errors.unsupported_surface` covers a lane that has not shipped |
//! | valid handle of another declared class | `MalformedRequest`, 11 of 11 | refused **at the codec**, by the class's own prefix rule, before the daemon sees a request |
//! | the field removed | `MalformedRequest`, 11 of 11 | there is no ambient value to fall back on |
//!
//! # Absences, stated (INV-007)
//!
//! - **45 of 75 operations cannot be probed live.** They have no handler in this build. The
//!   IDL sweep covers 75/75; nothing here claims a live behaviour for the other 45.
//! - **`CapabilityHandle` appears in no operation body** — it lives on the envelope only —
//!   so the dangling/foreign-handle probes reach 8 handle classes of the 19, plus
//!   `ArtifactHandle` and `Commitment`. The remaining classes are declared and unserved.
//! - **Doc-comment schema citation is not checked.** `rule encoding.opaque_payloads`
//!   requires every non-envelope `Opaque` to name a `schemas/` document in its doc comment;
//!   [`reader`] elides doc comments, so this file does not verify that clause.
//! - **One process, one connection.** `Daemon::dispatch` takes `&mut self`, so the
//!   "interleaving" below is sequential interleaving on one thread, not concurrency.

use std::collections::{BTreeMap, BTreeSet};

use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::codec::json::Json;
use continuumd::daemon::context::ContextFamily;
use continuumd::daemon::evidence::EvidenceFamily;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::{self, Blake3Identity};
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::daemon::state::{Acceptance, DaemonState, IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest};
use continuumd::protocol::envelope::{Budget, RequestEnvelope, ResultEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::context::{ContextCompileRequest, ContextExpandRequest};
use continuumd::protocol::operations::evidence::EvidenceGetRequest;
use continuumd::protocol::operations::intent::{IntentGetRequest, IntentLockRequest};
use continuumd::protocol::operations::observe::ObserveClassifyRequest;
use continuumd::protocol::operations::task::{
    TaskCancelRequest, TaskResumeRequest, TaskStatusRequest,
};
use continuumd::protocol::operations::workspace::{
    WorkspaceCreateRequest, WorkspaceForkRequest, WorkspaceSealRequest,
};
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, ContextHandle, ContinuationHandle, EpochIdentity, EvidenceHandle,
    IntentHandle, Opaque, OperationName, ProtocolVersion, RequestId, TaskHandle, WorkspaceHandle,
};
use continuumd::protocol::shared::{SnapshotComponents, SnapshotEpochs};
use continuumd::protocol::spec::{Annotation, Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, ExpansionRelation, ResultStatus,
};
use continuumd::transport::{self, Server};

/// The normative IDL — the only authority this sweep reads for protocol shape.
const IDL_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../notes/plan/schemas/continuumd-native-protocol.idl"
);

/// The plan, for its §10.2 operation registry — the independent completeness anchor.
const PLAN_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../notes/plan/plan.md");

// =========================================================================================
// 1. The reader
// =========================================================================================

mod reader {
    //! A second reader for the normative IDL, by **region extraction**.
    //!
    //! `tests/idl_conformance.rs` reads the same file by tokenizing it into a `Vec<Token>`
    //! and running a recursive-descent parser over the stream. This reader does not build a
    //! token stream at all: it elides comments and `"""` blocks in one pass, slices the
    //! result into top-level declarations by brace balance, and splits each declaration body
    //! into `;`-terminated statements which it reads with string operations. docs/03 §8 asks
    //! independent paths to differ in their parser, and two parsers that shared a strategy
    //! would share its blind spots.
    //!
    //! It is unforgiving on purpose: an unknown declaration keyword or a malformed field is a
    //! panic naming the offender, never a skipped declaration. A reader that silently dropped
    //! a construct would make the whole sweep pass vacuously for it, and
    //! [`Document::operation_keywords`] exists so that even a silent drop is caught — it is
    //! counted by a second, structurally different method (textual keyword occurrences) and
    //! compared against the number of operations actually parsed.

    use std::collections::BTreeMap;

    /// One `name: Type presence` declaration.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Field {
        /// The field name.
        pub name: String,
        /// The declared type, in the IDL's spelling, with interior spaces removed
        /// (`map<String, String>` reads `map<String,String>`).
        pub ty: String,
        /// `required`, `optional`, or `nullable`.
        pub presence: String,
    }

    /// A `request`/`response`/`events` body: a named struct, or an inline field list.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Body {
        /// `response VerificationResult;`
        Named(String),
        /// `request { … };`
        Inline(Vec<Field>),
    }

    /// One `operation namespace.verb { … }` declaration.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Operation {
        /// `namespace.verb`.
        pub name: String,
        /// Behavioural annotations, `@since` dropped (it dates a declaration, it does not
        /// describe one).
        pub annotations: Vec<String>,
        /// The `request` clause.
        pub request: Body,
        /// The `response` clause.
        pub response: Body,
    }

    /// The whole document, at the granularity this sweep needs.
    #[derive(Debug, Clone, Default)]
    pub struct Document {
        /// Declared handle classes, as `(name, prefix)`.
        pub handles: Vec<(String, String)>,
        /// Declared aliases, as `(name, base)`.
        pub aliases: Vec<(String, String)>,
        /// Declared scalar names.
        pub scalars: Vec<String>,
        /// Declared enum names.
        pub enums: Vec<String>,
        /// Declared unions, as `name -> [(variant, type)]`.
        pub unions: BTreeMap<String, Vec<(String, String)>>,
        /// Declared named structs, as `name -> fields`.
        pub structs: BTreeMap<String, Vec<Field>>,
        /// Declared operations, in file order.
        pub operations: Vec<Operation>,
        /// How many top-level `operation` keywords the *scanner* saw, counted before any
        /// declaration was parsed. The anti-drop guard.
        pub operation_keywords: usize,
    }

    /// Remove `//` comments and `"""` blocks, preserving string literals verbatim.
    ///
    /// Newlines are preserved so that a panic message's context is still findable by eye.
    ///
    /// # Panics
    ///
    /// On an unterminated `"""` block or string literal.
    #[must_use]
    pub fn elide(source: &str) -> String {
        let chars: Vec<char> = source.chars().collect();
        let mut out = String::with_capacity(source.len());
        let mut at = 0;
        while at < chars.len() {
            let here = chars[at];
            if here == '/' && chars.get(at + 1) == Some(&'/') {
                while at < chars.len() && chars[at] != '\n' {
                    at += 1;
                }
            } else if here == '"'
                && chars.get(at + 1) == Some(&'"')
                && chars.get(at + 2) == Some(&'"')
            {
                at += 3;
                let mut closed = false;
                while at + 2 < chars.len() {
                    if chars[at] == '"' && chars[at + 1] == '"' && chars[at + 2] == '"' {
                        at += 3;
                        closed = true;
                        break;
                    }
                    if chars[at] == '\n' {
                        out.push('\n');
                    }
                    at += 1;
                }
                assert!(closed, "an unterminated `\"\"\"` block");
            } else if here == '"' {
                out.push('"');
                at += 1;
                let mut closed = false;
                while at < chars.len() {
                    if chars[at] == '\\' && chars.get(at + 1) == Some(&'\\') {
                        out.push_str("\\\\");
                        at += 2;
                        continue;
                    }
                    out.push(chars[at]);
                    at += 1;
                    if chars[at - 1] == '"' {
                        closed = true;
                        break;
                    }
                }
                assert!(closed, "an unterminated string literal");
            } else {
                out.push(here);
                at += 1;
            }
        }
        out
    }

    /// The text between the `{` at `open` and its matching `}`, and the index just past that
    /// `}`. String literals are skipped so a brace inside one cannot unbalance the slice.
    ///
    /// # Panics
    ///
    /// When the braces do not balance.
    fn balanced(chars: &[char], open: usize) -> (String, usize) {
        assert_eq!(chars[open], '{', "a body opens with a brace");
        let mut depth = 0_usize;
        let mut at = open;
        let mut in_string = false;
        while at < chars.len() {
            let here = chars[at];
            if in_string {
                if here == '\\' {
                    at += 2;
                    continue;
                }
                if here == '"' {
                    in_string = false;
                }
            } else if here == '"' {
                in_string = true;
            } else if here == '{' {
                depth += 1;
            } else if here == '}' {
                depth -= 1;
                if depth == 0 {
                    return (chars[open + 1..at].iter().collect(), at + 1);
                }
            }
            at += 1;
        }
        panic!("unbalanced braces from offset {open}");
    }

    /// Split a declaration body into its `;`-terminated statements, at brace depth zero.
    fn statements(body: &str) -> Vec<String> {
        let chars: Vec<char> = body.chars().collect();
        let mut out = Vec::new();
        let mut buffer = String::new();
        let mut depth = 0_usize;
        let mut in_string = false;
        let mut at = 0;
        while at < chars.len() {
            let here = chars[at];
            if in_string {
                buffer.push(here);
                if here == '\\' {
                    at += 1;
                    if let Some(next) = chars.get(at) {
                        buffer.push(*next);
                    }
                } else if here == '"' {
                    in_string = false;
                }
            } else if here == '"' {
                in_string = true;
                buffer.push(here);
            } else if here == '{' || here == '[' {
                depth += 1;
                buffer.push(here);
            } else if here == '}' || here == ']' {
                depth -= 1;
                buffer.push(here);
            } else if here == ';' && depth == 0 {
                if !buffer.trim().is_empty() {
                    out.push(buffer.trim().to_owned());
                }
                buffer.clear();
            } else {
                buffer.push(here);
            }
            at += 1;
        }
        assert!(
            buffer.trim().is_empty(),
            "a declaration body ends mid-statement: {:?}",
            buffer.trim()
        );
        out
    }

    /// Read one `name: Type presence [@ann…]` statement.
    ///
    /// # Panics
    ///
    /// When the statement is not a field declaration.
    fn field(statement: &str) -> Field {
        let (name, rest) = statement
            .split_once(':')
            .unwrap_or_else(|| panic!("a field statement has a colon: {statement:?}"));
        let words: Vec<&str> = rest.split_whitespace().collect();
        let at = words
            .iter()
            .position(|word| matches!(*word, "required" | "optional" | "nullable"))
            .unwrap_or_else(|| panic!("a field statement declares a presence: {statement:?}"));
        assert!(at > 0, "a field statement declares a type: {statement:?}");
        Field {
            name: name.trim().to_owned(),
            ty: words[..at].concat(),
            presence: words[at].to_owned(),
        }
    }

    /// Parse the whole document.
    ///
    /// # Panics
    ///
    /// On any construct the IDL header's grammar does not describe.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn parse(source: &str) -> Document {
        let elided = elide(source);
        let mut document = Document {
            // The anti-drop count, taken textually and structurally independently of the
            // parse below: a top-level `operation` keyword starts a line in this layout.
            operation_keywords: elided
                .lines()
                .filter(|line| line.starts_with("operation "))
                .count(),
            ..Document::default()
        };

        let chars: Vec<char> = elided.chars().collect();
        let mut at = 0;
        let mut annotations: Vec<String> = Vec::new();

        let word = |chars: &[char], from: usize| -> (String, usize) {
            let mut end = from;
            while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '_') {
                end += 1;
            }
            (chars[from..end].iter().collect(), end)
        };
        let skip_space = |chars: &[char], from: usize| -> usize {
            let mut at = from;
            while at < chars.len() && chars[at].is_whitespace() {
                at += 1;
            }
            at
        };
        let to_semicolon = |chars: &[char], from: usize| -> (String, usize) {
            let mut end = from;
            while end < chars.len() && chars[end] != ';' {
                end += 1;
            }
            (chars[from..end].iter().collect(), end + 1)
        };

        loop {
            at = skip_space(&chars, at);
            if at >= chars.len() {
                break;
            }
            if chars[at] == '@' {
                let (name, next) = word(&chars, at + 1);
                at = skip_space(&chars, next);
                if at < chars.len() && chars[at] == '(' {
                    let mut depth = 0_usize;
                    while at < chars.len() {
                        if chars[at] == '(' {
                            depth += 1;
                        } else if chars[at] == ')' {
                            depth -= 1;
                            if depth == 0 {
                                at += 1;
                                break;
                            }
                        }
                        at += 1;
                    }
                }
                // `@since` dates a declaration; it describes no behaviour, so the sweep
                // drops it exactly as `idl_conformance.rs` does.
                if name != "since" {
                    annotations.push(name);
                }
                continue;
            }

            let (keyword, next) = word(&chars, at);
            assert!(!keyword.is_empty(), "a declaration starts with a keyword");
            at = skip_space(&chars, next);

            match keyword.as_str() {
                "scalar" => {
                    let (text, next) = to_semicolon(&chars, at);
                    document.scalars.push(text.trim().to_owned());
                    at = next;
                }
                "handle" => {
                    let (text, next) = to_semicolon(&chars, at);
                    let (name, prefix) = text
                        .split_once('=')
                        .unwrap_or_else(|| panic!("a handle declares a prefix: {text:?}"));
                    document.handles.push((
                        name.trim().to_owned(),
                        prefix.trim().trim_matches('"').to_owned(),
                    ));
                    at = next;
                }
                "alias" => {
                    let (text, next) = to_semicolon(&chars, at);
                    let (name, rest) = text
                        .split_once('=')
                        .unwrap_or_else(|| panic!("an alias declares a base: {text:?}"));
                    let base = rest.trim().split(['@', ' ']).next().unwrap_or("").trim();
                    document
                        .aliases
                        .push((name.trim().to_owned(), base.to_owned()));
                    at = next;
                }
                "protocol" | "rule" => {
                    // Neither carries structure this sweep compares: `protocol` is settings
                    // and `rule` bodies were elided above, leaving `{}`.
                    while at < chars.len() && chars[at] != '{' {
                        at += 1;
                    }
                    let (_, next) = balanced(&chars, at);
                    at = next;
                }
                "enum" => {
                    let (name, next) = word(&chars, at);
                    let open = skip_space(&chars, next);
                    let (_, next) = balanced(&chars, open);
                    document.enums.push(name);
                    at = next;
                }
                "struct" => {
                    let (name, next) = word(&chars, at);
                    let open = skip_space(&chars, next);
                    let (body, next) = balanced(&chars, open);
                    let fields = statements(&body).iter().map(|item| field(item)).collect();
                    document.structs.insert(name, fields);
                    at = next;
                }
                "union" => {
                    let (name, next) = word(&chars, at);
                    let open = skip_space(&chars, next);
                    let (body, next) = balanced(&chars, open);
                    let mut variants = Vec::new();
                    for item in body.split(',') {
                        let item = item.trim();
                        if item.is_empty() {
                            continue;
                        }
                        let (variant, ty) = item
                            .split_once('(')
                            .unwrap_or_else(|| panic!("a union variant carries a type: {item:?}"));
                        variants.push((
                            variant.trim().to_owned(),
                            ty.trim_end_matches(')').trim().to_owned(),
                        ));
                    }
                    document.unions.insert(name, variants);
                    at = next;
                }
                "operation" => {
                    let (namespace, next) = word(&chars, at);
                    assert_eq!(chars[next], '.', "an operation name is `namespace.verb`");
                    let (verb, next) = word(&chars, next + 1);
                    let open = skip_space(&chars, next);
                    let (body, next) = balanced(&chars, open);
                    let mut request = None;
                    let mut response = None;
                    for statement in statements(&body) {
                        let (clause, rest) = statement.split_once(char::is_whitespace).unwrap_or({
                            // `errors [];` splits on the bracket rather than on a space.
                            (
                                statement.split('[').next().unwrap_or("").trim(),
                                statement.split_once('[').map_or("", |(_, rest)| rest),
                            )
                        });
                        let rest = rest.trim();
                        let parsed = |rest: &str| -> Body {
                            rest.strip_prefix('{').map_or_else(
                                || Body::Named(rest.trim().to_owned()),
                                |_| {
                                    let inner = rest
                                        .trim()
                                        .trim_start_matches('{')
                                        .trim_end_matches('}')
                                        .to_owned();
                                    Body::Inline(
                                        statements(&inner).iter().map(|item| field(item)).collect(),
                                    )
                                },
                            )
                        };
                        match clause.trim() {
                            "request" => request = Some(parsed(rest)),
                            "response" => response = Some(parsed(rest)),
                            "authority" | "verdict" | "events" | "errors" => {}
                            other => panic!("unknown operation clause {other:?}"),
                        }
                    }
                    document.operations.push(Operation {
                        name: format!("{namespace}.{verb}"),
                        annotations: core::mem::take(&mut annotations),
                        request: request.expect("an operation declares a request"),
                        response: response.expect("an operation declares a response"),
                    });
                    at = next;
                }
                other => panic!("unknown declaration keyword {other:?}"),
            }
            if keyword != "operation" {
                annotations.clear();
            }
        }
        document
    }

    /// The IDL header's generated-name rule for an inline body: the operation name in
    /// PascalCase, plus the clause's suffix.
    #[must_use]
    pub fn generated_name(operation: &str, suffix: &str) -> String {
        let mut out = String::new();
        for part in operation.split(['.', '_']) {
            let mut characters = part.chars();
            if let Some(first) = characters.next() {
                out.extend(first.to_uppercase());
                out.push_str(characters.as_str());
            }
        }
        out.push_str(suffix);
        out
    }
}

/// Read and parse the normative IDL.
fn document() -> reader::Document {
    reader::parse(&std::fs::read_to_string(IDL_PATH).expect("the normative IDL is readable"))
}

// =========================================================================================
// 2. The classification
// =========================================================================================

/// What a leaf field's *declared type* makes it, before any adjudication.
///
/// The classification is a total function of the type name, so it cannot be argued with; the
/// arguable part lives in [`Naming`] and is tabulated exhaustively below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    /// One of the 19 declared `handle` classes. An explicit typed handle.
    Handle,
    /// `ArtifactHandle` — the class-agnostic handle alias, pattern-constrained to a class
    /// prefix. An explicit handle that does not fix the class.
    ArtifactHandle,
    /// `Commitment` — a content identity. `rule snapshot.by_reference` puts it on the same
    /// footing as a handle: "valid on a new connection, after a reconnection, and across a
    /// daemon upgrade within the major".
    Commitment,
    /// `EpochIdentity`, `ProtocolVersion`, `OperationName`, `ActorId`, `RequestId`,
    /// `PageToken`, `AuditCorrelationId` — typed identifiers of things that are not
    /// resources.
    TypedIdentifier,
    /// `SignerHandle` — a signer's name, the BLAKE3 content identity of its canonical record
    /// (ADR-0054 D5, `rule signing.identities`, protocol 3.8). A signer is held by the daemon
    /// and is not a plan §4.4 artifact class, so it has no handle class; its name is a content
    /// identity on the same footing as a `Commitment`, and names the signer as a handle would.
    SignerName,
    /// `Opaque` or `Bytes`.
    Inline,
    /// `Bool`, `U32`, `U64`, `Timestamp`, `DurationMs`, `ByteCount`, or an enum member.
    Scalar,
    /// `String`, `list<String>`, or `map<String,String>`.
    PlainString,
}

/// One classified leaf of one body.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Leaf {
    /// The operation, or `<envelope>` for the two envelopes.
    operation: String,
    /// `request` or `response`.
    direction: &'static str,
    /// The dotted path from the body root; union variants read `field|variant`.
    path: String,
    /// The struct that declares this field — the adjudication key's first half.
    declared_by: String,
    /// The field name — the adjudication key's second half.
    field: String,
    /// The declared type.
    ty: String,
    /// The class of that type.
    class: Class,
}

/// The element type of a `list<…>`/`map<…,…>`, or the type itself.
fn element(ty: &str) -> &str {
    if let Some(rest) = ty.strip_prefix("list<") {
        return rest.trim_end_matches('>');
    }
    if let Some(rest) = ty.strip_prefix("map<") {
        return rest.trim_end_matches('>').split(',').nth(1).unwrap_or("");
    }
    ty
}

/// The sweep's state: what the IDL declares, indexed for the walk.
struct Vocabulary {
    handles: BTreeSet<String>,
    enums: BTreeSet<String>,
    aliases: BTreeMap<String, String>,
    structs: BTreeMap<String, Vec<reader::Field>>,
    unions: BTreeMap<String, Vec<(String, String)>>,
}

impl Vocabulary {
    fn of(document: &reader::Document) -> Self {
        Self {
            handles: document
                .handles
                .iter()
                .map(|(name, _)| name.clone())
                .collect(),
            enums: document.enums.iter().cloned().collect(),
            aliases: document.aliases.iter().cloned().collect(),
            structs: document.structs.clone(),
            unions: document.unions.clone(),
        }
    }

    /// Classify a declared type. Panics on a type the IDL does not declare, so a renamed
    /// type is a failure here rather than a silently unclassified leaf.
    fn classify(&self, ty: &str) -> Option<Class> {
        let base = element(ty);
        if self.handles.contains(base) {
            return Some(Class::Handle);
        }
        if base == "ArtifactHandle" {
            return Some(Class::ArtifactHandle);
        }
        if base == "Commitment" {
            return Some(Class::Commitment);
        }
        if base == "SignerHandle" {
            return Some(Class::SignerName);
        }
        if base == "String" {
            return Some(Class::PlainString);
        }
        if matches!(base, "Opaque" | "Bytes") {
            return Some(Class::Inline);
        }
        if matches!(
            base,
            "Bool" | "U32" | "U64" | "Timestamp" | "DurationMs" | "ByteCount"
        ) {
            return Some(Class::Scalar);
        }
        if self.aliases.contains_key(base) {
            return Some(Class::TypedIdentifier);
        }
        if self.enums.contains(base) {
            return Some(Class::Scalar);
        }
        // A struct or a union: not a leaf, the walk recurses.
        if self.structs.contains_key(base) || self.unions.contains_key(base) {
            return None;
        }
        panic!("the IDL declares no type named {base:?}");
    }
}

/// Where one leaf sits: which operation, which direction, and which body it was reached
/// through. Bundled so the recursion below carries one value rather than three.
#[derive(Debug, Clone, Copy)]
struct Site<'a> {
    /// The operation, or `<envelope>`.
    operation: &'a str,
    /// `request` or `response`.
    direction: &'static str,
    /// The dotted path so far, empty at the body root.
    prefix: &'a str,
    /// The struct declaring the fields being walked.
    declared_by: &'a str,
}

/// The walk's accumulator: what the IDL declares, and the leaves found so far.
struct Walk<'a> {
    vocabulary: &'a Vocabulary,
    leaves: Vec<Leaf>,
}

impl Walk<'_> {
    /// Walk one body to its leaves, recursing through named structs and unions.
    ///
    /// `seen` is the set of struct names on the current path; a type already on it is a
    /// cycle and is not descended into again.
    fn body(&mut self, site: Site<'_>, fields: &[reader::Field], seen: &BTreeSet<String>) {
        for field in fields {
            let base = element(&field.ty).to_owned();
            let path = if site.prefix.is_empty() {
                field.name.clone()
            } else {
                format!("{}.{}", site.prefix, field.name)
            };
            match self.vocabulary.classify(&field.ty) {
                Some(class) => self.leaves.push(Leaf {
                    operation: site.operation.to_owned(),
                    direction: site.direction,
                    path,
                    declared_by: site.declared_by.to_owned(),
                    field: field.name.clone(),
                    ty: field.ty.clone(),
                    class,
                }),
                None if self.vocabulary.structs.contains_key(&base) => {
                    if seen.contains(&base) {
                        continue;
                    }
                    let mut deeper = seen.clone();
                    deeper.insert(base.clone());
                    let inner = self.vocabulary.structs[&base].clone();
                    self.body(
                        Site {
                            prefix: &path,
                            declared_by: &base,
                            ..site
                        },
                        &inner,
                        &deeper,
                    );
                }
                None => self.union(site, &base, &path, seen),
            }
        }
    }

    /// Walk a union's variants: a struct variant is descended into, anything else is a leaf.
    fn union(&mut self, site: Site<'_>, name: &str, prefix: &str, seen: &BTreeSet<String>) {
        for (variant, ty) in self.vocabulary.unions[name].clone() {
            let inner = element(&ty).to_owned();
            let path = format!("{prefix}|{variant}");
            if self.vocabulary.structs.contains_key(&inner) && !seen.contains(&inner) {
                let mut deeper = seen.clone();
                deeper.insert(inner.clone());
                let fields = self.vocabulary.structs[&inner].clone();
                self.body(
                    Site {
                        prefix: &path,
                        declared_by: &inner,
                        ..site
                    },
                    &fields,
                    &deeper,
                );
            } else {
                self.leaves.push(Leaf {
                    operation: site.operation.to_owned(),
                    direction: site.direction,
                    path,
                    declared_by: name.to_owned(),
                    field: variant,
                    class: self
                        .vocabulary
                        .classify(&ty)
                        .expect("a union variant of a non-struct type is a leaf"),
                    ty,
                });
            }
        }
    }
}

/// The body of one clause: its declaring struct name and its fields.
fn body<'a>(
    vocabulary: &'a Vocabulary,
    operation: &str,
    clause: &'a reader::Body,
    suffix: &str,
) -> (String, &'a [reader::Field]) {
    match clause {
        reader::Body::Named(name) => (
            name.clone(),
            vocabulary
                .structs
                .get(name)
                .unwrap_or_else(|| panic!("the IDL declares struct {name:?}"))
                .as_slice(),
        ),
        reader::Body::Inline(fields) => {
            (reader::generated_name(operation, suffix), fields.as_slice())
        }
    }
}

/// Sweep every operation body and both envelopes.
fn sweep(document: &reader::Document) -> Vec<Leaf> {
    let vocabulary = Vocabulary::of(document);
    let mut walk = Walk {
        vocabulary: &vocabulary,
        leaves: Vec::new(),
    };
    for operation in &document.operations {
        for (clause, direction, suffix) in [
            (&operation.request, "request", "Request"),
            (&operation.response, "response", "Response"),
        ] {
            let (name, fields) = body(&vocabulary, &operation.name, clause, suffix);
            let mut seen = BTreeSet::new();
            seen.insert(name.clone());
            walk.body(
                Site {
                    operation: &operation.name,
                    direction,
                    prefix: "",
                    declared_by: &name,
                },
                fields,
                &seen,
            );
        }
    }
    // The envelopes are the other half of the native API and carry three of its handles.
    for (name, direction) in [
        ("RequestEnvelope", "request"),
        ("ResultEnvelope", "response"),
    ] {
        let mut seen = BTreeSet::new();
        seen.insert(name.to_owned());
        walk.body(
            Site {
                operation: "<envelope>",
                direction,
                prefix: "",
                declared_by: name,
            },
            &vocabulary.structs[name],
            &seen,
        );
    }
    walk.leaves
}

// =========================================================================================
// 3. The adjudication
// =========================================================================================

/// What a non-handle, non-scalar leaf actually names.
///
/// Only the last two are findings. The distinction the criterion turns on is *resource*: a
/// thing the daemon holds, whose identity a later call must be able to state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Naming {
    /// Free text, a caller-minted correlation token, or a rationale. Names nothing.
    NamesNoResource,
    /// A member of a vocabulary this IDL does not declare as an enum — a §4.4 artifact-class
    /// prefix, a guarantee class, an observer projection, a checker identity. Not a
    /// resource; recorded because it is a *typed-vocabulary* gap, which is a different
    /// criterion (G2-02) from this one.
    Vocabulary,
    /// A position or element **inside** an artifact that a handle in the same call — in the
    /// request body or on the envelope — already named. A coordinate, not a name.
    Coordinate,
    /// A document travelling by value **into** the daemon. There is no handle for it yet
    /// because the daemon does not hold it yet.
    InboundDocument,
    /// An artifact's content returned in a call that also names its handle. Content beside
    /// a name, which is what a read is.
    ContentBesideAHandle,
    /// The encoding seam: an `Opaque` whose content is a declared struct of this same IDL,
    /// resolved by a `required` field of the same object (`rule encoding.opaque_payloads`).
    EncodedTypedBody,
    /// A string whose domain a **named rule** of the IDL declares — its grammar, what it
    /// names, and the refusal outside it — and which, by that rule's stated reason, has no
    /// handle class of its own. Not a finding, but not trusted on this table's word either:
    /// every such site is listed in [`DECLARED_DOMAINS`] with its rule, and
    /// [`every_declared_domain_is_a_rule_that_cites_its_site`] reads the IDL to confirm the
    /// rule exists and cites the site.
    DeclaredDomain,
    /// **FINDING.** Names something the daemon itself holds or registers, and does not use a
    /// declared handle class to do it.
    ResourceWithoutAHandle,
    /// **FINDING.** The declaration does not say what the value names, so a reader cannot
    /// tell whether a resource is being named without a handle.
    UndeclaredDomain,
}

impl Naming {
    /// Whether this adjudication is a finding against the criterion.
    const fn is_finding(self) -> bool {
        matches!(self, Self::ResourceWithoutAHandle | Self::UndeclaredDomain)
    }
}

/// Every `String`-typed declaration site the sweep reaches, adjudicated.
///
/// Keyed by `(declaring struct, field)`. `String` is the IDL's escape hatch, so this is the
/// table that decides the criterion: a resource named here rather than by a handle is exactly
/// what "explicit handles across native API" forbids.
const STRING_SITES: &[(&str, &str, Naming)] = &[
    // --- benchmark: was a finding (a task and graders by raw string); paid by bn-ah1k8 ---
    // `task_id` is the suite's task name under `rule benchmark.task_identity`, which also
    // says why no content handle is minted: one over the bundle is a function of its
    // hidden half. `graders` is `rule benchmark.graders`'s closed seven-stage vocabulary.
    ("BenchmarkRunRequest", "graders", Naming::DeclaredDomain),
    ("BenchmarkRunRequest", "task_id", Naming::DeclaredDomain),
    // --- context ------------------------------------------------------------------------
    ("ContextCompileRequest", "guarantees", Naming::Vocabulary),
    ("ContextCompileRequest", "question", Naming::NamesNoResource),
    // The anchor is inside the pack `context: ContextHandle` names in the same request.
    ("ContextExpandRequest", "anchor", Naming::Coordinate),
    // --- correspondence: elements inside the envelope's snapshot -------------------------
    ("CorrespondenceBindRequest", "element", Naming::Coordinate),
    ("CorrespondenceDriftResponse", "drifted", Naming::Coordinate),
    ("CorrespondenceStatusRequest", "element", Naming::Coordinate),
    ("Cost", "tokenizer_id", Naming::Vocabulary),
    // --- debug: events and observers inside the branch `DebugHandle` names ---------------
    ("DebugBranchRequest", "alternate", Naming::Coordinate),
    ("DebugCompareRequest", "observer", Naming::Vocabulary),
    ("DebugEnabledResponse", "events", Naming::Coordinate),
    ("DebugOpenRequest", "observer", Naming::Vocabulary),
    ("DebugStateRequest", "observer", Naming::Vocabulary),
    ("DebugStepEventRequest", "event", Naming::Coordinate),
    ("DebugWhyBlockedRequest", "event", Naming::Coordinate),
    ("DebugWhyEnabledRequest", "event", Naming::Coordinate),
    ("Diagnostic", "code", Naming::Vocabulary),
    ("Diagnostic", "detail", Naming::NamesNoResource),
    // --- evidence -----------------------------------------------------------------------
    ("EvidenceLinkRequest", "checker_profile", Naming::Vocabulary),
    ("EvidenceLinkResponse", "checker", Naming::Vocabulary),
    // Was "claim identity **or** property identifier", two readings and neither fixed.
    // `rule evidence.claim_identity` fixes the first: a node-grouping key, never resolved
    // from a property identifier (bn-ah1k8).
    ("EvidenceQuery", "claim_id", Naming::DeclaredDomain),
    ("EvidenceVerifyResponse", "checker", Naming::Vocabulary),
    (
        "EvidenceVerifyResponse",
        "validation_basis",
        Naming::Vocabulary,
    ),
    // --- failure ------------------------------------------------------------------------
    ("FailureBranchRequest", "alternate", Naming::Coordinate),
    ("FailureMinimizeRequest", "guarantee", Naming::Vocabulary),
    ("FailureMinimizeResponse", "guarantees", Naming::Vocabulary),
    // --- workspace: placement inside the snapshot being built, paired with a commitment --
    ("FileComponent", "path", Naming::Coordinate),
    ("FileOverlay", "path", Naming::Coordinate),
    // --- forge --------------------------------------------------------------------------
    ("ForgeArchiveRequest", "descriptors", Naming::Vocabulary),
    (
        "ForgeCreateRequest",
        "diversity_descriptors",
        Naming::Vocabulary,
    ),
    ("ForgeCreateRequest", "objectives", Naming::Vocabulary),
    // --- intent: the policy table is field-to-verb over the contract `IntentHandle` names -
    ("IntentChangeSet", "rationale", Naming::NamesNoResource),
    ("IntentLockRequest", "policy", Naming::Coordinate),
    ("IntentLockResponse", "policy", Naming::Coordinate),
    ("IntentRejectRequest", "reason", Naming::NamesNoResource),
    ("Milestone", "name", Naming::Vocabulary),
    (
        "ObserveIngestRequest",
        "instrumentation_profile",
        Naming::Vocabulary,
    ),
    // --- program / proof: coordinates inside the envelope's snapshot ---------------------
    ("ProgramExtractRequest", "roots", Naming::Coordinate),
    ("ProgramRunRequest", "entry", Naming::Coordinate),
    // "Tactic script or term, treated strictly as data" — the IDL says so itself.
    ("ProofAttemptRequest", "step", Naming::NamesNoResource),
    ("ProofGoalRequest", "obligation", Naming::Coordinate),
    ("ProofSliceResponse", "axioms", Naming::Coordinate),
    ("ProofSliceResponse", "declarations", Naming::Coordinate),
    // --- query: was the response-direction finding; paid by bn-ah1k8 ---------------------
    // `rule query.invalidation_edges` spells an edge `<handle>:<reason>` over a sibling
    // `ArtifactHandle`: the artifact is named by its handle, and the index record is a cache
    // entry with no identity to publish. `rule query.reuse_reasons` keys `reasons` by the
    // handles of `reused` and `recomputed` and closes the values.
    (
        "QueryExplainInvalidationResponse",
        "edges",
        Naming::DeclaredDomain,
    ),
    (
        "QueryExplainReuseResponse",
        "reasons",
        Naming::DeclaredDomain,
    ),
    ("Redacted", "original_class", Naming::Vocabulary),
    ("RefinementCheckRequest", "observer", Naming::Vocabulary),
    ("RepairApplyRequest", "hypothesis", Naming::NamesNoResource),
    ("RepairRejectRequest", "reason", Naming::NamesNoResource),
    ("SourceSpan", "file", Naming::Coordinate),
    // `Target` is `(kind, id)`: a kind-tagged coordinate inside the snapshot and intent the
    // envelope names. Seven of `TargetKind`'s eight members are intra-snapshot; the eighth,
    // `benchmark_task`, carries a task name under `rule benchmark.task_identity`, the rule
    // that governs `BenchmarkRunRequest.task_id`.
    ("Target", "id", Naming::Coordinate),
    (
        "TaskRecord",
        "non_resumable_reason",
        Naming::NamesNoResource,
    ),
    // --- the envelopes -------------------------------------------------------------------
    // `kind` labels the class; the sibling `handle: ArtifactHandle` names the artifact.
    ("ArtifactRef", "kind", Naming::Vocabulary),
    ("Error", "detail", Naming::NamesNoResource),
    ("Error", "non_resumable_reason", Naming::NamesNoResource),
    ("NextOperation", "rationale", Naming::NamesNoResource),
    // "What was omitted, in typed terms" — and `recoverable_by: ArtifactHandle` carries the
    // handle whenever one exists, so the resource is named by a handle where there is one.
    ("Omission", "subject", Naming::Vocabulary),
    ("ProducedDimension", "engine", Naming::Vocabulary),
    ("ProducedDimension", "summary", Naming::NamesNoResource),
    (
        "RequestEnvelope",
        "idempotency_key",
        Naming::NamesNoResource,
    ),
    ("TraceContext", "traceparent", Naming::NamesNoResource),
    ("TraceContext", "tracestate", Naming::NamesNoResource),
    ("UnsupportedDimension", "reason", Naming::Vocabulary),
    ("Warning", "code", Naming::Vocabulary),
    ("Warning", "detail", Naming::NamesNoResource),
];

/// Every `Opaque`/`Bytes` declaration site the sweep reaches, adjudicated.
///
/// The question here is the mirror of [`STRING_SITES`]': does a blob stand *in place of* a
/// handle, or beside one? [`inline_content_never_travels_without_a_handle_in_the_same_call`]
/// checks the response half mechanically rather than trusting this table.
const INLINE_SITES: &[(&str, &str, Naming)] = &[
    (
        "ContextCompileResponse",
        "pack",
        Naming::ContentBesideAHandle,
    ),
    (
        "ContextExpandResponse",
        "pack",
        Naming::ContentBesideAHandle,
    ),
    (
        "DebugOpenResponse",
        "frontier",
        Naming::ContentBesideAHandle,
    ),
    (
        "DebugReverseCausalResponse",
        "delta",
        Naming::ContentBesideAHandle,
    ),
    ("DebugStateResponse", "state", Naming::ContentBesideAHandle),
    (
        "DebugStepAbstractResponse",
        "delta",
        Naming::ContentBesideAHandle,
    ),
    (
        "DebugStepEventResponse",
        "delta",
        Naming::ContentBesideAHandle,
    ),
    ("EvidenceGetResponse", "edge", Naming::ContentBesideAHandle),
    ("EvidenceGetResponse", "node", Naming::ContentBesideAHandle),
    (
        "IntentAcceptResponse",
        "record",
        Naming::ContentBesideAHandle,
    ),
    (
        "IntentDiffResponse",
        "summary",
        Naming::ContentBesideAHandle,
    ),
    (
        "IntentGetResponse",
        "contract",
        Naming::ContentBesideAHandle,
    ),
    ("IntentGetResponse", "record", Naming::ContentBesideAHandle),
    (
        "ModelCompareResponse",
        "summary",
        Naming::ContentBesideAHandle,
    ),
    (
        "ObserveClassifyResponse",
        "classification",
        Naming::ContentBesideAHandle,
    ),
    (
        "ProgramReplayResponse",
        "divergence",
        Naming::ContentBesideAHandle,
    ),
    (
        "ProofCheckResponse",
        "proof_receipt",
        Naming::ContentBesideAHandle,
    ),
    ("ProofGoalResponse", "goal", Naming::ContentBesideAHandle),
    (
        "RepairPromoteResponse",
        "promotion_receipt",
        Naming::ContentBesideAHandle,
    ),
    (
        "WorkspaceDiffResponse",
        "summary",
        Naming::ContentBesideAHandle,
    ),
    // --- inbound: content by value, because the daemon does not hold it yet --------------
    ("FileOverlay", "content", Naming::InboundDocument),
    ("ForgeCreateRequest", "sketch", Naming::InboundDocument),
    ("IntentAcceptRequest", "acceptance", Naming::InboundDocument),
    ("IntentChangeSet", "changes", Naming::InboundDocument),
    (
        "ProgramRunRequest",
        "configuration",
        Naming::InboundDocument,
    ),
    ("RepairApplyRequest", "changes", Naming::InboundDocument),
    ("WhiteboardCompileRequest", "note", Naming::InboundDocument),
    // --- protocol 3.8, the signing wire (bn-3glnv) ---------------------------------------
    // A bundle, a pack, or an artifact and its signature travel in; the daemon holds none
    // of them yet. Everything that travels out sits beside the bundle handle or the signer
    // name it is about.
    (
        "IntentImportBundleRequest",
        "content",
        Naming::InboundDocument,
    ),
    ("SigningSignPackRequest", "pack", Naming::InboundDocument),
    ("SigningVerifyRequest", "artifact", Naming::InboundDocument),
    ("SigningVerifyRequest", "signature", Naming::InboundDocument),
    (
        "EvidenceGetResponse",
        "signature",
        Naming::ContentBesideAHandle,
    ),
    (
        "IntentExportBundleResponse",
        "content",
        Naming::ContentBesideAHandle,
    ),
    (
        "SigningMintResponse",
        "registry_head",
        Naming::ContentBesideAHandle,
    ),
    (
        "SigningMintResponse",
        "public_key",
        Naming::ContentBesideAHandle,
    ),
    (
        "SigningRegistryResponse",
        "allowed",
        Naming::ContentBesideAHandle,
    ),
    (
        "SigningRegistryResponse",
        "registry_head",
        Naming::ContentBesideAHandle,
    ),
    (
        "SigningRegistryResponse",
        "log",
        Naming::ContentBesideAHandle,
    ),
    (
        "SigningRevokeResponse",
        "registry_head",
        Naming::ContentBesideAHandle,
    ),
    (
        "SigningRotateResponse",
        "registry_head",
        Naming::ContentBesideAHandle,
    ),
    (
        "SigningRotateResponse",
        "public_key",
        Naming::ContentBesideAHandle,
    ),
    (
        "SigningSignPackResponse",
        "signature",
        Naming::ContentBesideAHandle,
    ),
    // --- the encoding seam ---------------------------------------------------------------
    ("Error", "data", Naming::EncodedTypedBody),
    ("NextOperation", "arguments", Naming::EncodedTypedBody),
    ("RequestEnvelope", "arguments", Naming::EncodedTypedBody),
    ("ResultEnvelope", "payload", Naming::EncodedTypedBody),
];

/// Look one site up in a table.
fn adjudication(table: &[(&str, &str, Naming)], declared_by: &str, field: &str) -> Option<Naming> {
    table
        .iter()
        .find(|(owner, name, _)| *owner == declared_by && *name == field)
        .map(|(_, _, naming)| *naming)
}

/// Every [`Naming::DeclaredDomain`] site, with the IDL rule that declares its domain.
///
/// Keyed like [`STRING_SITES`]. The rule body must cite the site as `` `Owner.field` ``, so
/// the rule cannot drift away from the field it governs without this file seeing it.
const DECLARED_DOMAINS: &[(&str, &str, &str)] = &[
    ("BenchmarkRunRequest", "graders", "benchmark.graders"),
    ("BenchmarkRunRequest", "task_id", "benchmark.task_identity"),
    ("EvidenceQuery", "claim_id", "evidence.claim_identity"),
    (
        "QueryExplainInvalidationResponse",
        "edges",
        "query.invalidation_edges",
    ),
    (
        "QueryExplainReuseResponse",
        "reasons",
        "query.reuse_reasons",
    ),
];

/// The `"""` body of `rule <name>`, read from the raw IDL text.
///
/// [`reader`] elides rule bodies, so this reads the text directly: the declaration line
/// `rule <name> {` at the start of a line, then the first `"""` block after it.
fn rule_body(source: &str, name: &str) -> Option<String> {
    let anchor = format!("\nrule {name} {{");
    let start = source.find(&anchor)? + anchor.len();
    let rest = &source[start..];
    let open = rest.find("\"\"\"")? + 3;
    let close = rest[open..].find("\"\"\"")? + open;
    Some(rest[open..close].to_owned())
}

/// The declared-domain sites whose rule is missing or does not cite them, for a given IDL
/// text. Empty is the clean state.
fn undeclared_domains(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (owner, field, rule) in DECLARED_DOMAINS {
        match rule_body(source, rule) {
            None => out.push(format!("{owner}.{field}: `rule {rule}` is not declared")),
            Some(body) if !body.contains(&format!("`{owner}.{field}`")) => {
                out.push(format!("{owner}.{field}: `rule {rule}` does not cite it"));
            }
            Some(_) => {}
        }
    }
    out
}

// --- the counts this file pins, so a protocol edit is visible here ------------------------

/// Leaf fields the sweep classifies, over 168 bodies. 666 until protocol 3.8 (bn-3glnv),
/// whose eight operations and one field add 36: 15 inline, 11 signer names, 4 handles, and 6
/// scalars.
const LEAVES: usize = 702;
/// Distinct `(declaring struct, field)` sites among those leaves.
const SITES: usize = 446;
/// Leaves whose declared type is one of the 19 handle classes.
const HANDLE_LEAVES: usize = 210;
/// Leaves whose declared type is `SignerHandle`, a signer's content-identity name (3.8).
const SIGNER_NAME_LEAVES: usize = 11;
/// Leaves whose declared type is the class-agnostic `ArtifactHandle`.
const ARTIFACT_HANDLE_LEAVES: usize = 12;
/// Leaves whose declared type is `Commitment`.
const COMMITMENT_LEAVES: usize = 21;
/// Leaves whose declared type is `String`, `list<String>`, or `map<String,String>` — the
/// class every finding lives in, and the denominator the six of them are six of.
const STRING_LEAVES: usize = 119;
/// Leaves whose declared type is `Opaque` or `Bytes`.
const INLINE_LEAVES: usize = 48;

// =========================================================================================
// 4. The IDL sweep — tests
// =========================================================================================

#[test]
fn the_reader_recovers_the_whole_document() {
    let document = document();
    // Counted twice by two different methods: once by the scanner's textual keyword count,
    // once by the declarations it actually produced. A reader that dropped a declaration
    // disagrees with itself here rather than passing the rest of the file vacuously.
    assert_eq!(
        document.operations.len(),
        document.operation_keywords,
        "the reader parsed fewer operations than the file declares"
    );
    assert_eq!(document.operations.len(), 83, "operations");
    assert_eq!(document.handles.len(), 19, "declared handle classes");
    assert_eq!(document.structs.len(), 47, "named structs");
    assert_eq!(document.unions.len(), 2, "unions");
    assert_eq!(document.scalars.len(), 9, "scalars");
    assert_eq!(document.aliases.len(), 10, "aliases");
    assert_eq!(document.enums.len(), 37, "enums");

    let names: BTreeSet<&str> = document
        .operations
        .iter()
        .map(|operation| operation.name.as_str())
        .collect();
    assert_eq!(names.len(), 83, "no operation is declared twice");
}

/// The independent completeness anchor: plan §10.2's own registry.
///
/// `idl_conformance.rs` reads the IDL and `registry_agreement.rs` reads RFC 0027 and RFC
/// 0026. Neither reads this block, and `rule conformance.registry_agreement` makes it
/// normative: "The operation set declared here MUST equal the plan §10.2 registry exactly".
#[test]
fn the_sweep_is_exhaustive_over_the_registry() {
    let plan = std::fs::read_to_string(PLAN_PATH).expect("the plan is readable");
    let start = plan
        .find("### 10.2 Core operations")
        .expect("the plan has a §10.2 heading");
    let block = &plan[start..];
    let open = block.find("```text").expect("§10.2 opens a text block") + "```text".len();
    let close = block[open..].find("```").expect("§10.2 closes its block") + open;

    let mut registry: Vec<String> = Vec::new();
    let mut namespace = String::new();
    for line in block[open..close].lines() {
        if line.trim().is_empty() {
            continue;
        }
        let continuation = line.starts_with(char::is_whitespace);
        let mut verbs: Vec<&str> = line.split('/').map(str::trim).collect();
        if !continuation {
            let head = verbs.remove(0);
            let (found, verb) = head
                .split_once('.')
                .unwrap_or_else(|| panic!("a §10.2 line opens with `namespace.verb`: {line:?}"));
            namespace = found.to_owned();
            registry.push(format!("{namespace}.{verb}"));
        }
        for verb in verbs {
            if !verb.is_empty() {
                registry.push(format!("{namespace}.{verb}"));
            }
        }
    }

    let idl: Vec<String> = document()
        .operations
        .into_iter()
        .map(|operation| operation.name)
        .collect();
    assert_eq!(registry.len(), 83, "plan §10.2 lists 83 operations");
    assert_eq!(
        registry, idl,
        "plan §10.2 and the IDL declare different operation registries"
    );
}

#[test]
fn every_body_of_every_operation_is_swept() {
    let document = document();
    let leaves = sweep(&document);
    assert_eq!(leaves.len(), LEAVES, "classified leaf fields");

    let swept: BTreeSet<&str> = leaves
        .iter()
        .map(|leaf| leaf.operation.as_str())
        .filter(|name| *name != "<envelope>")
        .collect();
    assert_eq!(
        swept.len(),
        document.operations.len(),
        "every operation contributed at least one leaf"
    );
    // One body is declared empty: `signing.registry`'s request (protocol 3.8), which reads a
    // deployment singleton and names nothing. An empty body contributes no leaf by
    // construction, so it is required to be *declared* empty — an inline body with no field —
    // rather than excused by name; a reader that dropped a body's fields would still fail.
    let mut empty_bodies = Vec::new();
    for operation in &document.operations {
        for (direction, body) in [
            ("request", &operation.request),
            ("response", &operation.response),
        ] {
            let contributed = leaves
                .iter()
                .any(|leaf| leaf.operation == operation.name && leaf.direction == direction);
            if !contributed {
                assert!(
                    matches!(body, reader::Body::Inline(fields) if fields.is_empty()),
                    "{} {direction} contributed no leaf",
                    operation.name
                );
                empty_bodies.push(format!("{} {direction}", operation.name));
            }
        }
    }
    assert_eq!(
        empty_bodies,
        ["signing.registry request"],
        "the empty bodies moved"
    );
    assert!(
        leaves.iter().any(|leaf| leaf.operation == "<envelope>"),
        "both envelopes are swept"
    );

    let sites: BTreeSet<(&str, &str)> = leaves
        .iter()
        .map(|leaf| (leaf.declared_by.as_str(), leaf.field.as_str()))
        .collect();
    assert_eq!(sites.len(), SITES, "distinct declaration sites");
}

#[test]
fn the_resource_naming_types_are_the_handle_vocabulary() {
    let leaves = sweep(&document());
    let count = |class: Class| leaves.iter().filter(|leaf| leaf.class == class).count();
    assert_eq!(count(Class::Handle), HANDLE_LEAVES);
    assert_eq!(count(Class::ArtifactHandle), ARTIFACT_HANDLE_LEAVES);
    assert_eq!(count(Class::Commitment), COMMITMENT_LEAVES);
    assert_eq!(count(Class::PlainString), STRING_LEAVES);
    assert_eq!(count(Class::Inline), INLINE_LEAVES);
    assert_eq!(count(Class::SignerName), SIGNER_NAME_LEAVES);
    // 254 of 702 leaves name a resource, and every one of them does it with a handle or a
    // content identity. That is the criterion's positive half as a number. It was 239 of
    // 666 until protocol 3.8 added four handles and eleven signer names.
    assert_eq!(
        HANDLE_LEAVES + ARTIFACT_HANDLE_LEAVES + COMMITMENT_LEAVES + SIGNER_NAME_LEAVES,
        254
    );
    // Every handle-typed leaf names one of the 19 declared classes; none is a bare string
    // that happens to look like a handle.
    let declared: BTreeSet<String> = document()
        .handles
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    for leaf in &leaves {
        if leaf.class == Class::Handle {
            assert!(
                declared.contains(element(&leaf.ty)),
                "{}.{} declares {:?}, which is not a handle class",
                leaf.declared_by,
                leaf.field,
                leaf.ty
            );
        }
    }
}

/// The exhaustiveness gate. A `String` or `Opaque` field added to the IDL fails here until
/// it is adjudicated, which is what keeps the tables from drifting into decoration.
#[test]
fn every_string_and_inline_site_is_adjudicated() {
    let leaves = sweep(&document());
    let mut unadjudicated = Vec::new();
    let mut string_sites = BTreeSet::new();
    let mut inline_sites = BTreeSet::new();
    for leaf in &leaves {
        let (table, seen) = match leaf.class {
            Class::PlainString => (STRING_SITES, &mut string_sites),
            Class::Inline => (INLINE_SITES, &mut inline_sites),
            _ => continue,
        };
        seen.insert((leaf.declared_by.clone(), leaf.field.clone()));
        if adjudication(table, &leaf.declared_by, &leaf.field).is_none() {
            unadjudicated.push(format!(
                "{} {} {} ({}) — declared by {}.{}",
                leaf.operation, leaf.direction, leaf.path, leaf.ty, leaf.declared_by, leaf.field
            ));
        }
    }
    assert!(
        unadjudicated.is_empty(),
        "these sites carry no adjudication:\n  {}",
        unadjudicated.join("\n  ")
    );
    assert_eq!(string_sites.len(), 66, "adjudicated `String` sites");
    assert_eq!(inline_sites.len(), 46, "adjudicated `Opaque`/`Bytes` sites");
    assert_eq!(STRING_SITES.len(), string_sites.len(), "no dead table rows");
    assert_eq!(INLINE_SITES.len(), inline_sites.len(), "no dead table rows");
}

/// The verdict, as an assertion: no finding remains.
///
/// Until bn-ah1k8 this pinned six leaves over five sites — `benchmark.run`'s `task_id` and
/// `graders`, `EvidenceQuery.claim_id` (reached by `evidence.query` and
/// `evidence.subscribe`), and the `edges` and `reasons` of the two explain operations. bn-ah1k8
/// paid all six by declaring each domain in a named rule ([`DECLARED_DOMAINS`]). The pin is
/// now a regression guard: a new `String` site adjudicated as a finding turns it red.
#[test]
fn no_leaf_names_a_resource_without_a_handle_or_a_declared_domain() {
    let leaves = sweep(&document());
    let mut findings: Vec<(String, &'static str, String, Naming)> = Vec::new();
    for leaf in &leaves {
        let table = match leaf.class {
            Class::PlainString => STRING_SITES,
            Class::Inline => INLINE_SITES,
            _ => continue,
        };
        let naming =
            adjudication(table, &leaf.declared_by, &leaf.field).expect("every site is adjudicated");
        if naming.is_finding() {
            findings.push((
                leaf.operation.clone(),
                leaf.direction,
                leaf.path.clone(),
                naming,
            ));
        }
    }
    findings.sort();
    assert_eq!(findings, Vec::new(), "a finding re-entered the protocol");
    // The six leaves that were findings are still there, still `String`, and each now sits
    // on a declared-domain site: the payment moved their adjudication, not their type.
    let declared: Vec<(String, String)> = leaves
        .iter()
        .filter(|leaf| {
            leaf.class == Class::PlainString
                && adjudication(STRING_SITES, &leaf.declared_by, &leaf.field)
                    == Some(Naming::DeclaredDomain)
        })
        .map(|leaf| (leaf.operation.clone(), leaf.path.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let expected: Vec<(String, String)> = [
        ("benchmark.run", "graders"),
        ("benchmark.run", "task_id"),
        ("evidence.query", "query.claim_id"),
        ("evidence.subscribe", "scope.claim_id"),
        ("query.explain_invalidation", "edges"),
        ("query.explain_reuse", "reasons"),
    ]
    .iter()
    .map(|(operation, path)| ((*operation).to_owned(), (*path).to_owned()))
    .collect();
    assert_eq!(declared, expected, "the declared-domain leaves moved");
}

/// Each declared-domain site's rule exists in the IDL and cites the site, and the table and
/// [`STRING_SITES`] agree on which sites are declared-domain sites.
#[test]
fn every_declared_domain_is_a_rule_that_cites_its_site() {
    let source = std::fs::read_to_string(IDL_PATH).expect("the normative IDL is readable");
    assert_eq!(undeclared_domains(&source), Vec::<String>::new());
    let in_table: BTreeSet<(&str, &str)> = DECLARED_DOMAINS
        .iter()
        .map(|(owner, field, _)| (*owner, *field))
        .collect();
    let in_sites: BTreeSet<(&str, &str)> = STRING_SITES
        .iter()
        .filter(|(_, _, naming)| *naming == Naming::DeclaredDomain)
        .map(|(owner, field, _)| (*owner, *field))
        .collect();
    assert_eq!(in_table, in_sites);
    assert_eq!(in_table.len(), 5);
    // A declared domain is not a finding, and the two finding classes stay available for
    // the next site that earns one: the table has no rows in them, not no way to say them.
    assert!(!Naming::DeclaredDomain.is_finding());
    assert!(Naming::ResourceWithoutAHandle.is_finding());
    assert!(Naming::UndeclaredDomain.is_finding());
}

/// Negative control for [`every_declared_domain_is_a_rule_that_cites_its_site`]: a rule that
/// disappears, or stops citing its site, is reported.
#[test]
fn a_removed_domain_rule_is_reported() {
    let source = std::fs::read_to_string(IDL_PATH).expect("the normative IDL is readable");
    let renamed = mutate(
        &source,
        "\nrule benchmark.graders {",
        "\nrule benchmark.gone {",
    );
    assert_eq!(
        undeclared_domains(&renamed),
        ["BenchmarkRunRequest.graders: `rule benchmark.graders` is not declared"]
    );
    let uncited = mutate(
        &source,
        "(`QueryExplainInvalidationResponse.edges`)",
        "(the edges)",
    );
    assert_eq!(
        undeclared_domains(&uncited),
        [
            "QueryExplainInvalidationResponse.edges: `rule query.invalidation_edges` does not cite it"
        ]
    );
}

/// The response direction: a resource the daemon mints or moves comes back as a handle.
#[test]
fn the_response_direction_returns_handles_wherever_it_mints_or_moves_a_resource() {
    let document = document();
    let leaves = sweep(&document);
    let names = |operation: &str, direction: &str| -> bool {
        leaves.iter().any(|leaf| {
            leaf.operation == operation
                && leaf.direction == direction
                && matches!(
                    leaf.class,
                    Class::Handle | Class::ArtifactHandle | Class::Commitment | Class::SignerName
                )
        })
    };
    let mut silent = Vec::new();
    for operation in &document.operations {
        if !names(&operation.name, "response") {
            silent.push(operation);
        }
    }
    // Five operations answer without naming a resource, and every one of them is `@readonly`
    // — they return a count, a state dump, or a classification of something the request
    // already named. No `@mutation` is among them: nothing this protocol creates or changes
    // is answered for without its handle.
    let quiet: Vec<&str> = silent
        .iter()
        .map(|operation| operation.name.as_str())
        .collect();
    assert_eq!(
        quiet,
        [
            "proof.slice",
            "correspondence.status",
            "debug.state",
            "debug.enabled",
            "observe.classify",
        ],
        "the set of operations answering without a handle moved"
    );
    for operation in silent {
        assert!(
            !operation.annotations.iter().any(|item| item == "mutation"),
            "{} is a @mutation and returns no handle",
            operation.name
        );
    }
}

/// Content beside a name, never instead of one.
#[test]
fn inline_content_never_travels_without_a_handle_in_the_same_call() {
    let document = document();
    let leaves = sweep(&document);
    let mut orphans = Vec::new();
    for leaf in leaves.iter().filter(|leaf| {
        leaf.class == Class::Inline
            && leaf.direction == "response"
            && leaf.operation != "<envelope>"
    }) {
        // The whole call, both directions: `debug.state` answers with a bare `Opaque`, and
        // the branch it is the state *of* is the request's `DebugHandle`.
        let named = leaves.iter().any(|other| {
            other.operation == leaf.operation
                && matches!(
                    other.class,
                    Class::Handle | Class::ArtifactHandle | Class::Commitment | Class::SignerName
                )
        });
        if !named {
            orphans.push(format!("{} {}", leaf.operation, leaf.path));
        }
    }
    assert!(
        orphans.is_empty(),
        "these responses return content with no handle anywhere in the call:\n  {}",
        orphans.join("\n  ")
    );
}

/// The inbound boundary, stated as a closed list rather than as a caveat.
#[test]
fn inbound_documents_travel_by_value_and_this_is_the_whole_list() {
    let leaves = sweep(&document());
    let mut inbound: Vec<String> = leaves
        .iter()
        .filter(|leaf| {
            leaf.class == Class::Inline
                && leaf.direction == "request"
                && leaf.operation != "<envelope>"
        })
        .map(|leaf| format!("{} {}", leaf.operation, leaf.path))
        .collect();
    inbound.sort();
    assert_eq!(
        inbound,
        [
            "forge.create sketch",
            "intent.accept acceptance",
            "intent.import_bundle content",
            "intent.propose_revision changes.changes",
            "program.run configuration",
            "repair.apply changes",
            "signing.sign_pack pack",
            "signing.verify artifact",
            "signing.verify signature",
            "whiteboard.compile note",
            "workspace.create overlay.content",
            "workspace.fork overlay.content",
        ],
        "the inbound-by-value set moved"
    );
    // `rule snapshot.file_components` is why this is a boundary rather than a defect:
    // "content reaches a daemon out of band in any case, since no operation in this file
    // carries file bytes". Three of the eight nevertheless name documents with a `schemas/`
    // artifact shape and no handle class, so an agent cannot name a note, a sketch, or a
    // change set it already sent.
    for entry in [
        "whiteboard.compile note",
        "forge.create sketch",
        "intent.propose_revision changes.changes",
    ] {
        assert!(inbound.iter().any(|item| item == entry));
    }
}

/// The envelope is where an implicit "current" context would live if there were one.
#[test]
fn the_request_envelope_names_its_scope_and_declares_its_absences() {
    let document = document();
    let envelope = &document.structs["RequestEnvelope"];
    let field = |name: &str| -> &reader::Field {
        envelope
            .iter()
            .find(|item| item.name == name)
            .unwrap_or_else(|| panic!("the envelope declares {name:?}"))
    };
    // Three resource fields, all handles. `nullable` rather than `optional` is the whole
    // point: "Explicit null when the operation takes no snapshot" is a *named* absence, so a
    // request that takes no snapshot says so rather than leaving the daemon to choose one.
    assert_eq!(field("snapshot").ty, "WorkspaceHandle");
    assert_eq!(field("snapshot").presence, "nullable");
    assert_eq!(field("intent").ty, "IntentHandle");
    assert_eq!(field("intent").presence, "nullable");
    assert_eq!(field("capability").ty, "CapabilityHandle");
    assert_eq!(field("capability").presence, "required");

    // And nothing else on the envelope names a resource: no session id, no connection id, no
    // "current" anything.
    let vocabulary = Vocabulary::of(&document);
    let resource: Vec<&str> = envelope
        .iter()
        .filter(|item| {
            matches!(
                vocabulary.classify(&item.ty),
                Some(Class::Handle | Class::ArtifactHandle | Class::Commitment | Class::SignerName)
            )
        })
        .map(|item| item.name.as_str())
        .collect();
    assert_eq!(resource, ["capability", "snapshot", "intent"]);
}

// =========================================================================================
// 5. Negative controls for the sweep
// =========================================================================================

/// A synthetic operation carrying the thing the criterion forbids.
///
/// Appended to the real IDL so it is read by the same reader under the same rules; a control
/// parsed by a lenient copy of the reader would prove nothing about the real one.
const RAW_PATH_OPERATION: &str = "
@readonly
operation workspace.read_file {
  authority read;
  request {
    snapshot_path: String required;
  };
  response {
    contents: Bytes required;
  };
  errors [];
}
";

/// Apply one textual mutation, asserting it actually changed the document.
fn mutate(source: &str, from: &str, to: &str) -> String {
    assert_eq!(
        source.matches(from).count(),
        1,
        "the mutation anchor {from:?} must occur exactly once"
    );
    source.replace(from, to)
}

/// The sites the tables do not adjudicate, for a given IDL text.
fn unadjudicated(source: &str) -> Vec<String> {
    let document = reader::parse(source);
    sweep(&document)
        .into_iter()
        .filter(|leaf| {
            let table = match leaf.class {
                Class::PlainString => STRING_SITES,
                Class::Inline => INLINE_SITES,
                _ => return false,
            };
            adjudication(table, &leaf.declared_by, &leaf.field).is_none()
        })
        .map(|leaf| format!("{} {} {}", leaf.operation, leaf.direction, leaf.path))
        .collect()
}

#[test]
fn the_sweep_flags_a_raw_path_parameter() {
    let source = std::fs::read_to_string(IDL_PATH).expect("the normative IDL is readable");
    assert!(
        unadjudicated(&source).is_empty(),
        "the unmutated file is clean, so the control below measures the mutation"
    );
    let flagged = unadjudicated(&(source + RAW_PATH_OPERATION));
    // Both halves of the synthetic operation are caught: the raw path that names the
    // resource, and the bare blob that returns it without one.
    assert_eq!(
        flagged,
        [
            "workspace.read_file request snapshot_path",
            "workspace.read_file response contents",
        ],
        "a raw-path parameter must be flagged as an unadjudicated resource-naming field"
    );
}

#[test]
fn the_sweep_flags_a_handle_downgraded_to_a_string() {
    let source = std::fs::read_to_string(IDL_PATH).expect("the normative IDL is readable");
    // `workspace.seal`'s request is the smallest handle-only body in the file.
    let mutated = mutate(
        &source,
        "operation workspace.seal {\n  authority propose;\n  request {\n    snapshot: WorkspaceHandle required;",
        "operation workspace.seal {\n  authority propose;\n  request {\n    snapshot: String required;",
    );
    let flagged = unadjudicated(&mutated);
    assert_eq!(
        flagged,
        ["workspace.seal request snapshot"],
        "a handle replaced by a string must be flagged"
    );
    // And the handle census moves with it, so the downgrade is visible twice.
    let leaves = sweep(&reader::parse(&mutated));
    assert_eq!(
        leaves
            .iter()
            .filter(|leaf| leaf.class == Class::Handle)
            .count(),
        HANDLE_LEAVES - 1
    );
}

#[test]
fn the_sweep_flags_an_inline_blob_with_no_handle_in_its_call() {
    let source = std::fs::read_to_string(IDL_PATH).expect("the normative IDL is readable");
    // Strip `observe.classify`'s only handle: the request's `evidence`. The response's bare
    // `classification: Opaque` is then a blob standing where a name should be.
    let mutated = mutate(
        &source,
        "operation observe.classify {\n  authority read;\n  request {\n    evidence: EvidenceHandle required;\n  };",
        "operation observe.classify {\n  authority read;\n  request {\n    subject: String required;\n  };",
    );
    let document = reader::parse(&mutated);
    let leaves = sweep(&document);
    let named = leaves.iter().any(|leaf| {
        leaf.operation == "observe.classify"
            && matches!(
                leaf.class,
                Class::Handle | Class::ArtifactHandle | Class::Commitment | Class::SignerName
            )
    });
    assert!(
        !named,
        "the mutation must remove the only handle from the call"
    );
    let orphan = leaves.iter().any(|leaf| {
        leaf.operation == "observe.classify"
            && leaf.direction == "response"
            && leaf.class == Class::Inline
    });
    assert!(orphan, "the response still returns an inline blob");
}

#[test]
fn the_sweep_flags_a_mutation_that_returns_no_handle() {
    let source = std::fs::read_to_string(IDL_PATH).expect("the normative IDL is readable");
    // `workspace.seal` is a `@mutation`; drop the handle from its response and keep the
    // digest, so the caller is told a snapshot changed without being told which.
    let mutated = mutate(
        &source,
        "  response {\n    snapshot: WorkspaceHandle required;\n    root_digest: Commitment required;\n  };",
        "  response {\n    root_digest_text: String required;\n  };",
    );
    let document = reader::parse(&mutated);
    let leaves = sweep(&document);
    let named = leaves.iter().any(|leaf| {
        leaf.operation == "workspace.seal"
            && leaf.direction == "response"
            && matches!(
                leaf.class,
                Class::Handle | Class::ArtifactHandle | Class::Commitment | Class::SignerName
            )
    });
    let seal = document
        .operations
        .iter()
        .find(|operation| operation.name == "workspace.seal")
        .expect("workspace.seal is declared");
    assert!(seal.annotations.iter().any(|item| item == "mutation"));
    assert!(
        !named,
        "a @mutation answering without a handle must be detectable"
    );
}

#[test]
fn a_removed_operation_breaks_the_registry_cross_check() {
    let source = std::fs::read_to_string(IDL_PATH).expect("the normative IDL is readable");
    let start = source
        .find("operation workspace.seal {")
        .expect("workspace.seal is declared");
    let end = source[start..]
        .find("\n}\n")
        .map(|offset| start + offset + 3)
        .expect("the declaration is terminated");
    let mutated = format!("{}{}", &source[..start], &source[end..]);
    let document = reader::parse(&mutated);
    assert_eq!(document.operations.len(), 82);
    assert_eq!(
        document.operation_keywords, 82,
        "both counting methods see the removal"
    );
    let names: Vec<&str> = document
        .operations
        .iter()
        .map(|operation| operation.name.as_str())
        .collect();
    assert!(
        !names.contains(&"workspace.seal"),
        "the cross-check against plan §10.2 would fail on this list"
    );
}

#[test]
fn the_adjudication_table_is_not_a_wildcard() {
    // A table that answered every query would make `every_string_and_inline_site_is_adjudicated`
    // vacuous. These two sites are real fields of real structs and are deliberately absent.
    assert!(adjudication(STRING_SITES, "Target", "kind").is_none());
    assert!(adjudication(STRING_SITES, "WorkspaceSealRequest", "snapshot").is_none());
    assert!(adjudication(INLINE_SITES, "Diagnostic", "detail").is_none());
    // And every row of both tables is reachable: none names a struct the IDL does not have.
    let document = document();
    for (owner, _, _) in STRING_SITES.iter().chain(INLINE_SITES.iter()) {
        let known = document.structs.contains_key(*owner)
            || document.operations.iter().any(|operation| {
                reader::generated_name(&operation.name, "Request") == *owner
                    || reader::generated_name(&operation.name, "Response") == *owner
            });
        assert!(known, "no declaration site is named {owner:?}");
    }
}

// =========================================================================================
// 6. The live daemon
// =========================================================================================

/// The TV-009 port's model — real bytes for the first workspace.
const DIE_HARD_MODEL: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");

/// The Die Hard Intent Contract, as `continuum-intent`'s own suites use it.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 1)
}

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("a well-formed capability handle")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("a well-formed actor identity")
}

fn name(operation: &str) -> OperationName {
    OperationName::new(operation).expect("a well-formed operation name")
}

fn grant(handle: &str, actor: &str) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(handle),
        actor: who(actor),
        // The top of the ladder, so that a T1 refusal never confounds a decode-level
        // assertion below. Authority is G1-07's criterion, not this one's.
        level: AuthorityLevel::Promote,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: 3,
        profile: Optional::Absent,
        instances: Optional::Absent,
    }
}

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-g1-02-acceptance".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.1 is served")
}

fn budget() -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(64),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

/// A request envelope whose obligations are read off the registry rather than hand-decided.
///
/// `obligation::check_request` demands an idempotency key on a `@mutation` and refuses one on
/// a `@readonly`, so a hand-written envelope that got it wrong would fail for a reason with
/// nothing to do with handles.
fn envelope(operation: &str, actor: &str, capability: &str, request_id: &str) -> RequestEnvelope {
    let spec = registry::operation(operation);
    let mutation = spec.is_some_and(|spec| spec.has(Annotation::Mutation));
    let task_starting = spec.is_some_and(|spec| spec.has(Annotation::TaskStarting));
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request_id).expect("a well-formed request id"),
        idempotency_key: if mutation {
            Optional::Present(format!("idem-{request_id}"))
        } else {
            Optional::Absent
        },
        actor: who(actor),
        capability: cap(capability),
        operation: name(operation),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: if task_starting {
            Optional::Present(budget())
        } else {
            Optional::Absent
        },
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

/// The intent handle a contract's own identity gives it.
fn intent_handle(contract: &IntentContract) -> IntentHandle {
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        continuum_workspace::artifact_path::ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    identity::intent_to_wire(&stored).expect("an `in_` handle")
}

/// A daemon with the six families this build serves, two principals, and an accepted intent.
///
/// Two principals rather than one because the ambient-context probes below are about whether
/// an answer depends on *who* asked and *what they asked last*; one principal cannot see
/// either.
fn deployment() -> (Daemon, IntentHandle) {
    let contract = IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes())
        .expect("the fixture decodes");
    let intent = intent_handle(&contract);
    let root = Some(cap("cap_root"));
    let mut daemon = Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .capability(
            {
                let mut descriptor = grant("cap_root", "service:continuumd");
                descriptor.delegation_depth = 4;
                descriptor
            },
            None,
        )
        .capability(grant("cap_alpha", "agent:alpha"), root.clone())
        .capability(grant("cap_beta", "agent:beta"), root)
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(EvidenceFamily::new())
        .family(ObserveFamily)
        .family(TaskFamily)
        .family(ContextFamily)
        .build();
    daemon.state_mut().put_intent(
        intent.clone(),
        IntentRecord {
            contract,
            status: RegistryStatus::Accepted,
            supersedes: None,
            superseded_by: None,
            // With the block `intent.accept` writes: `workspace.create` binds only an accepted
            // head that carries it (RFC 0037 correction 23).
            acceptance: Some(Acceptance {
                accepted_by: "human:steward".to_owned(),
                signature: "seeded".to_owned(),
                timestamp: "2026-08-01T00:00:00.000Z".to_owned(),
                audit_record: "seeded".to_owned(),
                chain: Vec::new(),
            }),
        },
    );
    (daemon, intent)
}

fn epochs() -> SnapshotEpochs {
    SnapshotEpochs {
        semantic: EpochIdentity::new("semantic-1").expect("epoch"),
        proof: EpochIdentity::new("proof-1").expect("epoch"),
        toolchain: Optional::Absent,
    }
}

/// Stage one file and build the components that name it.
fn components(
    daemon: &mut Daemon,
    intent: &IntentHandle,
    path: &str,
    body: &str,
) -> SnapshotComponents {
    let commitment = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new(path).expect("a workspace path"),
            body.as_bytes().to_vec(),
        )
        .expect("staging names its content");
    SnapshotComponents {
        files: vec![commitment],
        cml_modules: Vec::new(),
        rust_extraction: Vec::new(),
        domain_packs: Vec::new(),
        dependencies: Vec::new(),
        epochs: epochs(),
        intent: intent.clone(),
        correspondence: Vec::new(),
        proof_environment: Vec::new(),
        configuration: Vec::new(),
        file_components: Optional::Absent,
    }
}

fn dispatch(
    daemon: &mut Daemon,
    operation: &str,
    actor: &str,
    capability: &str,
    request_id: &str,
    arguments: Arguments,
) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: envelope(operation, actor, capability, request_id),
        arguments,
    })
}

/// The `ws_` handle a create answered with.
fn created(outcome: &OperationOutcome) -> WorkspaceHandle {
    match &outcome.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("workspace.create answered {other:?}"),
    }
}

// --- the live probes ----------------------------------------------------------------------

/// The liveness half: the fixture really creates two distinct workspaces, so every negative
/// below is a negative about a working surface.
#[test]
fn the_fixture_publishes_two_distinct_snapshots_named_by_handle() {
    let (mut daemon, intent) = deployment();
    let alpha = components(&mut daemon, &intent, "DieHard.ctm", DIE_HARD_MODEL);
    let beta = components(
        &mut daemon,
        &intent,
        "README.md",
        "# a different workspace\n",
    );
    let first = dispatch(
        &mut daemon,
        "workspace.create",
        "agent:alpha",
        "cap_alpha",
        "req_live_a",
        Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: alpha,
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    );
    let second = dispatch(
        &mut daemon,
        "workspace.create",
        "agent:beta",
        "cap_beta",
        "req_live_b",
        Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: beta,
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    );
    assert_eq!(first.envelope.status, ResultStatus::Ok);
    assert_eq!(second.envelope.status, ResultStatus::Ok);
    let (a, b) = (created(&first), created(&second));
    assert_ne!(a, b, "two contents, two handles");
    assert!(a.as_str().starts_with("ws_") && b.as_str().starts_with("ws_"));
    // The response direction, live: every artifact the call published is named by a handle in
    // the envelope's `artifacts` list, with its commitment beside it.
    assert!(
        !first.envelope.artifacts.is_empty(),
        "a create publishes at least one artifact"
    );
    for reference in &first.envelope.artifacts {
        assert!(
            reference.handle.as_str().contains('_'),
            "an ArtifactRef carries a class-prefixed handle: {:?}",
            reference.handle
        );
    }
}

/// **The ambient-context probe.** Two principals, two resources, both interleavings.
///
/// If the daemon held a "current workspace" the second permutation would answer the third and
/// fourth steps differently from the first, because a different create ran last. The
/// comparison is on the typed payloads, which is where a resource identity would move; the
/// envelopes differ by `request_id` and by the audit correlation derived from actor and
/// request id, and holding those equal would have hidden the result rather than shown it.
#[test]
fn an_answer_is_a_function_of_the_handles_in_its_own_request() {
    /// One step of the script: who asks, what they ask, under which request id.
    struct Step {
        label: &'static str,
        actor: &'static str,
        capability: &'static str,
        request_id: &'static str,
        model: &'static str,
        path: &'static str,
    }
    const A: Step = Step {
        label: "alpha",
        actor: "agent:alpha",
        capability: "cap_alpha",
        request_id: "req_perm_alpha",
        model: DIE_HARD_MODEL,
        path: "DieHard.ctm",
    };
    const B: Step = Step {
        label: "beta",
        actor: "agent:beta",
        capability: "cap_beta",
        request_id: "req_perm_beta",
        model: "# beta's workspace\n",
        path: "README.md",
    };

    /// Run one permutation on a freshly built daemon; return each step's create payload and
    /// each step's seal payload, keyed by label.
    fn run(order: [&Step; 2]) -> BTreeMap<&'static str, (Payload, Payload)> {
        let (mut daemon, intent) = deployment();
        let mut created_handles: BTreeMap<&'static str, WorkspaceHandle> = BTreeMap::new();
        let mut creates: BTreeMap<&'static str, Payload> = BTreeMap::new();
        for step in order {
            let parts = components(&mut daemon, &intent, step.path, step.model);
            let outcome = dispatch(
                &mut daemon,
                "workspace.create",
                step.actor,
                step.capability,
                step.request_id,
                Arguments::WorkspaceCreate(WorkspaceCreateRequest {
                    components: parts,
                    overlay: Optional::Absent,
                    seal: Optional::Absent,
                }),
            );
            assert_eq!(outcome.envelope.status, ResultStatus::Ok, "{}", step.label);
            created_handles.insert(step.label, created(&outcome));
            creates.insert(step.label, outcome.payload);
        }
        // The seals run in the *same* order as the creates, so between the two permutations
        // "which resource was touched last" differs at every seal.
        let mut out = BTreeMap::new();
        for step in order {
            let outcome = dispatch(
                &mut daemon,
                "workspace.seal",
                step.actor,
                step.capability,
                &format!("{}_seal", step.request_id),
                Arguments::WorkspaceSeal(WorkspaceSealRequest {
                    snapshot: created_handles[step.label].clone(),
                }),
            );
            assert_eq!(outcome.envelope.status, ResultStatus::Ok, "{}", step.label);
            out.insert(step.label, (creates[step.label].clone(), outcome.payload));
        }
        out
    }

    let forward = run([&A, &B]);
    let reverse = run([&B, &A]);
    assert_eq!(
        forward, reverse,
        "an answer changed when the order of two independent requests changed"
    );
    // And the two resources really are distinct in both runs, so the equality above is not
    // an equality of two copies of one thing.
    assert_ne!(forward["alpha"], forward["beta"]);
}

/// The negative control for the probe above: the same comparison, run against a script that
/// **does** resolve "the resource" the way an ambient protocol would.
///
/// A daemon with a current-workspace pointer answers a request by acting on whatever was
/// touched last. That behaviour cannot be injected into this daemon — it has no such pointer
/// to set — so it is simulated at the only place it is reachable: the script seals *the most
/// recently created* snapshot instead of the one its own step created. The two permutations
/// then disagree, which is what the real test asserts they do not. Without this, "the answers
/// matched" could mean "the comparison cannot tell two answers apart".
#[test]
fn the_order_independence_comparison_detects_an_ambient_resolution() {
    /// Run one permutation, resolving the seal's target ambiently rather than by handle.
    fn run(
        order: [(&'static str, &'static str, &'static str, &'static str); 2],
    ) -> BTreeMap<&'static str, Payload> {
        let (mut daemon, intent) = deployment();
        let mut most_recent: Option<WorkspaceHandle> = None;
        let mut labels: Vec<&'static str> = Vec::new();
        for (label, actor, capability, path) in order {
            let parts = components(&mut daemon, &intent, path, path);
            let outcome = dispatch(
                &mut daemon,
                "workspace.create",
                actor,
                capability,
                &format!("req_ambient_{label}"),
                Arguments::WorkspaceCreate(WorkspaceCreateRequest {
                    components: parts,
                    overlay: Optional::Absent,
                    seal: Optional::Absent,
                }),
            );
            assert_eq!(outcome.envelope.status, ResultStatus::Ok);
            most_recent = Some(created(&outcome));
            labels.push(label);
        }
        // The ambient step: every seal names the last thing created, not its own subject.
        let current = most_recent.expect("two creates ran");
        let mut out = BTreeMap::new();
        for (label, actor, capability, _) in order {
            let outcome = dispatch(
                &mut daemon,
                "workspace.seal",
                actor,
                capability,
                &format!("req_ambient_{label}_seal"),
                Arguments::WorkspaceSeal(WorkspaceSealRequest {
                    snapshot: current.clone(),
                }),
            );
            assert_eq!(outcome.envelope.status, ResultStatus::Ok);
            out.insert(label, outcome.payload);
        }
        out
    }

    let a = ("alpha", "agent:alpha", "cap_alpha", "DieHard.ctm");
    let b = ("beta", "agent:beta", "cap_beta", "README.md");
    let forward = run([a, b]);
    let reverse = run([b, a]);
    assert_ne!(
        forward, reverse,
        "an ambient resolution must make the two permutations disagree; if it does not,          `an_answer_is_a_function_of_the_handles_in_its_own_request` proves nothing"
    );
}

/// A handle is global, not per-principal: the same handle read by the other principal answers
/// the same thing. A daemon with a per-caller namespace would fail here.
#[test]
fn a_handle_names_the_same_resource_whoever_presents_it() {
    let seal_by = |actor: &str, capability: &str| -> Payload {
        let (mut daemon, intent) = deployment();
        let parts = components(&mut daemon, &intent, "DieHard.ctm", DIE_HARD_MODEL);
        let created_by_alpha = dispatch(
            &mut daemon,
            "workspace.create",
            "agent:alpha",
            "cap_alpha",
            "req_shared_create",
            Arguments::WorkspaceCreate(WorkspaceCreateRequest {
                components: parts,
                overlay: Optional::Absent,
                seal: Optional::Absent,
            }),
        );
        let handle = created(&created_by_alpha);
        let outcome = dispatch(
            &mut daemon,
            "workspace.seal",
            actor,
            capability,
            "req_shared_seal",
            Arguments::WorkspaceSeal(WorkspaceSealRequest { snapshot: handle }),
        );
        assert_eq!(outcome.envelope.status, ResultStatus::Ok);
        outcome.payload
    };
    assert_eq!(
        seal_by("agent:alpha", "cap_alpha"),
        seal_by("agent:beta", "cap_beta"),
        "the resource a handle names depends on the caller"
    );
}

/// The envelope's `snapshot` is scope, not a default. Setting it to a *different* existing
/// snapshot does not change which snapshot the request body names.
#[test]
fn the_envelope_scope_is_not_a_fallback_for_a_named_handle() {
    let (mut daemon, intent) = deployment();
    let alpha = components(&mut daemon, &intent, "DieHard.ctm", DIE_HARD_MODEL);
    let beta = components(&mut daemon, &intent, "README.md", "# other\n");
    let a = created(&dispatch(
        &mut daemon,
        "workspace.create",
        "agent:alpha",
        "cap_alpha",
        "req_scope_a",
        Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: alpha,
            overlay: Optional::Absent,
            seal: Optional::Absent,
        }),
    ));
    let b = created(&dispatch(
        &mut daemon,
        "workspace.create",
        "agent:beta",
        "cap_beta",
        "req_scope_b",
        Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: beta,
            overlay: Optional::Absent,
            seal: Optional::Absent,
        }),
    ));
    assert_ne!(a, b);

    let seal_with_scope = |daemon: &mut Daemon, scope: Nullable<WorkspaceHandle>, id: &str| {
        let mut envelope = envelope("workspace.seal", "agent:alpha", "cap_alpha", id);
        envelope.snapshot = scope;
        daemon.dispatch(&OperationRequest {
            envelope,
            arguments: Arguments::WorkspaceSeal(WorkspaceSealRequest {
                snapshot: a.clone(),
            }),
        })
    };
    let unscoped = seal_with_scope(&mut daemon, Nullable::Null, "req_scope_seal_null");
    let scoped_elsewhere =
        seal_with_scope(&mut daemon, Nullable::Value(b.clone()), "req_scope_seal_b");
    assert_eq!(unscoped.envelope.status, ResultStatus::Ok);
    assert_eq!(scoped_elsewhere.envelope.status, ResultStatus::Ok);
    assert_eq!(
        unscoped.payload, scoped_elsewhere.payload,
        "the envelope's snapshot displaced the handle the request body named"
    );
    match &unscoped.payload {
        Payload::WorkspaceSeal(response) => assert_eq!(response.snapshot, a),
        other => panic!("workspace.seal answered {other:?}"),
    }
}

// --- the dangling / foreign-handle probes -------------------------------------------------

/// One probe: an operation, the request field that carries its handle, and three spellings.
struct HandleProbe {
    /// The operation to drive.
    operation: &'static str,
    /// The top-level request field carrying the handle.
    field: &'static str,
    /// A syntactically valid handle of the declared class that no daemon holds.
    dangling: &'static str,
    /// A syntactically valid handle of a *different* declared class.
    foreign: &'static str,
    /// Typed arguments carrying `dangling`, so the frame is otherwise complete and valid.
    arguments: fn() -> Arguments,
}

fn handle(text: &str) -> WorkspaceHandle {
    WorkspaceHandle::new(text).expect("a `ws_` handle")
}

/// Eight handle classes plus the class-agnostic alias, across nine served operations.
///
/// `Commitment` is probed separately — it is a bare `String` alias with no pattern, so
/// "syntactically invalid" is not a state it has.
fn probes() -> Vec<HandleProbe> {
    vec![
        HandleProbe {
            operation: "workspace.seal",
            field: "snapshot",
            dangling: "ws_g102nosuchsnapshotaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            foreign: "task_g102wrongclassaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            arguments: || {
                Arguments::WorkspaceSeal(WorkspaceSealRequest {
                    snapshot: handle("ws_g102nosuchsnapshotaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                })
            },
        },
        HandleProbe {
            operation: "workspace.fork",
            field: "base",
            dangling: "ws_g102nosuchbaseaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            foreign: "ctx_g102wrongclassaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            arguments: || {
                Arguments::WorkspaceFork(WorkspaceForkRequest {
                    base: handle("ws_g102nosuchbaseaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                    overlay: Optional::Absent,
                    patches: Optional::Absent,
                })
            },
        },
        HandleProbe {
            operation: "intent.get",
            field: "intent",
            dangling: "in_g102nosuchintentaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            foreign: "ws_g102wrongclassaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            arguments: || {
                Arguments::IntentGet(IntentGetRequest {
                    intent: IntentHandle::new(
                        "in_g102nosuchintentaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    )
                    .expect("an `in_` handle"),
                })
            },
        },
        HandleProbe {
            operation: "intent.lock",
            field: "intent",
            dangling: "in_g102nosuchlockaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            foreign: "task_g102wrongclassaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            arguments: || {
                Arguments::IntentLock(IntentLockRequest {
                    intent: IntentHandle::new(
                        "in_g102nosuchlockaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    )
                    .expect("an `in_` handle"),
                    policy: BTreeMap::new(),
                })
            },
        },
        HandleProbe {
            operation: "evidence.get",
            field: "evidence",
            dangling: "ev_g102nosuchnodeaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            foreign: "ws_g102wrongclassaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            arguments: || {
                Arguments::EvidenceGet(EvidenceGetRequest {
                    evidence: EvidenceHandle::new(
                        "ev_g102nosuchnodeaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    )
                    .expect("an `ev_` handle"),
                    inline: Optional::Absent,
                })
            },
        },
        HandleProbe {
            operation: "observe.classify",
            field: "evidence",
            dangling: "ev_g102nosuchtraceaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            foreign: "cont_g102wrongclassaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            arguments: || {
                Arguments::ObserveClassify(ObserveClassifyRequest {
                    evidence: EvidenceHandle::new(
                        "ev_g102nosuchtraceaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    )
                    .expect("an `ev_` handle"),
                })
            },
        },
        HandleProbe {
            operation: "task.status",
            field: "task",
            dangling: "task_g102nosuchtaskaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            foreign: "ws_g102wrongclassaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            arguments: || {
                Arguments::TaskStatus(TaskStatusRequest {
                    task: TaskHandle::new("task_g102nosuchtaskaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                        .expect("a `task_` handle"),
                })
            },
        },
        HandleProbe {
            operation: "task.cancel",
            field: "task",
            dangling: "task_g102nosuchcancelaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            foreign: "in_g102wrongclassaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            arguments: || {
                Arguments::TaskCancel(TaskCancelRequest {
                    task: TaskHandle::new("task_g102nosuchcancelaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                        .expect("a `task_` handle"),
                })
            },
        },
        HandleProbe {
            operation: "task.resume",
            field: "continuation",
            dangling: "cont_g102nosuchcontaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            foreign: "task_g102wrongclassaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            arguments: || {
                Arguments::TaskResume(TaskResumeRequest {
                    continuation: ContinuationHandle::new(
                        "cont_g102nosuchcontaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    )
                    .expect("a `cont_` handle"),
                    budget: Optional::Absent,
                })
            },
        },
        HandleProbe {
            operation: "context.expand",
            field: "context",
            dangling: "ctx_g102nosuchpackaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            foreign: "ws_g102wrongclassaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            arguments: || {
                Arguments::ContextExpand(ContextExpandRequest {
                    context: ContextHandle::new(
                        "ctx_g102nosuchpackaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    )
                    .expect("a `ctx_` handle"),
                    anchor: "anchor".to_owned(),
                    relation: ExpansionRelation::CausalPredecessors,
                    depth: Optional::Absent,
                })
            },
        },
        HandleProbe {
            operation: "context.compile",
            field: "evidence_root",
            dangling: "ev_g102nosuchrootaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            // `ArtifactHandle` accepts any class prefix by construction, so the only foreign
            // spelling it *can* refuse is one with no class prefix at all. That is a real
            // narrowing of the criterion for the three operations declared over the alias,
            // and it is asserted rather than argued.
            foreign: "notahandle",
            arguments: || {
                Arguments::ContextCompile(ContextCompileRequest {
                    evidence_root: continuumd::protocol::scalar::ArtifactHandle::new(
                        "ev_g102nosuchrootaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    )
                    .expect("an artifact handle"),
                    question: "does it hold?".to_owned(),
                    audience: Optional::Absent,
                    guarantees: Optional::Absent,
                })
            },
        },
    ]
}

/// Encode a probe's arguments, then rewrite the handle field's spelling in the encoded body.
///
/// The body is built from the typed struct and re-read as a document, so every other field is
/// exactly what a real client would have sent; only the one string under test moves.
fn arguments_with(probe: &HandleProbe, spelling: Option<&str>) -> Opaque {
    let encoded = transport::encode_arguments(&(probe.arguments)()).expect("the body encodes");
    let Json::Object(mut fields) = Json::parse(encoded.as_bytes()).expect("the body is canonical")
    else {
        panic!("a request body is an object")
    };
    assert!(
        fields.contains_key(probe.field),
        "{} declares {}",
        probe.operation,
        probe.field
    );
    match spelling {
        Some(text) => {
            fields.insert(probe.field.to_owned(), Json::String(text.to_owned()));
        }
        None => {
            fields.remove(probe.field);
        }
    }
    Opaque::from_bytes(Json::Object(fields).to_canonical_bytes())
}

/// Drive one frame through the real byte boundary.
fn answer(server: &mut Server, envelope: &RequestEnvelope) -> ResultEnvelope {
    let frame = continuumd::codec::write_in::<Json, _>(envelope).expect("the envelope encodes");
    let bytes = server.answer(&frame).expect("a result is encodable");
    continuumd::codec::read_in::<Json, ResultEnvelope>(&bytes).expect("the result decodes")
}

fn code(result: &ResultEnvelope) -> Option<ErrorCode> {
    result.error.value().map(|error| error.code)
}

/// **The dangling and foreign-handle probe.** Across the byte boundary, over 11 operations
/// and 8 handle classes plus `ArtifactHandle`.
///
/// Three spellings per operation, and the three outcomes are the evidence:
///
/// - a valid handle of the declared class that nothing holds **decodes**, and is then refused
///   *semantically* — never `MalformedRequest`, and never silently answered about something
///   else;
/// - a valid handle of another declared class is refused **at the codec**, by the class's own
///   prefix rule, before the daemon sees a request at all;
/// - the field removed is likewise `MalformedRequest`: there is no ambient value to fall back
///   on, which is the whole of "no implicit current context".
///
/// The pairing is what makes this non-vacuous. If every spelling were refused the same way,
/// nothing would distinguish a typed rejection from a daemon that refuses everything.
#[test]
fn a_dangling_or_foreign_handle_is_refused_by_type_and_never_by_fallback() {
    let (daemon, _) = deployment();
    let mut server = Server::new(daemon, negotiated());
    let probes = probes();
    assert_eq!(probes.len(), 11, "probed operations");
    let classes: BTreeSet<&str> = probes
        .iter()
        .map(|probe| probe.dangling.split_once('_').expect("a prefixed handle").0)
        .collect();
    assert_eq!(
        classes.len(),
        6,
        "distinct declared handle prefixes under probe"
    );

    let mut observed: Vec<(&str, Option<ErrorCode>)> = Vec::new();
    for (index, probe) in probes.iter().enumerate() {
        let mut make = |suffix: &str, arguments: Opaque| -> ResultEnvelope {
            let mut envelope = envelope(
                probe.operation,
                "agent:alpha",
                "cap_alpha",
                &format!("req_probe{index}{suffix}"),
            );
            envelope.arguments = arguments;
            answer(&mut server, &envelope)
        };

        let dangling = make("d", arguments_with(probe, None.or(Some(probe.dangling))));
        assert_ne!(
            code(&dangling),
            Some(ErrorCode::MalformedRequest),
            "{}: a well-formed handle of the declared class must decode",
            probe.operation
        );
        assert_eq!(
            dangling.status,
            ResultStatus::Error,
            "{}: a handle nothing holds must be refused, never answered",
            probe.operation
        );
        // The refusal is one of exactly two declared codes, and which one is itself typed:
        // `CapabilityDenied` where the lane exists and RFC 0027 X2 forbids a distinguishable
        // not-found, `UnsupportedSemanticFeature` where the lane has not shipped
        // (`rule errors.unsupported_surface`). Neither is prose, and neither is a guess.
        assert!(
            matches!(
                code(&dangling),
                Some(ErrorCode::CapabilityDenied | ErrorCode::UnsupportedSemanticFeature)
            ),
            "{}: a dangling handle answered {:?}",
            probe.operation,
            code(&dangling)
        );
        observed.push((probe.operation, code(&dangling)));

        let foreign = make("f", arguments_with(probe, Some(probe.foreign)));
        assert_eq!(
            code(&foreign),
            Some(ErrorCode::MalformedRequest),
            "{}: a handle of the wrong class must be refused at the codec, not interpreted",
            probe.operation
        );

        let absent = make("n", arguments_with(probe, None));
        assert_eq!(
            code(&absent),
            Some(ErrorCode::MalformedRequest),
            "{}: an absent handle must be refused, never defaulted",
            probe.operation
        );
    }

    // Pinned as a set, so that a lane landing (or a code changing) is visible here rather
    // than absorbed by the `matches!` above.
    let unsupported: Vec<&str> = observed
        .iter()
        .filter(|(_, code)| *code == Some(ErrorCode::UnsupportedSemanticFeature))
        .map(|(operation, _)| *operation)
        .collect();
    assert_eq!(
        unsupported,
        ["observe.classify", "context.compile"],
        "the two lanes that answer `UnsupportedSemanticFeature` rather than denying moved"
    );
}

/// The typed API's own half of the same property: a wrong-class handle is not a runtime
/// check, it is a value that cannot be built.
///
/// `protocol_handle!` makes the constructor the parser, so the string the codec refuses above
/// is the string this refuses here — one prefix rule, not two.
#[test]
fn a_handle_of_the_wrong_class_cannot_be_constructed_at_all() {
    let task = "task_g102aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let workspace = "ws_g102aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    assert!(TaskHandle::new(task).is_ok());
    assert!(WorkspaceHandle::new(workspace).is_ok());
    assert!(
        WorkspaceHandle::new(task).is_err(),
        "a `task_` string is not a WorkspaceHandle"
    );
    assert!(TaskHandle::new(workspace).is_err());
    assert!(IntentHandle::new(task).is_err());
    assert!(ContinuationHandle::new(task).is_err());
    assert!(ContextHandle::new(workspace).is_err());
    assert!(EvidenceHandle::new(workspace).is_err());
    // And an empty opaque part is not a handle either, so the prefix alone cannot be
    // presented as a name.
    assert!(WorkspaceHandle::new("ws_").is_err());
    // The class-agnostic alias is the narrower guarantee, and this is its exact extent: it
    // refuses a string with no class prefix, and accepts every declared class.
    for class in ["ws_", "task_", "ev_", "ctx_", "cont_", "in_"] {
        assert!(
            continuumd::protocol::scalar::ArtifactHandle::new(&format!("{class}g102aaaa")).is_ok(),
            "{class} is a class prefix"
        );
    }
    assert!(continuumd::protocol::scalar::ArtifactHandle::new("notahandle").is_err());
}

/// The live surface, counted. The IDL sweep is 83/83; this is what the probes can reach.
///
/// The name records the count this campaign pinned at protocol 3.6 (30 of 75). Protocol 3.8
/// (bn-3glnv) grew the registry and the landed set together by eight, to 38 of 83, and the
/// name is kept because governance evidence cites it.
#[test]
fn the_live_surface_is_thirty_of_the_seventy_five_and_this_is_which() {
    let document = document();
    assert_eq!(document.operations.len(), 83);
    let served: BTreeSet<&str> = [
        "workspace.create",
        "workspace.create_by_reference",
        "workspace.fork",
        "workspace.diff",
        "workspace.seal",
        "intent.get",
        "intent.diff",
        "intent.propose_revision",
        "intent.accept",
        "intent.reject",
        "intent.lock",
        "evidence.get",
        "evidence.query",
        "evidence.verify",
        "evidence.subscribe",
        "evidence.link",
        "observe.ingest",
        "observe.classify",
        "observe.result",
        "verification.start",
        "verification.result",
        "verification.await",
        "task.status",
        "task.cancel",
        "task.resume",
        "task.subscribe",
        "task.update_budget",
        "context.compile",
        "context.expand",
        "whiteboard.compile",
        "intent.export_bundle",
        "intent.import_bundle",
        "signing.mint",
        "signing.rotate",
        "signing.revoke",
        "signing.registry",
        "signing.verify",
        "signing.sign_pack",
    ]
    .into_iter()
    .collect();
    assert_eq!(served.len(), 38, "operations with a decoded request body");
    for operation in &served {
        assert!(
            document
                .operations
                .iter()
                .any(|declared| declared.name == *operation),
            "{operation} is not a declared operation"
        );
    }
    // The other 45 are refused by the codec on the operation name, before a payload is read.
    // That is an accounting of absence, not a defence: nothing here claims a live handle
    // property for them.
    let (daemon, _) = deployment();
    let mut server = Server::new(daemon, negotiated());
    for operation in ["proof.goal", "forge.create", "benchmark.run", "debug.open"] {
        assert!(!served.contains(operation));
        let mut envelope = envelope(operation, "agent:alpha", "cap_alpha", "req_unserved");
        envelope.arguments = Opaque::from_bytes(Json::Object(BTreeMap::new()).to_canonical_bytes());
        let result = answer(&mut server, &envelope);
        assert_eq!(result.status, ResultStatus::Error, "{operation}");
    }
}

/// A typed absence, pinned so it cannot be silently paid or silently forgotten.
///
/// `ResultEnvelope.next_operations` is "Allowed operations from this state — the safe recovery
/// and discovery surface (plan §0.2)", and it is the one place the protocol would hand an
/// agent a *typed* next step with its arguments already filled in. `daemon::result` builds it
/// as `Vec::new()` at both construction sites, so the surface is declared and empty. This
/// test goes red the moment a daemon populates it, which is when the property it would carry
/// becomes checkable.
#[test]
fn the_typed_discovery_surface_is_declared_and_unpopulated() {
    let (mut daemon, intent) = deployment();
    let parts = components(&mut daemon, &intent, "DieHard.ctm", DIE_HARD_MODEL);
    let outcome = dispatch(
        &mut daemon,
        "workspace.create",
        "agent:alpha",
        "cap_alpha",
        "req_discovery",
        Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: parts,
            overlay: Optional::Absent,
            seal: Optional::Absent,
        }),
    );
    assert_eq!(outcome.envelope.status, ResultStatus::Ok);
    assert!(
        outcome.envelope.next_operations.is_empty(),
        "next_operations is populated; the discovery surface is now checkable and this \
         acceptance file must grow a probe for it"
    );
    // The field the IDL declares beside it is populated, and that is the contrast: an
    // omission manifest is present and empty rather than absent, so "nothing was omitted" is
    // a statement rather than a silence (INV-007).
    assert!(outcome.envelope.omissions.is_empty());
}

/// The state a daemon holds is keyed by handle, end to end.
///
/// A "current workspace" would have to live somewhere. `DaemonState::new()` is the empty
/// state, and after a create the only thing that changed is reachable by the handle the
/// response named — so a second daemon built from nothing and given the same request answers
/// the same way, and one given no request answers nothing.
#[test]
fn nothing_is_reachable_except_through_the_handle_the_daemon_named() {
    let (mut daemon, intent) = deployment();
    let parts = components(&mut daemon, &intent, "DieHard.ctm", DIE_HARD_MODEL);
    let handle = created(&dispatch(
        &mut daemon,
        "workspace.create",
        "agent:alpha",
        "cap_alpha",
        "req_reach_create",
        Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: parts,
            overlay: Optional::Absent,
            seal: Optional::Absent,
        }),
    ));
    let sealed = dispatch(
        &mut daemon,
        "workspace.seal",
        "agent:alpha",
        "cap_alpha",
        "req_reach_seal",
        Arguments::WorkspaceSeal(WorkspaceSealRequest {
            snapshot: handle.clone(),
        }),
    );
    assert_eq!(sealed.envelope.status, ResultStatus::Ok);

    // The mutant: a daemon that never received the create. The same handle, the same
    // capability, the same actor — and no answer, because there is nothing ambient for the
    // request to have meant.
    let (mut fresh, _) = deployment();
    let denied = dispatch(
        &mut fresh,
        "workspace.seal",
        "agent:alpha",
        "cap_alpha",
        "req_reach_seal",
        Arguments::WorkspaceSeal(WorkspaceSealRequest { snapshot: handle }),
    );
    assert_eq!(denied.envelope.status, ResultStatus::Error);
    assert_ne!(
        denied.error_code(),
        Some(ErrorCode::MalformedRequest),
        "the handle is well formed; what is missing is the resource"
    );
    // `DaemonState::new()` is the whole of what a daemon starts with, and it holds nothing
    // a request could have meant: no workspace, no intent, no evidence, no task.
    let empty = DaemonState::new();
    assert!(
        empty
            .workspace(&WorkspaceHandle::new("ws_g102emptyaaaa").expect("a handle"))
            .is_none()
    );
}
