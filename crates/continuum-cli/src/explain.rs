//! `continuum explain compile` — compile a Context Pack and render its projection.
//!
//! # The registry mapping, stated
//!
//! | Command | Wire operation |
//! |---|---|
//! | `explain compile` | `context.compile` |
//!
//! One verb, and it names the registry verb it drives. The `context` namespace has exactly
//! two operations and the other one, `context.expand`, is already
//! [`continuum context expand`](crate::context) — the PR-11 navigation surface. Giving
//! `explain` a second verb that sent the same frame would be two commands with one meaning,
//! which is the "second code path" acceptance criterion 1 rules out. What `explain` adds over
//! `context expand` is not another call: it is the *projection*. `context expand` reports the
//! child pack's length and embeds it; this command reads the pack's own schema-normative
//! fields and renders them.
//!
//! # Prose is a projection of the JSON, never a separate truth
//!
//! > No prose-only machine interfaces.
//! >
//! > — INV-003; plan §3.1
//!
//! The text and pretty formats here print no sentence this module composed. Every line is
//! `key  value` where the key names a field of `schemas/context-pack.schema.json` and the
//! value is that field read out of the pack document the daemon sent — the same document
//! `--format json` embeds verbatim under `pack`. A key the document does not carry reads
//! `none`; a value of an unexpected JSON shape reads `none` too, because guessing what a
//! malformed field meant is how a projection becomes an author.
//!
//! In particular the pack's *own* `omissions` manifest (RFC 0028: the candidate set equals
//! the selection plus the sum of the manifest counts) is rendered under `pack.omissions`,
//! and the *result envelope's* INV-007 manifest is rendered under `omissions` by
//! [`crate::render::project`] as it is for every other command in this crate. They are two
//! different manifests about two different things and neither is folded into the other.
//!
//! # Depth today: registered, and not served
//!
//! `continuumd::daemon::context::ContextFamily` serves the `context` namespace and answers
//! `context.compile` with the typed `UnsupportedSemanticFeature` its `errors` clause
//! declares:
//!
//! > no Context Pack compiler is served by this daemon; a pack is compiled from the evidence
//! > graph, the property automaton and the correspondence graph, and none of those is wired
//! > here
//!
//! That is a stronger fact than the one [`crate::debug`] reports: the family exists, is
//! registered, and serves this namespace's other operation, so the refusal is the *family's
//! own* statement about a compiler rather than the dispatcher's statement about a missing
//! namespace. Either way this command reports it as a typed [`crate::render::Depth`] token
//! beside the operation name, derived from the daemon's code and asserted nowhere. The
//! success projection below is unit-tested against the real wire types for the reason
//! bn-3tz60 gave for `context.expand` and bn-1g7e4 for `debug`/`repair`: a renderer that only
//! handled the refusal it happens to see today would be untested on the day the compiler
//! lands.

