//! `continuum task status|resume|cancel` — the PR-6 lifecycle, driven through the wire.
//!
//! - [`status`] renders a [`TaskRecord`] — cost, budget, and the INV-007 omission manifest
//!   — faithfully: every field the wire carries, present or explicitly `none`.
//! - [`resume`] spends a continuation under a declared `states` budget and renders the
//!   typed refusal when the continuation's epochs or inputs no longer validate
//!   (`ErrorCode::StaleSnapshot`, `ContinuationEpochMismatch`, `EpochUnsupported`), never a
//!   bare failure.
//! - [`cancel`] renders `rule task.cancel_correct`'s own disjunction — a valid
//!   continuation, or a clean absence with artifacts committed-or-absent — as a named
//!   [`CancelOutcome`] token, the same three-way reading
//!   `continuumd/tests/pr6_exit_evidence.rs`'s `ExitSide` uses for the identical contract.

use continuumd::codec::json::Json;
use continuumd::daemon::family::Payload;
use continuumd::protocol::envelope::{Budget, Omission};
use continuumd::protocol::operations::task::{TaskCancelResponse, TaskResumeResponse};
use continuumd::protocol::scalar::{ContinuationHandle, TaskHandle};
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::task::TaskRecord;

use crate::error::CliError;
use crate::format::Format;
use crate::render::{self, Rendered};
use crate::wire::{Connection, Outcome, Transport};

/// A `states`-only budget: the one dimension this daemon enforces
/// (`continuumd::daemon::verification`), mirroring `continuum_mcp::client::states_budget`'s
/// identical reading of the same fact — the other eight dimensions come back as typed
/// INV-007 omissions rather than silently accepted.
#[must_use]
pub fn states_budget(states: u64) -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(states),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

// --- task status ---------------------------------------------------------------------

/// Run `task status`: read the record, render it faithfully.
///
/// # Errors
///
/// [`CliError::Connection`] when the wire call fails.
pub fn status(
    connection: &mut Connection,
    transport: &mut dyn Transport,
    task: &TaskHandle,
    format: Format,
) -> Result<Rendered, CliError> {
    let outcome = connection.task_status(transport, task)?;
    Ok(render_status(&outcome, format))
}

fn render_status(outcome: &Outcome<Payload>, format: Format) -> Rendered {
    let empty = Vec::new();
    let (record, omissions, exit_code) = match outcome {
        Outcome::Admitted(admitted) => (task_record(&admitted.payload), &admitted.omissions, 0),
        Outcome::Refused(_) => (None, &empty, 1),
    };
    match format {
        Format::Json => render_status_json(outcome, record, omissions, exit_code),
        Format::Text => render_status_lines(outcome, record, omissions, exit_code, false),
        Format::Pretty => render_status_lines(outcome, record, omissions, exit_code, true),
    }
}

fn task_record(payload: &Payload) -> Option<&TaskRecord> {
    match payload {
        Payload::TaskStatus(record) => Some(record),
        _ => None,
    }
}

fn render_status_lines(
    outcome: &Outcome<Payload>,
    record: Option<&TaskRecord>,
    omissions: &[Omission],
    exit_code: i32,
    pretty: bool,
) -> Rendered {
    let mut lines = if pretty {
        vec![("command".to_owned(), "task status".to_owned())]
    } else {
        Vec::new()
    };
    if let Some(record) = record {
        lines.push(("task".to_owned(), record.task.as_str().to_owned()));
        lines.push(("operation".to_owned(), record.operation.as_str().to_owned()));
        lines.push(("status".to_owned(), record.status.as_wire().to_owned()));
        lines.push((
            "snapshot".to_owned(),
            render::string_or_none(record.snapshot.value().map(|handle| handle.as_str())),
        ));
        lines.push((
            "intent".to_owned(),
            render::string_or_none(record.intent.value().map(|handle| handle.as_str())),
        ));
        lines.push((
            "priority_class".to_owned(),
            record.priority_class.as_wire().to_owned(),
        ));
        lines.extend(render::budget_lines("budget", &record.budget));
        lines.extend(render::cost_lines("cost", &record.cost));
        lines.push((
            "failed_reason".to_owned(),
            render::wire_or_none(record.failed_reason.value().copied()),
        ));
        lines.push((
            "continuation".to_owned(),
            render::string_or_none(record.continuation.value().map(|handle| handle.as_str())),
        ));
        lines.push((
            "non_resumable_reason".to_owned(),
            render::string_or_none(record.non_resumable_reason.value().map(String::as_str)),
        ));
        lines.push(("milestones".to_owned(), record.milestones.len().to_string()));
        lines.push((
            "committed_evidence".to_owned(),
            record.committed_evidence.len().to_string(),
        ));
    } else if let Outcome::Refused(refusal) = outcome {
        lines.push(("status".to_owned(), "refused".to_owned()));
        lines.extend(render::refusal_lines(refusal));
    }
    lines.extend(render::omission_lines(omissions));
    let next_operations = match outcome {
        Outcome::Admitted(admitted) => admitted.next_operations.len(),
        Outcome::Refused(refusal) => refusal.recovery.len(),
    };
    lines.push(("next_operations".to_owned(), next_operations.to_string()));
    Rendered {
        text: render::join_lines(&lines),
        exit_code,
    }
}

