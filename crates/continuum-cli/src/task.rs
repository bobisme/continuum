//! `continuum task status|resume|cancel` — the PR-6 lifecycle, driven through the wire.
//!
//! # The registry mapping, stated
//!
//! | Command | Wire operation |
//! |---|---|
//! | `task status` | `task.status` |
//! | `task resume` | `task.resume` |
//! | `task cancel` | `task.cancel` |
//!
//! - [`status`] renders a [`TaskRecord`] — cost, budget, epochs, milestones, and the INV-007
//!   omission manifest — faithfully: every field the wire carries, present or explicitly
//!   `none`.
//! - [`resume`] spends a continuation under a declared `states` budget and renders the
//!   typed refusal when the continuation's epochs or inputs no longer validate
//!   (`ErrorCode::StaleSnapshot`, `ContinuationEpochMismatch`, `EpochUnsupported`), never a
//!   bare failure.
//! - [`cancel`] renders `rule task.cancel_correct`'s own disjunction — a valid
//!   continuation, or a clean absence with artifacts committed-or-absent — as a named
//!   [`CancelOutcome`] token, the same three-way reading
//!   `continuumd/tests/pr6_exit_evidence.rs`'s `ExitSide` uses for the identical contract.
//!
//! # What bn-ybh1z changed here
//!
//! This group shipped before the shared output contract did and rendered its own envelopes.
//! Four divergences from [`crate::contract`] were fixed rather than documented as
//! exceptions:
//!
//! 1. **No `operation`, no [`crate::render::Depth`].** A caller could not tell a refusal
//!    about this request from one about a surface this deployment does not serve — the
//!    INV-008 distinction the rest of the crate carries as a typed token. All three commands
//!    now go through [`crate::render::project`], so both are on every arm.
//! 2. **A JSON refusal arm that dropped every response key.** `task status` and `task cancel`
//!    answered `{error, omissions, advice}` on the refusal arm and a dozen more keys on the
//!    success arm, so a parser had to branch on which keys existed. The `absent_json` half of
//!    [`crate::render::Projection`] exists to prevent exactly that, and these commands now
//!    use it: one key set, whichever arm the answer took.
//! 3. **An invented `refused: bool`.** `task resume` carried one. `depth` says the same
//!    thing, typed and finer, and `error` says it structurally; a third spelling of one fact
//!    is a field the daemon never sent.
//! 4. **Dropped fields.** `TaskRecord.epochs` reached no format at all, and `milestones` and
//!    `committed_evidence` reached the machine channel as counts. The record's six epochs are
//!    rendered by name, and both lists travel as lists in JSON with their counts still
//!    readable in the text formats — an array renders its length under the contract's one
//!    rule, so nothing had to be printed twice to keep both readings.
//!
//! The daemon's response body is rendered under `record.`, on all three commands. That is
//! forced and not cosmetic: [`TaskRecord`] declares an `operation` field — the operation the
//! *task* runs — and `operation` is one of [`crate::contract::RESERVED_KEYS`], naming the
//! operation this *command* drove. Two facts under one key is the ambiguity the contract
//! exists to prevent, so the record keeps its own names one level down and the request echo
//! keeps the top level.

use continuumd::codec::json::Json;
use continuumd::daemon::family::Payload;
use continuumd::protocol::envelope::{Budget, EpochSet};
use continuumd::protocol::operations::task::{TaskCancelResponse, TaskResumeResponse};
use continuumd::protocol::scalar::{ContinuationHandle, TaskHandle};
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::task::{Milestone, TaskRecord};

use crate::error::CliError;
use crate::format::Format;
use crate::render::{self, Lines, Projection, Rendered};
use crate::wire::{Admitted, Connection, Outcome, Transport};

/// The wire operation `task status` drives, in the registry's own spelling.
pub const STATUS_OPERATION: &str = "task.status";

/// The wire operation `task resume` drives.
pub const RESUME_OPERATION: &str = "task.resume";

/// The wire operation `task cancel` drives.
pub const CANCEL_OPERATION: &str = "task.cancel";

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
    Ok(render_status(task, &outcome, format))
}

/// Project one `task.status` answer, in whichever [`Format`] was resolved.
#[must_use]
pub fn render_status(task: &TaskHandle, outcome: &Outcome<Payload>, format: Format) -> Rendered {
    let success = |admitted: &Admitted<Payload>| {
        let record = task_record(&admitted.payload);
        (record_lines(record), record_json(record))
    };
    render::project(
        &Projection {
            command: "task status",
            operation: STATUS_OPERATION,
            request: vec![("task".to_owned(), task.as_str().to_owned())],
            request_json: vec![("task".to_owned(), Json::String(task.as_str().to_owned()))],
            absent_json: vec![("record".to_owned(), Json::Null)],
        },
        outcome,
        success,
        format,
    )
}

