//! `continuum evidence show` — query the PR-7 evidence graph by an explicit handle.
//!
//! One wire call, `evidence.get`, which `continuumd::daemon::evidence::EvidenceFamily`
//! serves for real: this is the one command in bn-1g7e4's group whose backing service has
//! landed, so its success path is driven end to end against a live daemon in
//! `tests/evidence_show.rs` rather than proven renderer-first.
//!
//! # Three things this command must not flatten
//!
//! 1. **A node and an edge are different answers.** `ev_` is the handle class of *both*
//!    halves of the graph, and `evidence.get` answers with `node` set or `edge` set — the
//!    other reading `null`. Both are rendered, each present or explicitly `none`, so a
//!    caller never has to infer which kind of thing it asked about.
//! 2. **Withheld is not absent.** RFC 0026: "a client MUST be able to tell 'withheld' from
//!    'absent' structurally, without inference." The typed [`Redacted`] stub is rendered
//!    field by field, and the daemon's own `omissions` entry with reason `redaction`
//!    travels beside it — the two are different obligations and neither substitutes for the
//!    other.
//! 3. **A denial is not a not-found.** RFC 0027 X2: a read of an evidence node the caller is
//!    not authorized for returns `CapabilityDenied` *whether or not it exists*, because a
//!    distinct not-found is an existence oracle. This command renders that refusal exactly
//!    as the daemon typed it and adds no "no such node" gloss of its own — there is no
//!    branch here that could, because the only thing it knows is the code.
//!
//! # `--inline`, and why asking for it is not an error
//!
//! `evidence.get`'s `inline` requests bounded inline content, and this daemon has no
//! bounded-content channel in the response body. Asking for it is still *admitted*: the
//! answer carries the record plus a typed INV-007 omission naming
//! `evidence.inline_content` with reason `unsupported`. That omission is the daemon's, and
//! this command prints it unabridged like every other — which is the whole INV-007 shape in
//! one round trip, and the reason `tests/evidence_show.rs` drives it.

use continuumd::codec::json::Json;
use continuumd::daemon::family::Payload;
use continuumd::protocol::envelope::Redacted;
use continuumd::protocol::operations::evidence::{EvidenceGetRequest, EvidenceGetResponse};
use continuumd::protocol::scalar::EvidenceHandle;
use continuumd::protocol::spec::{Optional, ProtocolEnum};

use crate::error::CliError;
use crate::format::Format;
use crate::render::{self, Projection, Rendered};
use crate::wire::{Connection, Outcome, Transport};

/// The wire operation `evidence show` drives, in the registry's own spelling.
pub const OPERATION: &str = "evidence.get";

/// The parsed, typed arguments of one `evidence show` invocation.
///
/// Two fields and no third: there is no remembered "current" node, no last-queried handle,
/// and no session cursor anywhere in this crate (INV-002). The handle is on the command
/// line every time.
#[derive(Debug, Clone)]
pub struct ShowArgs {
    /// The evidence-graph node or edge to read.
    pub evidence: EvidenceHandle,
    /// Whether to ask for bounded inline content (see this module's doc).
    pub inline: bool,
}

impl ShowArgs {
    fn request(&self) -> EvidenceGetRequest {
        EvidenceGetRequest {
            evidence: self.evidence.clone(),
            // `Absent` rather than `Present(false)` when the flag is not given: the IDL
            // declares the field `optional`, and a caller that did not ask for inline
            // content has not asked for it to be *suppressed* either.
            inline: if self.inline {
                Optional::Present(true)
            } else {
                Optional::Absent
            },
        }
    }
}

/// Run `evidence show`: read the node or edge the handle names, render it faithfully.
///
/// # Errors
///
/// [`CliError::Connection`] when the wire call fails. A daemon's typed refusal is never an
/// error here — it is a rendered answer with a non-zero exit code.
pub fn show(
    connection: &mut Connection,
    transport: &mut dyn Transport,
    args: &ShowArgs,
    format: Format,
) -> Result<Rendered, CliError> {
    let outcome = connection.evidence_get(transport, &args.request())?;
    Ok(render(args, &outcome, format))
}

