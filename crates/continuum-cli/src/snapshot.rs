//! `continuum snapshot create|fork|seal` — the PR-3 workspace family, over explicit handles.
//!
//! # The registry mapping, stated
//!
//! | Command | Wire operation |
//! |---|---|
//! | `snapshot create` | `workspace.create` |
//! | `snapshot fork` | `workspace.fork` |
//! | `snapshot seal` | `workspace.seal` |
//!
//! Three verbs, because the registry's `workspace` namespace has exactly these three
//! operations that *name a snapshot*. The fourth, `workspace.diff`, is the RFC 0031
//! semantic-diff read — a different question, answered by a different lane (PR 12), and
//! `continuumd::daemon::workspace` refuses it today with the typed
//! `UnsupportedSemanticFeature` its `errors` clause declares. It is deliberately not a verb
//! of this group: a `snapshot diff` here would be a diff *command* whose depth is
//! `unsupported` in every deployment this workspace can build, and the honest place for the
//! diff surface is beside the rest of PR 12's lane rather than inside the family that makes
//! snapshots.
//!
//! # `continuum` never means "whatever is on disk"
//!
//! > A tool call never means "whatever is on disk"; it means a named snapshot.
//! >
//! > — the IDL, on `workspace.create`
//!
//! That sentence is why this command takes `--components`: a document of *content
//! identities*, which reached the daemon out of band (staging is administration, IDL §7 —
//! "no operation in this file carries file bytes"). This command does not walk a working
//! tree, does not hash anything, and holds no notion of a current directory. What it does is
//! carry the caller's declaration to the daemon and render the answer.
//!
//! The one exception the protocol itself makes is [`FileOverlay`], whose `content` is
//! literal bytes — editor buffers overlaid on the named components — so `--overlay
//! <path>=<bytes>` is the only channel in this group through which content rather than an
//! identity travels, exactly as the IDL declares it.
//!
//! # Sealing is a separate verb because it is a separate decision
//!
//! `workspace.create` has a `seal: Bool optional` field *and* `workspace.seal` exists. Both
//! are surfaced: `--seal` on `create` for the caller that knows at creation time, and
//! `snapshot seal <ws_*>` for the caller that decides later. Neither is defaulted — an
//! absent `--seal` sends `Optional::Absent` rather than `Present(false)`, because a caller
//! that did not ask for a seal has not asked for the daemon's own default to be overridden
//! either.

use continuumd::codec::json::Json;
use continuumd::daemon::family::Payload;
use continuumd::protocol::envelope::Diagnostic;
use continuumd::protocol::operations::workspace::{
    WorkspaceCreateRequest, WorkspaceCreateResponse, WorkspaceForkRequest, WorkspaceForkResponse,
    WorkspaceSealRequest, WorkspaceSealResponse,
};
use continuumd::protocol::scalar::{Commitment, WorkspaceHandle};
use continuumd::protocol::shared::{FileOverlay, SnapshotComponents};
use continuumd::protocol::spec::{Optional, ProtocolEnum};

use crate::error::CliError;
use crate::format::Format;
use crate::render::{self, Projection, Rendered};
use crate::wire::{Admitted, Connection, Outcome, Transport};

/// The wire operation `snapshot create` drives, in the registry's own spelling.
pub const CREATE_OPERATION: &str = "workspace.create";

/// The wire operation `snapshot fork` drives.
pub const FORK_OPERATION: &str = "workspace.fork";

/// The wire operation `snapshot seal` drives.
pub const SEAL_OPERATION: &str = "workspace.seal";

// --- snapshot create ------------------------------------------------------------------------

/// The parsed, typed arguments of one `snapshot create` invocation.
#[derive(Debug, Clone)]
pub struct CreateArgs {
    /// The ten plan §4.2 component lists, their epochs, and the governing intent — the
    /// caller's declaration, decoded from `--components` before any frame is written.
    pub components: SnapshotComponents,
    /// Editor buffers overlaid on `components`. Empty means the caller declared none, which
    /// travels as `Optional::Absent` rather than an empty list.
    pub overlay: Vec<FileOverlay>,
    /// Whether to seal on creation. `false` means "not asked for", not "asked not to" — see
    /// this module's doc.
    pub seal: bool,
}

impl CreateArgs {
    fn request(&self) -> WorkspaceCreateRequest {
        WorkspaceCreateRequest {
            components: self.components.clone(),
            overlay: optional_list(&self.overlay),
            seal: if self.seal {
                Optional::Present(true)
            } else {
                Optional::Absent
            },
        }
    }
}