fn task_record(payload: &Payload) -> Option<&TaskRecord> {
    match payload {
        Payload::TaskStatus(record) => Some(record),
        _ => None,
    }
}

/// The whole [`TaskRecord`] as `key  value` lines under `record.`, or the same key set
/// reading `none`.
///
/// Every declared field, including the two the pre-bn-ybh1z rendering left out: `epochs` (all
/// six identities plus the negotiated protocol version) and the *members* of `milestones`.
/// A task's epochs are what a continuation is validated against — `ContinuationEpochMismatch`
/// is a refusal a caller can only understand with them in hand — so leaving them unrendered
/// made the one command that could explain a stale continuation unable to.
fn record_lines(record: Option<&TaskRecord>) -> Lines {
    let Some(record) = record else {
        return vec![("record".to_owned(), "none".to_owned())];
    };
    let mut lines = vec![
        ("record".to_owned(), RECORD_FIELDS.to_string()),
        ("record.task".to_owned(), record.task.as_str().to_owned()),
        (
            "record.operation".to_owned(),
            record.operation.as_str().to_owned(),
        ),
        (
            "record.status".to_owned(),
            record.status.as_wire().to_owned(),
        ),
        (
            "record.snapshot".to_owned(),
            render::string_or_none(record.snapshot.value().map(|handle| handle.as_str())),
        ),
        (
            "record.intent".to_owned(),
            render::string_or_none(record.intent.value().map(|handle| handle.as_str())),
        ),
        (
            "record.priority_class".to_owned(),
            record.priority_class.as_wire().to_owned(),
        ),
    ];
    lines.extend(render::budget_lines("record.budget", &record.budget));
    lines.extend(render::cost_lines("record.cost", &record.cost));
    lines.extend(epoch_lines("record.epochs", &record.epochs));
    lines.push((
        "record.failed_reason".to_owned(),
        render::wire_or_none(record.failed_reason.value().copied()),
    ));
    lines.push((
        "record.continuation".to_owned(),
        render::string_or_none(record.continuation.value().map(|handle| handle.as_str())),
    ));
    lines.push((
        "record.non_resumable_reason".to_owned(),
        render::string_or_none(record.non_resumable_reason.value().map(String::as_str)),
    ));
    lines.extend(milestone_lines("record.milestones", &record.milestones));
    lines.extend(render::handle_lines(
        "record.committed_evidence",
        record
            .committed_evidence
            .iter()
            .map(|handle| handle.as_str()),
    ));
    lines
}

/// The count of fields [`record_json`] puts in the `record` object, which is what the bare
/// `record` line reads under [`crate::contract`]'s one rule.
///
/// A named constant rather than a literal so the two renderings cannot drift: a field added
/// to one and not the other fails `tests/output_contract.rs` immediately, and a field added
/// to both without touching this fails it too.
const RECORD_FIELDS: usize = 14;

fn record_json(record: Option<&TaskRecord>) -> Vec<(String, Json)> {
    let Some(record) = record else {
        return vec![("record".to_owned(), Json::Null)];
    };
    let fields = vec![
        (
            "task".to_owned(),
            Json::String(record.task.as_str().to_owned()),
        ),
        (
            "operation".to_owned(),
            Json::String(record.operation.as_str().to_owned()),
        ),
        (
            "status".to_owned(),
            Json::String(record.status.as_wire().to_owned()),
        ),
        (
            "snapshot".to_owned(),
            record.snapshot.value().map_or(Json::Null, |handle| {
                Json::String(handle.as_str().to_owned())
            }),
        ),
        (
            "intent".to_owned(),
            record.intent.value().map_or(Json::Null, |handle| {
                Json::String(handle.as_str().to_owned())
            }),
        ),
        (
            "priority_class".to_owned(),
            Json::String(record.priority_class.as_wire().to_owned()),
        ),
        ("budget".to_owned(), render::budget_json(&record.budget)),
        ("cost".to_owned(), render::cost_json(&record.cost)),
        ("epochs".to_owned(), epoch_json(&record.epochs)),
        (
            "failed_reason".to_owned(),
            record
                .failed_reason
                .value()
                .map_or(Json::Null, |code| Json::String(code.as_wire().to_owned())),
        ),
        (
            "continuation".to_owned(),
            record.continuation.value().map_or(Json::Null, |handle| {
                Json::String(handle.as_str().to_owned())
            }),
        ),
        (
            "non_resumable_reason".to_owned(),
            record
                .non_resumable_reason
                .value()
                .map_or(Json::Null, |reason| Json::String(reason.clone())),
        ),
        ("milestones".to_owned(), milestones_json(&record.milestones)),
        (
            "committed_evidence".to_owned(),
            render::handle_json(
                record
                    .committed_evidence
                    .iter()
                    .map(|handle| handle.as_str()),
            ),
        ),
    ];
    debug_assert_eq!(
        fields.len(),
        RECORD_FIELDS,
        "the bare `record` line reads this object's field count"
    );
    vec![(
        "record".to_owned(),
        Json::object(fields).expect("the record's field names are distinct"),
    )]
}