fn render_status_json(
    outcome: &Outcome<Payload>,
    record: Option<&TaskRecord>,
    omissions: &[Omission],
    exit_code: i32,
) -> Rendered {
    let mut fields = Vec::new();
    if let Some(record) = record {
        fields.push((
            "task".to_owned(),
            Json::String(record.task.as_str().to_owned()),
        ));
        fields.push((
            "operation".to_owned(),
            Json::String(record.operation.as_str().to_owned()),
        ));
        fields.push((
            "status".to_owned(),
            Json::String(record.status.as_wire().to_owned()),
        ));
        fields.push((
            "snapshot".to_owned(),
            record.snapshot.value().map_or(Json::Null, |handle| {
                Json::String(handle.as_str().to_owned())
            }),
        ));
        fields.push((
            "intent".to_owned(),
            record.intent.value().map_or(Json::Null, |handle| {
                Json::String(handle.as_str().to_owned())
            }),
        ));
        fields.push((
            "priority_class".to_owned(),
            Json::String(record.priority_class.as_wire().to_owned()),
        ));
        fields.push(("budget".to_owned(), render::budget_json(&record.budget)));
        fields.push(("cost".to_owned(), render::cost_json(&record.cost)));
        fields.push((
            "failed_reason".to_owned(),
            record
                .failed_reason
                .value()
                .map_or(Json::Null, |code| Json::String(code.as_wire().to_owned())),
        ));
        fields.push((
            "continuation".to_owned(),
            record.continuation.value().map_or(Json::Null, |handle| {
                Json::String(handle.as_str().to_owned())
            }),
        ));
        fields.push((
            "non_resumable_reason".to_owned(),
            record
                .non_resumable_reason
                .value()
                .map_or(Json::Null, |reason| Json::String(reason.clone())),
        ));
        fields.push((
            "milestones".to_owned(),
            Json::Integer(record.milestones.len() as u64),
        ));
        fields.push((
            "committed_evidence".to_owned(),
            Json::Integer(record.committed_evidence.len() as u64),
        ));
        fields.push(("error".to_owned(), Json::Null));
    } else if let Outcome::Refused(refusal) = outcome {
        fields.push(("error".to_owned(), render::refusal_json(refusal)));
    }
    fields.push(("omissions".to_owned(), render::omissions_json(omissions)));
    Rendered {
        text: render::json_text(&render::envelope_json(fields)),
        exit_code,
    }
}

// --- task resume -----------------------------------------------------------------------

/// Run `task resume`: spend `continuation` under a `states` ceiling, render the typed
/// refusal if the continuation's epochs or inputs no longer validate.
///
/// # Errors
///
/// [`CliError::Connection`] when the wire call fails.
pub fn resume(
    connection: &mut Connection,
    transport: &mut dyn Transport,
    continuation: &ContinuationHandle,
    states: u64,
    format: Format,
) -> Result<Rendered, CliError> {
    let outcome = connection.task_resume(transport, continuation, states_budget(states))?;
    Ok(render_resume(&outcome, format))
}

fn resume_payload(payload: &Payload) -> Option<&TaskResumeResponse> {
    match payload {
        Payload::TaskResume(response) => Some(response),
        _ => None,
    }
}