/// Run `snapshot create`: name an immutable snapshot from the declared identities.
///
/// # Errors
///
/// [`CliError::Connection`] when the wire call fails. A daemon's typed refusal is never an
/// error here — it is a rendered answer with a non-zero exit code.
pub fn create(
    connection: &mut Connection,
    transport: &mut dyn Transport,
    args: &CreateArgs,
    format: Format,
) -> Result<Rendered, CliError> {
    let outcome = connection.workspace_create(transport, &args.request())?;
    Ok(render_create(args, &outcome, format))
}

/// Project one `workspace.create` answer, in whichever [`Format`] was resolved.
#[must_use]
pub fn render_create(args: &CreateArgs, outcome: &Outcome<Payload>, format: Format) -> Rendered {
    let request = vec![
        (
            "components.intent".to_owned(),
            args.components.intent.as_str().to_owned(),
        ),
        (
            "components.files".to_owned(),
            args.components.files.len().to_string(),
        ),
        ("overlay".to_owned(), args.overlay.len().to_string()),
        ("seal".to_owned(), args.seal.to_string()),
    ];
    let request_json = vec![
        (
            "components_intent".to_owned(),
            Json::String(args.components.intent.as_str().to_owned()),
        ),
        (
            "components_files".to_owned(),
            Json::Integer(args.components.files.len() as u64),
        ),
        (
            "overlay".to_owned(),
            Json::Integer(args.overlay.len() as u64),
        ),
        ("seal".to_owned(), Json::Bool(args.seal)),
    ];
    let success = |admitted: &Admitted<Payload>| {
        let body = create_response(&admitted.payload);
        let mut lines = vec![
            (
                "snapshot".to_owned(),
                render::string_or_none(body.map(|body| body.snapshot.as_str())),
            ),
            (
                "sealed".to_owned(),
                body.map_or_else(|| "none".to_owned(), |body| body.sealed.to_string()),
            ),
        ];
        lines.extend(diagnostic_lines(body.map_or(&[], |body| &body.diagnostics)));
        let fields = vec![
            (
                "snapshot".to_owned(),
                body.map_or(Json::Null, |body| {
                    Json::String(body.snapshot.as_str().to_owned())
                }),
            ),
            (
                "sealed".to_owned(),
                body.map_or(Json::Null, |body| Json::Bool(body.sealed)),
            ),
            (
                "diagnostics".to_owned(),
                diagnostics_json(body.map_or(&[], |body| &body.diagnostics)),
            ),
        ];
        (lines, fields)
    };
    render::project(
        &Projection {
            command: "snapshot create",
            operation: CREATE_OPERATION,
            request,
            request_json,
            absent_json: vec![
                ("snapshot".to_owned(), Json::Null),
                ("sealed".to_owned(), Json::Null),
                ("diagnostics".to_owned(), Json::Null),
            ],
        },
        outcome,
        success,
        format,
    )
}

fn create_response(payload: &Payload) -> Option<&WorkspaceCreateResponse> {
    match payload {
        Payload::WorkspaceCreate(response) => Some(response),
        _ => None,
    }
}

// --- snapshot fork --------------------------------------------------------------------------

/// The parsed, typed arguments of one `snapshot fork` invocation.
#[derive(Debug, Clone)]
pub struct ForkArgs {
    /// The snapshot to fork from. Positional on every invocation — this crate remembers no
    /// "current" snapshot (INV-002).
    pub base: WorkspaceHandle,
    /// Editor buffers overlaid on the base.
    pub overlay: Vec<FileOverlay>,
    /// Patch content identities to apply to the base.
    pub patches: Vec<Commitment>,
}

impl ForkArgs {
    fn request(&self) -> WorkspaceForkRequest {
        WorkspaceForkRequest {
            base: self.base.clone(),
            overlay: optional_list(&self.overlay),
            patches: optional_list(&self.patches),
        }
    }
}

/// Run `snapshot fork`: derive a snapshot from `args.base`.
///
/// # Errors
///
/// [`CliError::Connection`] as [`create`].
pub fn fork(
    connection: &mut Connection,
    transport: &mut dyn Transport,
    args: &ForkArgs,
    format: Format,
) -> Result<Rendered, CliError> {
    let outcome = connection.workspace_fork(transport, &args.request())?;
    Ok(render_fork(args, &outcome, format))
}

