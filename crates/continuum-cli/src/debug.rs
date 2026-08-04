//! `continuum debug open|state` — drive the PR-19 verification debugger over explicit
//! handles.
//!
//! - [`open`] opens a causal-debugger branch on a failure artifact the caller names
//!   (`debug.open`).
//! - [`state`] reads the state view at a branch the caller names (`debug.state`).
//!
//! # Two verbs, because a branch handle is not a session
//!
//! `debug.open` answers with a `dbg_*` branch and `debug.state` takes one. The tempting
//! shape is a single `continuum debug <subject>` that opens a branch, remembers it, and
//! lets later commands say "the current branch" — and that is exactly the ambient state
//! INV-002 forbids: a cursor the daemon cannot see, that two terminals disagree about, and
//! that makes the same command line mean two different things on two days.
//! [`crate::wire::Connection`] holds no `dbg_*` and this module stores none; the branch is a
//! positional argument of [`state`], printed in full by [`open`] so a caller can paste it.
//!
//! # Phase A depth, stated rather than papered over
//!
//! `continuumd` registers all eleven `debug` operations
//! (`continuumd::protocol::registry`) and declares their request and response structs
//! (`continuumd::protocol::operations::debug`), and serves **none** of them: there is no
//! `debug` [`OperationFamily`](continuumd::daemon::family::OperationFamily) and no
//! [`Arguments`](continuumd::daemon::family::Arguments) arm, so
//! `codec::operations::decode_arguments` refuses the frame before dispatch begins
//! (`CodecError::UnknownOperation` ⇒ `ErrorCode::UnsupportedSemanticFeature`, `rule
//! errors.unsupported_surface`).
//!
//! Both commands therefore report [`Depth::Unsupported`] today, beside the operation name
//! that is unsupported — a typed, machine-readable answer, not a crash and not a bare
//! failure. The call is still a real frame over a real transport: nothing here
//! short-circuits, so the day PR 19 registers a `debug` family these same commands are
//! admitted and the same renderer prints the branch. The success arms below are unit-tested
//! against the real wire types now, for the reason bn-3tz60 gave for `context.expand`: a
//! renderer that only handled the refusal it happens to see today would be untested on the
//! day the family lands.

use continuumd::codec::json::Json;
use continuumd::protocol::operations::debug::{
    DebugOpenRequest, DebugOpenResponse, DebugStateRequest, DebugStateResponse,
};
use continuumd::protocol::scalar::{ArtifactHandle, DebugHandle};
use continuumd::protocol::spec::Optional;

use crate::error::CliError;
use crate::format::Format;
use crate::render::{self, Projection, Rendered};
use crate::wire::{Connection, Outcome, Transport};

/// The wire operation `debug open` drives, in the registry's own spelling.
pub const OPEN_OPERATION: &str = "debug.open";

/// The wire operation `debug state` drives, in the registry's own spelling.
pub const STATE_OPERATION: &str = "debug.state";

// --- debug open ---------------------------------------------------------------------------

/// The parsed, typed arguments of one `debug open` invocation.
#[derive(Debug, Clone)]
pub struct OpenArgs {
    /// The failure artifact — a crashpack, an execution, an evidence node — to open a
    /// branch on. Explicit on every invocation (INV-002).
    pub subject: ArtifactHandle,
    /// The observer whose view the debugger presents, when one is named. `None` leaves the
    /// daemon's own default in force rather than inventing one.
    pub observer: Option<String>,
}

impl OpenArgs {
    fn request(&self) -> DebugOpenRequest {
        DebugOpenRequest {
            subject: self.subject.clone(),
            observer: match &self.observer {
                Some(observer) => Optional::Present(observer.clone()),
                None => Optional::Absent,
            },
        }
    }
}

/// Run `debug open`: ask for a branch on `args.subject`, render the answer.
///
/// # Errors
///
/// [`CliError::Connection`] when the wire call fails. A daemon's typed refusal — including
/// the unsupported-surface refusal this deployment answers today — is a rendered answer,
/// not an error.
pub fn open(
    connection: &mut Connection,
    transport: &mut dyn Transport,
    args: &OpenArgs,
    format: Format,
) -> Result<Rendered, CliError> {
    let outcome = connection.debug_open(transport, &args.request())?;
    Ok(render_open(args, &outcome, format))
}