fn render_resume(outcome: &Outcome<Payload>, format: Format) -> Rendered {
    let empty = Vec::new();
    let (response, omissions, exit_code) = match outcome {
        Outcome::Admitted(admitted) => (resume_payload(&admitted.payload), &admitted.omissions, 0),
        Outcome::Refused(_) => (None, &empty, 1),
    };
    let mut lines = if format == Format::Pretty {
        vec![("command".to_owned(), "task resume".to_owned())]
    } else {
        Vec::new()
    };
    match (response, outcome) {
        (Some(response), _) => {
            lines.push(("task".to_owned(), response.task.as_str().to_owned()));
            lines.push(("status".to_owned(), response.status.as_wire().to_owned()));
            lines.push(("refused".to_owned(), "false".to_owned()));
        }
        (None, Outcome::Refused(refusal)) => {
            lines.push(("refused".to_owned(), "true".to_owned()));
            lines.extend(render::refusal_lines(refusal));
        }
        (None, Outcome::Admitted(_)) => {
            // Unreachable: `Admitted` always decodes a `TaskResume` payload for
            // `task.resume` (`continuumd`'s own `debug_assert` in `Daemon::dispatch`
            // guarantees the payload matches the operation). Rendered plainly rather than
            // panicking, because a renderer must be total over what the type admits.
            lines.push(("refused".to_owned(), "false".to_owned()));
        }
    }
    lines.extend(render::omission_lines(omissions));
    let next_operations = match outcome {
        Outcome::Admitted(admitted) => admitted.next_operations.len(),
        Outcome::Refused(refusal) => refusal.recovery.len(),
    };
    lines.push(("next_operations".to_owned(), next_operations.to_string()));

    if format == Format::Json {
        return render_resume_json(response, outcome, omissions, exit_code);
    }
    Rendered {
        text: render::join_lines(&lines),
        exit_code,
    }
}

fn render_resume_json(
    response: Option<&TaskResumeResponse>,
    outcome: &Outcome<Payload>,
    omissions: &[Omission],
    exit_code: i32,
) -> Rendered {
    let mut fields = Vec::new();
    match (response, outcome) {
        (Some(response), _) => {
            fields.push((
                "task".to_owned(),
                Json::String(response.task.as_str().to_owned()),
            ));
            fields.push((
                "status".to_owned(),
                Json::String(response.status.as_wire().to_owned()),
            ));
            fields.push(("refused".to_owned(), Json::Bool(false)));
            fields.push(("error".to_owned(), Json::Null));
        }
        (None, Outcome::Refused(refusal)) => {
            fields.push(("refused".to_owned(), Json::Bool(true)));
            fields.push(("error".to_owned(), render::refusal_json(refusal)));
        }
        (None, Outcome::Admitted(_)) => {
            fields.push(("refused".to_owned(), Json::Bool(false)));
            fields.push(("error".to_owned(), Json::Null));
        }
    }
    fields.push(("omissions".to_owned(), render::omissions_json(omissions)));
    Rendered {
        text: render::json_text(&render::envelope_json(fields)),
        exit_code,
    }
}

// --- task cancel -----------------------------------------------------------------------

/// `rule task.cancel_correct`'s disjunction, named for a caller to branch on: a valid
/// continuation, or a clean absence — the campaign either published nothing, or it closed
/// with everything it published committed and nothing dangling over it. The same three-way
/// reading `continuumd/tests/pr6_exit_evidence.rs`'s `ExitSide` gives the identical wire
/// contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelOutcome {
    /// A `cont_*` was handed back: committed partial evidence sits behind it, valid by the
    /// protocol's own resumability rule.
    ValidContinuation,
    /// No continuation, and nothing was published — a clean absence.
    NothingPublished,
    /// No continuation, and the campaign closed: what it published is a whole answer, not
    /// a partial one, so nothing dangles over it — also a clean absence, of a different
    /// shape.
    ClosedAndComplete,
}

impl CancelOutcome {
    /// A stable, kebab-case token for rendering.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::ValidContinuation => "valid-continuation",
            Self::NothingPublished => "nothing-published",
            Self::ClosedAndComplete => "closed-and-complete",
        }
    }

    /// Classify one `task.cancel` answer.
    #[must_use]
    pub fn of(response: &TaskCancelResponse) -> Self {
        match (&response.continuation, response.committed_evidence.len()) {
            (Nullable::Value(_), _) => Self::ValidContinuation,
            (Nullable::Null, 0) => Self::NothingPublished,
            (Nullable::Null, _) => Self::ClosedAndComplete,
        }
    }
}