/// Project one `workspace.fork` answer.
///
/// The preserved `intent` is rendered beside the new snapshot because that is the whole
/// content of INV-001 at this operation: "the fork preserves the intent binding by identity".
/// A rendering that printed only the new handle would leave a caller unable to see whether
/// the binding it depends on survived, which is the fact the invariant is about.
#[must_use]
pub fn render_fork(args: &ForkArgs, outcome: &Outcome<Payload>, format: Format) -> Rendered {
    let request = vec![
        ("base".to_owned(), args.base.as_str().to_owned()),
        ("overlay".to_owned(), args.overlay.len().to_string()),
        ("patches".to_owned(), args.patches.len().to_string()),
    ];
    let request_json = vec![
        (
            "base".to_owned(),
            Json::String(args.base.as_str().to_owned()),
        ),
        (
            "overlay".to_owned(),
            Json::Integer(args.overlay.len() as u64),
        ),
        (
            "patches".to_owned(),
            Json::Integer(args.patches.len() as u64),
        ),
    ];
    let success = |admitted: &Admitted<Payload>| {
        let body = fork_response(&admitted.payload);
        let mut lines = vec![
            (
                "snapshot".to_owned(),
                render::string_or_none(body.map(|body| body.snapshot.as_str())),
            ),
            (
                "intent".to_owned(),
                render::string_or_none(body.map(|body| body.intent.as_str())),
            ),
            (
                "pre_diff".to_owned(),
                render::string_or_none(
                    body.and_then(|body| body.pre_diff.value())
                        .map(|handle| handle.as_str()),
                ),
            ),
        ];
        lines.extend(diagnostic_lines(body.map_or(&[], |body| &body.diagnostics)));
        let fields = vec![
            (
                "snapshot".to_owned(),
                body.map_or(Json::Null, |body| {
                    Json::String(body.snapshot.as_str().to_owned())
                }),
            ),
            (
                "intent".to_owned(),
                body.map_or(Json::Null, |body| {
                    Json::String(body.intent.as_str().to_owned())
                }),
            ),
            (
                "pre_diff".to_owned(),
                body.and_then(|body| body.pre_diff.value())
                    .map_or(Json::Null, |handle| {
                        Json::String(handle.as_str().to_owned())
                    }),
            ),
            (
                "diagnostics".to_owned(),
                diagnostics_json(body.map_or(&[], |body| &body.diagnostics)),
            ),
        ];
        (lines, fields)
    };
    render::project(
        &Projection {
            command: "snapshot fork",
            operation: FORK_OPERATION,
            request,
            request_json,
            absent_json: vec![
                ("snapshot".to_owned(), Json::Null),
                ("intent".to_owned(), Json::Null),
                ("pre_diff".to_owned(), Json::Null),
                ("diagnostics".to_owned(), Json::Null),
            ],
        },
        outcome,
        success,
        format,
    )
}

fn fork_response(payload: &Payload) -> Option<&WorkspaceForkResponse> {
    match payload {
        Payload::WorkspaceFork(response) => Some(response),
        _ => None,
    }
}

// --- snapshot seal --------------------------------------------------------------------------

/// The parsed, typed arguments of one `snapshot seal` invocation.
#[derive(Debug, Clone)]
pub struct SealArgs {
    /// The snapshot to seal. Positional on every invocation (INV-002).
    pub snapshot: WorkspaceHandle,
}

impl SealArgs {
    fn request(&self) -> WorkspaceSealRequest {
        WorkspaceSealRequest {
            snapshot: self.snapshot.clone(),
        }
    }
}

/// Run `snapshot seal`: make `args.snapshot` immutable.
///
/// # Errors
///
/// [`CliError::Connection`] as [`create`].
pub fn seal(
    connection: &mut Connection,
    transport: &mut dyn Transport,
    args: &SealArgs,
    format: Format,
) -> Result<Rendered, CliError> {
    let outcome = connection.workspace_seal(transport, &args.request())?;
    Ok(render_seal(args, &outcome, format))
}