/// Project one `debug.open` answer, in whichever [`Format`] was resolved.
#[must_use]
pub fn render_open(
    args: &OpenArgs,
    outcome: &Outcome<DebugOpenResponse>,
    format: Format,
) -> Rendered {
    let request = vec![
        ("subject".to_owned(), args.subject.as_str().to_owned()),
        (
            "observer".to_owned(),
            render::string_or_none(args.observer.as_deref()),
        ),
    ];
    let request_json = vec![
        (
            "subject".to_owned(),
            Json::String(args.subject.as_str().to_owned()),
        ),
        (
            "observer".to_owned(),
            args.observer
                .as_ref()
                .map_or(Json::Null, |observer| Json::String(observer.clone())),
        ),
    ];
    let success = |response: &DebugOpenResponse| {
        (
            vec![
                ("branch".to_owned(), response.branch.as_str().to_owned()),
                (
                    "frontier.bytes".to_owned(),
                    render::embedded_bytes_line(Some(&response.frontier)),
                ),
            ],
            vec![
                (
                    "branch".to_owned(),
                    Json::String(response.branch.as_str().to_owned()),
                ),
                (
                    "frontier".to_owned(),
                    render::embedded(Some(&response.frontier)),
                ),
                (
                    "frontier_bytes".to_owned(),
                    render::embedded_bytes(Some(&response.frontier)),
                ),
            ],
        )
    };
    render::project(
        &Projection {
            command: "debug open",
            operation: OPEN_OPERATION,
            request,
            request_json,
            absent_json: vec![
                ("branch".to_owned(), Json::Null),
                ("frontier".to_owned(), Json::Null),
                ("frontier_bytes".to_owned(), Json::Null),
            ],
        },
        outcome,
        success,
        format,
    )
}

// --- debug state --------------------------------------------------------------------------

/// The parsed, typed arguments of one `debug state` invocation.
#[derive(Debug, Clone)]
pub struct StateArgs {
    /// The branch to read. Explicit on every invocation — this crate remembers none
    /// (INV-002).
    pub branch: DebugHandle,
    /// The observer whose view to present, when one is named.
    pub observer: Option<String>,
}

impl StateArgs {
    fn request(&self) -> DebugStateRequest {
        DebugStateRequest {
            branch: self.branch.clone(),
            observer: match &self.observer {
                Some(observer) => Optional::Present(observer.clone()),
                None => Optional::Absent,
            },
        }
    }
}

/// Run `debug state`: read the state view at `args.branch`, render the answer.
///
/// # Errors
///
/// [`CliError::Connection`] as [`open`].
pub fn state(
    connection: &mut Connection,
    transport: &mut dyn Transport,
    args: &StateArgs,
    format: Format,
) -> Result<Rendered, CliError> {
    let outcome = connection.debug_state(transport, &args.request())?;
    Ok(render_state(args, &outcome, format))
}

/// Project one `debug.state` answer, in whichever [`Format`] was resolved.
#[must_use]
pub fn render_state(
    args: &StateArgs,
    outcome: &Outcome<DebugStateResponse>,
    format: Format,
) -> Rendered {
    let request = vec![
        ("branch".to_owned(), args.branch.as_str().to_owned()),
        (
            "observer".to_owned(),
            render::string_or_none(args.observer.as_deref()),
        ),
    ];
    let request_json = vec![
        (
            "branch".to_owned(),
            Json::String(args.branch.as_str().to_owned()),
        ),
        (
            "observer".to_owned(),
            args.observer
                .as_ref()
                .map_or(Json::Null, |observer| Json::String(observer.clone())),
        ),
    ];
    let success = |response: &DebugStateResponse| {
        (
            vec![(
                "state.bytes".to_owned(),
                render::embedded_bytes_line(Some(&response.state)),
            )],
            vec![
                ("state".to_owned(), render::embedded(Some(&response.state))),
                (
                    "state_bytes".to_owned(),
                    render::embedded_bytes(Some(&response.state)),
                ),
            ],
        )
    };
    render::project(
        &Projection {
            command: "debug state",
            operation: STATE_OPERATION,
            request,
            request_json,
            absent_json: vec![
                ("state".to_owned(), Json::Null),
                ("state_bytes".to_owned(), Json::Null),
            ],
        },
        outcome,
        success,
        format,
    )
}