/// Run `task cancel`: request, drain, finalize; render which side of `rule
/// task.cancel_correct`'s disjunction the answer landed on.
///
/// # Errors
///
/// [`CliError::Connection`] when the wire call fails.
pub fn cancel(
    connection: &mut Connection,
    transport: &mut dyn Transport,
    task: &TaskHandle,
    format: Format,
) -> Result<Rendered, CliError> {
    let outcome = connection.task_cancel(transport, task)?;
    Ok(render_cancel(&outcome, format))
}

fn cancel_payload(payload: &Payload) -> Option<&TaskCancelResponse> {
    match payload {
        Payload::TaskCancel(response) => Some(response),
        _ => None,
    }
}

fn render_cancel(outcome: &Outcome<Payload>, format: Format) -> Rendered {
    let empty = Vec::new();
    let (response, omissions, exit_code) = match outcome {
        Outcome::Admitted(admitted) => (cancel_payload(&admitted.payload), &admitted.omissions, 0),
        Outcome::Refused(_) => (None, &empty, 1),
    };
    match format {
        Format::Json => render_cancel_json(response, outcome, omissions, exit_code),
        Format::Text => render_cancel_lines(response, outcome, omissions, exit_code, false),
        Format::Pretty => render_cancel_lines(response, outcome, omissions, exit_code, true),
    }
}

fn render_cancel_lines(
    response: Option<&TaskCancelResponse>,
    outcome: &Outcome<Payload>,
    omissions: &[Omission],
    exit_code: i32,
    pretty: bool,
) -> Rendered {
    let mut lines = if pretty {
        vec![("command".to_owned(), "task cancel".to_owned())]
    } else {
        Vec::new()
    };
    if let Some(response) = response {
        let side = CancelOutcome::of(response);
        lines.push(("task".to_owned(), response.task.as_str().to_owned()));
        lines.push(("status".to_owned(), response.status.as_wire().to_owned()));
        lines.push(("outcome".to_owned(), side.token().to_owned()));
        lines.push((
            "continuation".to_owned(),
            render::string_or_none(response.continuation.value().map(|handle| handle.as_str())),
        ));
        lines.push((
            "committed_evidence".to_owned(),
            response.committed_evidence.len().to_string(),
        ));
    } else if let Outcome::Refused(refusal) = outcome {
        lines.push(("status".to_owned(), "refused".to_owned()));
        lines.extend(render::refusal_lines(refusal));
    }
    lines.extend(render::omission_lines(omissions));
    let next_operations = match outcome {
        Outcome::Admitted(admitted) => admitted.next_operations.len(),
        Outcome::Refused(refusal) => refusal.recovery.len(),
    };
    lines.push(("next_operations".to_owned(), next_operations.to_string()));
    Rendered {
        text: render::join_lines(&lines),
        exit_code,
    }
}

fn render_cancel_json(
    response: Option<&TaskCancelResponse>,
    outcome: &Outcome<Payload>,
    omissions: &[Omission],
    exit_code: i32,
) -> Rendered {
    let mut fields = Vec::new();
    if let Some(response) = response {
        let side = CancelOutcome::of(response);
        fields.push((
            "task".to_owned(),
            Json::String(response.task.as_str().to_owned()),
        ));
        fields.push((
            "status".to_owned(),
            Json::String(response.status.as_wire().to_owned()),
        ));
        fields.push(("outcome".to_owned(), Json::String(side.token().to_owned())));
        fields.push((
            "continuation".to_owned(),
            response.continuation.value().map_or(Json::Null, |handle| {
                Json::String(handle.as_str().to_owned())
            }),
        ));
        fields.push((
            "committed_evidence".to_owned(),
            Json::Integer(response.committed_evidence.len() as u64),
        ));
        fields.push(("error".to_owned(), Json::Null));
    } else if let Outcome::Refused(refusal) = outcome {
        fields.push(("error".to_owned(), render::refusal_json(refusal)));
    }
    fields.push(("omissions".to_owned(), render::omissions_json(omissions)));
    Rendered {
        text: render::json_text(&render::envelope_json(fields)),
        exit_code,
    }
}