/// The six epoch identities a task is pinned to, plus the negotiated protocol version.
///
/// `EpochSet.protocol` renders as its canonical `major.minor` spelling — the same string
/// `ProtocolEpoch::identity` mints — because that *is* its content identity, and a projection
/// that split it into two numbers would give one fact two keys.
fn epoch_lines(prefix: &str, epochs: &EpochSet) -> Lines {
    let mut lines = vec![(prefix.to_owned(), EPOCH_FIELDS.to_string())];
    lines.push((format!("{prefix}.protocol"), epochs.protocol.to_string()));
    for (name, value) in epoch_identities(epochs) {
        lines.push((
            format!("{prefix}.{name}"),
            render::string_or_none(value.value().map(|epoch| epoch.as_str())),
        ));
    }
    lines
}

fn epoch_json(epochs: &EpochSet) -> Json {
    let mut fields = vec![(
        "protocol".to_owned(),
        Json::String(epochs.protocol.to_string()),
    )];
    for (name, value) in epoch_identities(epochs) {
        fields.push((
            (*name).to_owned(),
            value
                .value()
                .map_or(Json::Null, |epoch| Json::String(epoch.as_str().to_owned())),
        ));
    }
    debug_assert_eq!(fields.len(), EPOCH_FIELDS);
    Json::object(fields).expect("the seven epoch names are distinct")
}

/// The count of fields [`epoch_json`] emits, which the bare `record.epochs` line reads.
const EPOCH_FIELDS: usize = 7;

/// The six nullable identities of an [`EpochSet`], in the order it declares them.
fn epoch_identities(
    epochs: &EpochSet,
) -> [(
    &'static str,
    &Nullable<continuumd::protocol::scalar::EpochIdentity>,
); 6] {
    [
        ("semantic", &epochs.semantic),
        ("intent", &epochs.intent),
        ("evidence", &epochs.evidence),
        ("proof", &epochs.proof),
        ("corpus", &epochs.corpus),
        ("engine", &epochs.engine),
    ]
}

/// Every milestone the task reached, by name and timestamp — never the count alone.
///
/// "Semantic milestones reached so far, in order" is the record's own description, and the
/// order carries the meaning: which milestone a task last reached is what says how far it
/// got before it suspended.
fn milestone_lines(prefix: &str, milestones: &[Milestone]) -> Lines {
    let mut lines = vec![(prefix.to_owned(), milestones.len().to_string())];
    for (index, milestone) in milestones.iter().enumerate() {
        lines.push((format!("{prefix}[{index}].name"), milestone.name.clone()));
        lines.push((
            format!("{prefix}[{index}].at"),
            milestone.at.as_str().to_owned(),
        ));
    }
    lines
}

fn milestones_json(milestones: &[Milestone]) -> Json {
    Json::Array(
        milestones
            .iter()
            .map(|milestone| {
                Json::object([
                    ("name".to_owned(), Json::String(milestone.name.clone())),
                    (
                        "at".to_owned(),
                        Json::String(milestone.at.as_str().to_owned()),
                    ),
                ])
                .expect("two distinct literal keys never collide")
            })
            .collect(),
    )
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
    Ok(render_resume(continuation, states, &outcome, format))
}

fn resume_payload(payload: &Payload) -> Option<&TaskResumeResponse> {
    match payload {
        Payload::TaskResume(response) => Some(response),
        _ => None,
    }
}

/// Project one `task.resume` answer, in whichever [`Format`] was resolved.
///
/// The request echo is the *continuation* and the ceiling the caller declared, which are the
/// two things this invocation chose; the task the continuation resolves to is the daemon's
/// answer, under `record.`, and this crate never asserts which one it will be.
#[must_use]
pub fn render_resume(
    continuation: &ContinuationHandle,
    states: u64,
    outcome: &Outcome<Payload>,
    format: Format,
) -> Rendered {
    let success = |admitted: &Admitted<Payload>| {
        let response = resume_payload(&admitted.payload);
        let Some(response) = response else {
            // Unreachable: `Admitted` always decodes a `TaskResume` payload for
            // `task.resume` (`continuumd`'s own `debug_assert` in `Daemon::dispatch`
            // guarantees the payload matches the operation). Rendered plainly rather than
            // panicking, because a renderer must be total over what the type admits.
            return (
                vec![("record".to_owned(), "none".to_owned())],
                vec![("record".to_owned(), Json::Null)],
            );
        };
        (
            vec![
                ("record".to_owned(), "2".to_owned()),
                ("record.task".to_owned(), response.task.as_str().to_owned()),
                (
                    "record.status".to_owned(),
                    response.status.as_wire().to_owned(),
                ),
            ],
            vec![(
                "record".to_owned(),
                Json::object([
                    (
                        "task".to_owned(),
                        Json::String(response.task.as_str().to_owned()),
                    ),
                    (
                        "status".to_owned(),
                        Json::String(response.status.as_wire().to_owned()),
                    ),
                ])
                .expect("two distinct literal keys never collide"),
            )],
        )
    };
    render::project(
        &Projection {
            command: "task resume",
            operation: RESUME_OPERATION,
            request: vec![
                ("continuation".to_owned(), continuation.as_str().to_owned()),
                ("budget_states".to_owned(), states.to_string()),
            ],
            request_json: vec![
                (
                    "continuation".to_owned(),
                    Json::String(continuation.as_str().to_owned()),
                ),
                ("budget_states".to_owned(), Json::Integer(states)),
            ],
            absent_json: vec![("record".to_owned(), Json::Null)],
        },
        outcome,
        success,
        format,
    )
}