use continuumd::codec::json::Json;
use continuumd::daemon::family::Payload;
use continuumd::protocol::envelope::Budget;
use continuumd::protocol::operations::context::{ContextCompileRequest, ContextCompileResponse};
use continuumd::protocol::scalar::{ArtifactHandle, ByteCount};
use continuumd::protocol::spec::{Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::Audience;

use crate::error::CliError;
use crate::format::Format;
use crate::render::{self, Lines, Projection, Rendered};
use crate::wire::{Admitted, Connection, Outcome, Transport};

/// The wire operation `explain compile` drives, in the registry's own spelling.
pub const COMPILE_OPERATION: &str = "context.compile";

/// The parsed, typed arguments of one `explain compile` invocation.
#[derive(Debug, Clone)]
pub struct CompileArgs {
    /// The evidence root to compile from. Explicit on every invocation (INV-002).
    pub evidence_root: ArtifactHandle,
    /// The question the pack answers, verbatim (IDL `question: String required`).
    pub question: String,
    /// The intended reader. Advisory only — RFC 0027 Safety: it "never changes a verdict,
    /// never widens authority, and never suppresses an omission or an uncertainty" — and
    /// `Absent` leaves the daemon's own default in force.
    pub audience: Optional<Audience>,
    /// Guarantees the caller requires of the pack; the pack declares which it achieved.
    pub guarantees: Vec<String>,
    /// The `bytes` ceiling — "the enforced context contract (RFC 0027)". Required on the
    /// command line: `context.compile` is `@task_starting`, so a budget travels, and a
    /// ceiling this crate invented would be deciding how much context is enough.
    pub bytes: u64,
}

impl CompileArgs {
    fn request(&self) -> ContextCompileRequest {
        ContextCompileRequest {
            evidence_root: self.evidence_root.clone(),
            question: self.question.clone(),
            audience: self.audience,
            guarantees: if self.guarantees.is_empty() {
                Optional::Absent
            } else {
                Optional::Present(self.guarantees.clone())
            },
        }
    }

    fn budget(&self) -> Budget {
        Budget {
            wall_ms: Optional::Absent,
            cpu_ms: Optional::Absent,
            memory_bytes: Optional::Absent,
            states: Optional::Absent,
            solver_ms: Optional::Absent,
            proof_ms: Optional::Absent,
            tokens: Optional::Absent,
            candidates: Optional::Absent,
            bytes: Optional::Present(ByteCount::new(self.bytes)),
        }
    }
}

/// Run `explain compile`: ask for a pack, render its projection.
///
/// # Errors
///
/// [`CliError::Connection`] when the wire call fails. A daemon's typed refusal — including
/// the unsupported-compiler refusal this deployment answers today — is a rendered answer,
/// not an error.
pub fn compile(
    connection: &mut Connection,
    transport: &mut dyn Transport,
    args: &CompileArgs,
    format: Format,
) -> Result<Rendered, CliError> {
    let outcome = connection.context_compile(transport, &args.request(), args.budget())?;
    Ok(render_compile(args, &outcome, format))
}

/// Project one `context.compile` answer, in whichever [`Format`] was resolved.
#[must_use]
pub fn render_compile(args: &CompileArgs, outcome: &Outcome<Payload>, format: Format) -> Rendered {
    let mut request = vec![
        (
            "evidence_root".to_owned(),
            args.evidence_root.as_str().to_owned(),
        ),
        ("question".to_owned(), args.question.clone()),
        (
            "audience".to_owned(),
            render::wire_or_none(args.audience.value().copied()),
        ),
        ("budget_bytes".to_owned(), args.bytes.to_string()),
    ];
    request.extend(render::handle_lines(
        "guarantees",
        args.guarantees.iter().map(String::as_str),
    ));
    let request_json = vec![
        (
            "evidence_root".to_owned(),
            Json::String(args.evidence_root.as_str().to_owned()),
        ),
        ("question".to_owned(), Json::String(args.question.clone())),
        (
            "audience".to_owned(),
            args.audience.value().map_or(Json::Null, |audience| {
                Json::String(audience.as_wire().to_owned())
            }),
        ),
        ("budget_bytes".to_owned(), Json::Integer(args.bytes)),
        (
            "guarantees".to_owned(),
            render::handle_json(args.guarantees.iter().map(String::as_str)),
        ),
    ];
    let success = |admitted: &Admitted<Payload>| {
        let body = compile_response(&admitted.payload);
        let pack = body.map(|body| &body.pack);
        let document = pack.and_then(|pack| Json::parse(pack.as_bytes()).ok());
        let mut lines = vec![
            (
                "context".to_owned(),
                render::string_or_none(body.map(|body| body.context.as_str())),
            ),
            ("pack_bytes".to_owned(), render::embedded_bytes_line(pack)),
        ];
        lines.extend(pack_lines(document.as_ref()));
        let fields = vec![
            (
                "context".to_owned(),
                body.map_or(Json::Null, |body| {
                    Json::String(body.context.as_str().to_owned())
                }),
            ),
            ("pack".to_owned(), render::embedded(pack)),
            ("pack_bytes".to_owned(), render::embedded_bytes(pack)),
        ];
        (lines, fields)
    };
    render::project(
        &Projection {
            command: "explain compile",
            operation: COMPILE_OPERATION,
            request,
            request_json,
            absent_json: vec![
                ("context".to_owned(), Json::Null),
                ("pack".to_owned(), Json::Null),
                ("pack_bytes".to_owned(), Json::Null),
            ],
        },
        outcome,
        success,
        format,
    )
}

fn compile_response(payload: &Payload) -> Option<&ContextCompileResponse> {
    match payload {
        Payload::ContextCompile(response) => Some(response),
        _ => None,
    }
}

// --- the Context Pack projection ------------------------------------------------------------

/// The pack's scalar fields, in the order this projection prints them.
///
/// A table rather than a sequence of `push` calls so the key set is one list a reader can
/// check against `schemas/context-pack.schema.json`, and so the "absent pack" arm below
/// cannot drift from the present one: both walk this array.
///
/// Each entry is `(rendered key suffix, path into the document)`. A one- or two-element path,
/// because the schema nests exactly that far for the fields worth a line of their own.
const PACK_SCALARS: [(&str, &[&str]); 18] = [
    ("schema_id", &["schema_id"]),
    ("schema_epoch", &["schema_epoch"]),
    ("context_id", &["context_id"]),
    ("parent", &["parent"]),
    ("snapshot", &["snapshot"]),
    ("intent", &["intent"]),
    ("question", &["question"]),
    ("verdict", &["verdict"]),
    ("inconclusive_reason", &["inconclusive_reason"]),
    ("assurance.class", &["assurance", "class"]),
    ("content_budget.bytes", &["content_budget", "bytes"]),
    ("content_budget.nodes", &["content_budget", "nodes"]),
    ("content_budget.tokens", &["content_budget", "tokens"]),
    // Beside `tokens` and never apart from it: RFC 0028 F4 requires the tokenizer identity
    // wherever a token count is reported, so a projection that printed one without the other
    // would make an advisory number look comparable across models.
    (
        "content_budget.tokenizer_id",
        &["content_budget", "tokenizer_id"],
    ),
    ("content_hash", &["content_hash"]),
    ("semantic_epoch", &["semantic_epoch"]),
    ("replay", &["replay"]),
    ("debugger_branch", &["debugger_branch"]),
];

/// The Context Pack, projected as `key  value` lines under `pack.`.
///
/// Total: an absent or unparseable document renders the same key set reading `none`, so the
/// text output of a refusal and of a success differ in their values and never in their shape.
#[must_use]
pub fn pack_lines(pack: Option<&Json>) -> Lines {
    let mut lines: Lines = PACK_SCALARS
        .iter()
        .map(|(key, path)| {
            (
                format!("pack.{key}"),
                pack.and_then(|pack| scalar(pack, path))
                    .unwrap_or_else(|| "none".to_owned()),
            )
        })
        .collect();

    lines.extend(items("pack.guarantees", pack, &["guarantees"], &[]));
    lines.extend(items("pack.evidence", pack, &["evidence"], &[]));
    lines.extend(items(
        "pack.selected",
        pack,
        &["selected"],
        &[
            ("id", &["id"]),
            ("kind", &["kind"]),
            ("artifact", &["artifact"]),
        ],
    ));
    lines.extend(items(
        "pack.omissions",
        pack,
        &["omissions"],
        &[
            ("kind", &["kind"]),
            ("reason", &["reason"]),
            ("count", &["count"]),
            ("expandable", &["expandable"]),
            ("expansion.relation", &["expansion", "relation"]),
            ("expansion.anchor", &["expansion", "anchor"]),
        ],
    ));
    lines.extend(items(
        "pack.expansions",
        pack,
        &["expansions"],
        &[("relation", &["relation"]), ("anchor", &["anchor"])],
    ));
    lines.extend(items(
        "pack.redactions",
        pack,
        &["redactions"],
        &[
            ("reason", &["reason"]),
            ("commitment", &["commitment"]),
            ("original_class", &["original_class"]),
        ],
    ));
    lines
}

/// One array field: its length, then each element — either the element itself when `fields`
/// is empty (a list of strings), or the named sub-fields of each element object.
///
/// `none` for the count when the document is absent or carries no such key, which is
/// distinguishable from `0` — an empty manifest is an *assertion* under RFC 0028 ("the
/// selection is the whole candidate set"), and a projection that printed `0` for a missing
/// field would be making that assertion on the pack's behalf.
///
/// A key that is present but is *not* an array falls through to [`render_scalar`] rather
/// than reading `none`: a pack whose `omissions` were an object is malformed, and the
/// honest reading of a malformed field is what is there, not a claim that nothing is
/// (bn-ybh1z — and it is what keeps this projection re-renderable from the embedded document
/// under [`crate::contract`]'s one rule, on adversarial input as much as on a conforming
/// pack).
fn items(prefix: &str, pack: Option<&Json>, path: &[&str], fields: &[(&str, &[&str])]) -> Lines {
    let value = pack.and_then(|pack| walk(pack, path));
    let Some(Json::Array(array)) = value else {
        return vec![(
            prefix.to_owned(),
            value
                .and_then(render_scalar)
                .unwrap_or_else(|| "none".to_owned()),
        )];
    };
    let mut lines = vec![(prefix.to_owned(), array.len().to_string())];
    for (index, item) in array.iter().enumerate() {
        if fields.is_empty() {
            lines.push((
                format!("{prefix}[{index}]"),
                render_scalar(item).unwrap_or_else(|| "none".to_owned()),
            ));
            continue;
        }
        for (key, field_path) in fields {
            lines.push((
                format!("{prefix}[{index}].{key}"),
                scalar(item, field_path).unwrap_or_else(|| "none".to_owned()),
            ));
        }
    }
    lines
}

/// Follow `path` through nested objects.
fn walk<'a>(document: &'a Json, path: &[&str]) -> Option<&'a Json> {
    let mut cursor = document;
    for key in path {
        let Json::Object(fields) = cursor else {
            return None;
        };
        cursor = fields.get(*key)?;
    }
    Some(cursor)
}

/// The scalar at `path`, rendered — or `None` when it is absent or null.
fn scalar(document: &Json, path: &[&str]) -> Option<String> {
    render_scalar(walk(document, path)?)
}

/// One JSON value as a line value — [`crate::contract::render_value`], modulo `null`, which
/// this function reports as [`None`] so a caller can tell "the pack said null" from "the
/// path led nowhere" before both collapse to `none`.
///
/// A container answers its length rather than a `{…}` notation the document does not have,
/// which is the contract's one rule and not a per-command choice: a pack whose `parent` were
/// an object would otherwise read `none` here and `1` in the machine channel, and the two
/// channels would disagree about a field neither of them invented.
fn render_scalar(value: &Json) -> Option<String> {
    match value {
        Json::Null => None,
        other => Some(crate::contract::render_value(other)),
    }
}