/// Project one `workspace.seal` answer.
///
/// The `root_digest` is printed in full: it is the content identity a sealed snapshot is
/// *named by*, and the fact a caller needs to check that two deployments sealed the same
/// thing. Truncating it would make the one field that can be compared uncomparable.
#[must_use]
pub fn render_seal(args: &SealArgs, outcome: &Outcome<Payload>, format: Format) -> Rendered {
    let request = vec![("snapshot".to_owned(), args.snapshot.as_str().to_owned())];
    let request_json = vec![(
        "snapshot".to_owned(),
        Json::String(args.snapshot.as_str().to_owned()),
    )];
    let success = |admitted: &Admitted<Payload>| {
        let body = seal_response(&admitted.payload);
        (
            vec![
                (
                    "sealed".to_owned(),
                    render::string_or_none(body.map(|body| body.snapshot.as_str())),
                ),
                (
                    "root_digest".to_owned(),
                    render::string_or_none(body.map(|body| body.root_digest.as_str())),
                ),
            ],
            vec![
                (
                    "sealed".to_owned(),
                    body.map_or(Json::Null, |body| {
                        Json::String(body.snapshot.as_str().to_owned())
                    }),
                ),
                (
                    "root_digest".to_owned(),
                    body.map_or(Json::Null, |body| {
                        Json::String(body.root_digest.as_str().to_owned())
                    }),
                ),
            ],
        )
    };
    render::project(
        &Projection {
            command: "snapshot seal",
            operation: SEAL_OPERATION,
            request,
            request_json,
            absent_json: vec![
                ("sealed".to_owned(), Json::Null),
                ("root_digest".to_owned(), Json::Null),
            ],
        },
        outcome,
        success,
        format,
    )
}

fn seal_response(payload: &Payload) -> Option<&WorkspaceSealResponse> {
    match payload {
        Payload::WorkspaceSeal(response) => Some(response),
        _ => None,
    }
}

// --- shared ---------------------------------------------------------------------------------

/// An empty list is an absent `optional` field, never a present empty one.
///
/// The IDL declares `overlay` and `patches` `optional`, and "absent is not null": a caller
/// that named no overlay has not asked for an empty one, and sending `[]` would be this
/// crate answering a question nobody asked.
fn optional_list<T: Clone>(values: &[T]) -> Optional<Vec<T>> {
    if values.is_empty() {
        Optional::Absent
    } else {
        Optional::Present(values.to_vec())
    }
}

/// The diagnostics a create or fork answered with, unabridged: a count, then each
/// diagnostic's severity, code, and detail.
///
/// Every one of them, never a "3 warnings" summary — a diagnostic a caller cannot read is a
/// diagnostic the daemon produced and this projection dropped.
fn diagnostic_lines(diagnostics: &[Diagnostic]) -> render::Lines {
    let mut lines = vec![("diagnostics".to_owned(), diagnostics.len().to_string())];
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        lines.push((
            format!("diagnostic[{index}].severity"),
            diagnostic.severity.as_wire().to_owned(),
        ));
        lines.push((format!("diagnostic[{index}].code"), diagnostic.code.clone()));
        lines.push((
            format!("diagnostic[{index}].detail"),
            diagnostic.detail.clone(),
        ));
        lines.push((
            format!("diagnostic[{index}].span"),
            render::string_or_none(diagnostic.span.value().map(|span| span.file.as_str())),
        ));
    }
    lines
}

/// The same diagnostics as a JSON array, one object each.
fn diagnostics_json(diagnostics: &[Diagnostic]) -> Json {
    Json::Array(
        diagnostics
            .iter()
            .map(|diagnostic| {
                Json::object([
                    (
                        "severity".to_owned(),
                        Json::String(diagnostic.severity.as_wire().to_owned()),
                    ),
                    ("code".to_owned(), Json::String(diagnostic.code.clone())),
                    ("detail".to_owned(), Json::String(diagnostic.detail.clone())),
                    (
                        "span".to_owned(),
                        diagnostic.span.value().map_or(Json::Null, |span| {
                            Json::object([
                                ("file".to_owned(), Json::String(span.file.clone())),
                                (
                                    "start_line".to_owned(),
                                    Json::Integer(u64::from(span.start_line)),
                                ),
                                (
                                    "start_column".to_owned(),
                                    Json::Integer(u64::from(span.start_column)),
                                ),
                                (
                                    "end_line".to_owned(),
                                    Json::Integer(u64::from(span.end_line)),
                                ),
                                (
                                    "end_column".to_owned(),
                                    Json::Integer(u64::from(span.end_column)),
                                ),
                            ])
                            .expect("five distinct literal keys never collide")
                        }),
                    ),
                ])
                .expect("four distinct literal keys never collide")
            })
            .collect(),
    )
}