// --- task cancel -----------------------------------------------------------------------

/// `rule task.cancel_correct`'s disjunction, named for a caller to branch on: a valid
/// continuation, or a clean absence — the campaign either published nothing, or it closed
/// with everything it published committed and nothing dangling over it. The same three-way
/// reading `continuumd/tests/pr6_exit_evidence.rs`'s `ExitSide` gives the identical wire
/// contract.
///
/// A *derived token* in [`crate::contract`]'s sense: a total function of the answer that adds
/// no fact the answer does not carry — `continuation` and `committed_evidence` are both
/// rendered beside it — and present in every format.
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
    Ok(render_cancel(task, &outcome, format))
}

fn cancel_payload(payload: &Payload) -> Option<&TaskCancelResponse> {
    match payload {
        Payload::TaskCancel(response) => Some(response),
        _ => None,
    }
}

/// Project one `task.cancel` answer, in whichever [`Format`] was resolved.
///
/// `TaskCancelResponse.task` is the echo of the request's own `task`, so it is rendered once
/// — under the request's key — per [`crate::contract`]'s echoed-subject rule. The three
/// fields that are the daemon's *answer* (`status`, `continuation`, `committed_evidence`)
/// are under `record.`, and [`CancelOutcome`] names which side of `rule
/// task.cancel_correct`'s disjunction they land on.
#[must_use]
pub fn render_cancel(task: &TaskHandle, outcome: &Outcome<Payload>, format: Format) -> Rendered {
    let success = |admitted: &Admitted<Payload>| {
        let response = cancel_payload(&admitted.payload);
        let side = response.map(CancelOutcome::of);
        let lines = vec![
            (
                "outcome".to_owned(),
                side.map_or_else(|| "none".to_owned(), |side| side.token().to_owned()),
            ),
            (
                "record".to_owned(),
                response.map_or_else(|| "none".to_owned(), |_| "3".to_owned()),
            ),
            (
                "record.status".to_owned(),
                response.map_or_else(
                    || "none".to_owned(),
                    |response| response.status.as_wire().to_owned(),
                ),
            ),
            (
                "record.continuation".to_owned(),
                render::string_or_none(
                    response
                        .and_then(|response| response.continuation.value())
                        .map(|handle| handle.as_str()),
                ),
            ),
            (
                "record.committed_evidence".to_owned(),
                render::number_or_none(
                    response.map(|response| response.committed_evidence.len() as u64),
                ),
            ),
        ];
        let fields = vec![
            (
                "outcome".to_owned(),
                side.map_or(Json::Null, |side| Json::String(side.token().to_owned())),
            ),
            (
                "record".to_owned(),
                response.map_or(Json::Null, |response| {
                    Json::object([
                        (
                            "status".to_owned(),
                            Json::String(response.status.as_wire().to_owned()),
                        ),
                        (
                            "continuation".to_owned(),
                            response.continuation.value().map_or(Json::Null, |handle| {
                                Json::String(handle.as_str().to_owned())
                            }),
                        ),
                        (
                            "committed_evidence".to_owned(),
                            render::handle_json(
                                response
                                    .committed_evidence
                                    .iter()
                                    .map(|handle| handle.as_str()),
                            ),
                        ),
                    ])
                    .expect("three distinct literal keys never collide")
                }),
            ),
        ];
        (lines, fields)
    };
    render::project(
        &Projection {
            command: "task cancel",
            operation: CANCEL_OPERATION,
            request: vec![("task".to_owned(), task.as_str().to_owned())],
            request_json: vec![("task".to_owned(), Json::String(task.as_str().to_owned()))],
            absent_json: vec![
                ("outcome".to_owned(), Json::Null),
                ("record".to_owned(), Json::Null),
            ],
        },
        outcome,
        success,
        format,
    )
}