/// Project one `evidence.get` answer, in whichever [`Format`] was resolved.
///
/// Total over both arms, and the INV-007 manifest travels through both: there is no branch
/// here, and no flag in [`crate::cli`]'s parser, that removes it.
#[must_use]
pub fn render(args: &ShowArgs, outcome: &Outcome<Payload>, format: Format) -> Rendered {
    let request = vec![
        ("evidence".to_owned(), args.evidence.as_str().to_owned()),
        ("inline".to_owned(), args.inline.to_string()),
    ];
    let request_json = vec![
        (
            "evidence".to_owned(),
            Json::String(args.evidence.as_str().to_owned()),
        ),
        ("inline".to_owned(), Json::Bool(args.inline)),
    ];
    let success = |payload: &Payload| {
        let body = response(payload);
        let node = body.and_then(|body| body.node.value());
        let edge = body.and_then(|body| body.edge.value());
        let stub = body.and_then(|body| body.redacted.value());
        let mut lines = vec![
            ("node.bytes".to_owned(), render::embedded_bytes_line(node)),
            ("edge.bytes".to_owned(), render::embedded_bytes_line(edge)),
        ];
        lines.extend(redaction_lines(stub));
        let fields = vec![
            ("node".to_owned(), render::embedded(node)),
            ("node_bytes".to_owned(), render::embedded_bytes(node)),
            ("edge".to_owned(), render::embedded(edge)),
            ("edge_bytes".to_owned(), render::embedded_bytes(edge)),
            ("redacted".to_owned(), redaction_json(stub)),
        ];
        (lines, fields)
    };
    render::project(
        &Projection {
            command: "evidence show",
            operation: OPERATION,
            request,
            request_json,
            absent_json: vec![
                ("node".to_owned(), Json::Null),
                ("node_bytes".to_owned(), Json::Null),
                ("edge".to_owned(), Json::Null),
                ("edge_bytes".to_owned(), Json::Null),
                ("redacted".to_owned(), Json::Null),
            ],
        },
        outcome,
        success,
        format,
    )
}

/// The `evidence.get` response body, when the answer carried one.
fn response(payload: &Payload) -> Option<&EvidenceGetResponse> {
    match payload {
        Payload::EvidenceGet(response) => Some(response),
        _ => None,
    }
}

/// The typed [`Redacted`] stub, field by field — or four explicit `none`s.
///
/// Four lines either way, on purpose: "withheld" and "absent" have to be distinguishable
/// *structurally*, and a rendering that printed nothing when nothing was withheld would
/// make the absence of the stub indistinguishable from a renderer that forgot it.
fn redaction_lines(redacted: Option<&Redacted>) -> render::Lines {
    vec![
        (
            "redacted".to_owned(),
            redacted.map_or_else(|| "false".to_owned(), |stub| stub.redacted.to_string()),
        ),
        (
            "redacted.reason".to_owned(),
            render::wire_or_none(redacted.map(|stub| stub.reason)),
        ),
        (
            "redacted.commitment".to_owned(),
            render::string_or_none(redacted.map(|stub| stub.commitment.as_str())),
        ),
        (
            "redacted.original_class".to_owned(),
            render::string_or_none(redacted.map(|stub| stub.original_class.as_str())),
        ),
    ]
}

fn redaction_json(redacted: Option<&Redacted>) -> Json {
    match redacted {
        None => Json::Null,
        Some(stub) => Json::object([
            ("redacted".to_owned(), Json::Bool(stub.redacted)),
            (
                "reason".to_owned(),
                Json::String(stub.reason.as_wire().to_owned()),
            ),
            (
                "commitment".to_owned(),
                Json::String(stub.commitment.as_str().to_owned()),
            ),
            (
                "original_class".to_owned(),
                Json::String(stub.original_class.clone()),
            ),
        ])
        .expect("four distinct literal keys never collide"),
    }
}
