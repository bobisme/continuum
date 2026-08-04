//! `continuum check start|result|await` — submit a verification campaign, and read the
//! verdict it earns.
//!
//! # The registry mapping, stated
//!
//! | Command | Wire operation |
//! |---|---|
//! | `check start` | `verification.start` |
//! | `check result` | `verification.result` |
//! | `check await` | `verification.await` |
//!
//! # How much of the flow one invocation performs, and why
//!
//! **Submit and print the handle. Never submit-then-poll.** `check start` makes exactly one
//! wire call and renders exactly what came back; `check result` takes the `task_*` handle on
//! its own command line. That is not a limitation working around a missing feature, it is
//! INV-002 and the "never silently wait forever" clause of this bone, in the same decision:
//!
//! - A submit-then-poll `check` would have to *remember* the task between the submit and the
//!   read. There is nowhere to remember it that the daemon can see — that is exactly the
//!   ambient session state INV-002 forbids, and the same reasoning [`crate::debug`] gives for
//!   `debug open`/`debug state` being two verbs rather than one.
//! - A poll loop has to decide when to stop. Every stopping rule a client could invent is a
//!   *client-side* claim about a campaign the daemon owns; the protocol already has the
//!   bounded-wait operation (`verification.await`, whose bound is the caller's own
//!   `timeout_ms`), so inventing a second one here would be a CLI-only behavior with no
//!   daemon counterpart — precisely what acceptance criterion 1 rules out.
//!
//! So a suspension is *printed*, not waited on: the status, the continuation handle, and the
//! typed facts beside them. `continuum task resume <cont_*> --states N` is the operation that
//! spends it, and it is a separate command a caller runs deliberately.
//!
//! # Two answer shapes, neither collapsed into the other
//!
//! `verification.start`'s response declares `task: TaskHandle optional` and
//! `result: VerificationResult optional` — "present instead of `task` when a cached result is
//! returned". Both are rendered, and which one arrived is a named token,
//! [`AnswerShape`], derived from the answer rather than assumed from the status.
//!
//! On the task-shaped arm the envelope's status is `task_started` or `task_suspended`, and
//! this command does **not** report that work began:
//!
//! > `task_started` means "the answer is task-shaped, not result-shaped", never "new work
//! > began at this call".
//! >
//! > — RFC 0026, "What `task_started` means when an identity resolves to a task that will not
//! > run again" (bn-3p32's A12, ratified)
//!
//! An identity that resolves to a `Failed` or `Cancelled` task takes the same lane and will
//! not run again. This command therefore prints the status token and the handle and claims
//! nothing else; `continuum task status <task_*>` is the authority on whether that task is
//! going to do anything further, and it is one command away.
//!
//! # The verdict is carried, never re-derived
//!
//! `verification.start` declares no `verdict` clause, so its answers carry `verdict: null` —
//! on *both* arms, including the cached one. The verdict rides `verification.result`, whose
//! clause is `SemanticVerdictValue`. [`result`] is therefore the command that renders it, and
//! what it renders is read straight off [`crate::wire::Admitted::verdict`] and
//! [`crate::wire::Admitted::assurance`]:
//!
//! - the verdict member, in the protocol's own wire token;
//! - `inconclusive_reason` **verbatim** — INV-008's typed reason, never inferred from the
//!   verdict, from the budget, or from the assurance class (see [`crate::render::verdict_lines`]);
//! - `assurance_class`, and the full nine-dimension assurance envelope, every dimension
//!   naming its producing engine or carrying its typed unsupported reason
//!   (`rule envelope.assurance_required`).
//!
//! Nothing in this module compares a verdict to a string, computes one from a cost, or
//! decides that a bounded run "really means" anything. Budget exhaustion in particular is not
//! a verdict anywhere here: it arrives as `inconclusive` with
//! `inconclusive_reason = ResourceExhausted`, and it is printed as exactly that.

use continuumd::codec::json::Json;
use continuumd::daemon::family::Payload;
use continuumd::protocol::envelope::Budget;
use continuumd::protocol::operations::verification::{
    VerificationAwaitRequest, VerificationResultRequest, VerificationStartRequest,
    VerificationStartResponse,
};
use continuumd::protocol::scalar::{DurationMs, TaskHandle, WorkspaceHandle};
use continuumd::protocol::shared::{Target, VerificationResult};
use continuumd::protocol::spec::{Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{Portfolio, PriorityClass};

use crate::error::CliError;
use crate::format::Format;
use crate::render::{self, Projection, Rendered};
use crate::task::states_budget;
use crate::wire::{Admitted, Connection, Outcome, Transport};

/// The wire operation `check start` drives, in the registry's own spelling.
pub const START_OPERATION: &str = "verification.start";

/// The wire operation `check result` drives.
pub const RESULT_OPERATION: &str = "verification.result";

/// The wire operation `check await` drives.
pub const AWAIT_OPERATION: &str = "verification.await";

// --- which of the two shapes came back --------------------------------------------------

/// Which of `verification.start`'s two declared answer shapes arrived.
///
/// Derived from the response body's two `optional` fields and from nothing else — not from
/// the envelope's status, which is a *different* statement (see this module's doc on what
/// `task_started` means). The two impossible-by-intent combinations are named rather than
/// folded into the others: an answer carrying neither field, and one carrying both, are
/// different malformations and a token that said "task-shaped" for either would be this
/// crate guessing on the daemon's behalf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnswerShape {
    /// `task` present, `result` absent: the answer names a task.
    Task,
    /// `result` present, `task` absent: a cached result was returned instead of a task.
    Result,
    /// Neither field present.
    Neither,
    /// Both fields present.
    Both,
}

impl AnswerShape {
    /// A stable, kebab-case token for rendering — the same string in all three formats.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Task => "task-shaped",
            Self::Result => "result-shaped",
            Self::Neither => "neither",
            Self::Both => "both",
        }
    }

    /// Classify one `verification.start` response body.
    #[must_use]
    pub const fn of(response: &VerificationStartResponse) -> Self {
        match (&response.task, &response.result) {
            (Optional::Present(_), Optional::Absent) => Self::Task,
            (Optional::Absent, Optional::Present(_)) => Self::Result,
            (Optional::Absent, Optional::Absent) => Self::Neither,
            (Optional::Present(_), Optional::Present(_)) => Self::Both,
        }
    }
}

// --- check start ------------------------------------------------------------------------

/// The parsed, typed arguments of one `check start` invocation.
#[derive(Debug, Clone)]
pub struct StartArgs {
    /// The sealed snapshot the campaign runs over. It rides the *envelope* (the IDL declares
    /// `snapshot` on `RequestEnvelope`), and the Intent Contract the campaign is checked
    /// against is the one bound to it by identity — see [`crate::wire::Connection::envelope`]
    /// for why this command names no second intent.
    pub snapshot: WorkspaceHandle,
    /// What the campaign is aimed at.
    pub target: Target,
    /// Which portfolio profile to run.
    pub portfolio: Portfolio,
    /// Scheduling class; `Absent` leaves the daemon's own default in force.
    pub priority_class: Optional<PriorityClass>,
    /// The `states` ceiling. Required on the command line and never invented: a campaign is
    /// `@task_starting`, so RFC 0026 requires a budget, and choosing one for the caller would
    /// be this crate deciding how much of a state space is enough.
    pub states: u64,
}

impl StartArgs {
    fn request(&self) -> VerificationStartRequest {
        VerificationStartRequest {
            target: self.target.clone(),
            portfolio: self.portfolio,
            // `Absent`: this daemon compiles no Context Pack, and a policy this command
            // invented would be asking for one on the caller's behalf.
            context_policy: Optional::Absent,
            priority_class: self.priority_class,
        }
    }

    fn budget(&self) -> Budget {
        states_budget(self.states)
    }
}

/// Run `check start`: submit the campaign, render the answer, print the handle.
///
/// # Errors
///
/// [`CliError::Connection`] when the wire call fails. A daemon's typed refusal is never an
/// error here — it is a rendered answer with a non-zero exit code.
pub fn start(
    connection: &mut Connection,
    transport: &mut dyn Transport,
    args: &StartArgs,
    format: Format,
) -> Result<Rendered, CliError> {
    let outcome =
        connection.verification_start(transport, &args.snapshot, &args.request(), args.budget())?;
    Ok(render_start(args, &outcome, format))
}

/// Project one `verification.start` answer, in whichever [`Format`] was resolved.
#[must_use]
pub fn render_start(args: &StartArgs, outcome: &Outcome<Payload>, format: Format) -> Rendered {
    let request = vec![
        ("snapshot".to_owned(), args.snapshot.as_str().to_owned()),
        (
            "target_kind".to_owned(),
            args.target.kind.as_wire().to_owned(),
        ),
        ("target_id".to_owned(), args.target.id.clone()),
        ("portfolio".to_owned(), args.portfolio.as_wire().to_owned()),
        (
            "priority_class".to_owned(),
            render::wire_or_none(args.priority_class.value().copied()),
        ),
        ("budget_states".to_owned(), args.states.to_string()),
    ];
    let request_json = vec![
        (
            "snapshot".to_owned(),
            Json::String(args.snapshot.as_str().to_owned()),
        ),
        (
            "target_kind".to_owned(),
            Json::String(args.target.kind.as_wire().to_owned()),
        ),
        ("target_id".to_owned(), Json::String(args.target.id.clone())),
        (
            "portfolio".to_owned(),
            Json::String(args.portfolio.as_wire().to_owned()),
        ),
        (
            "priority_class".to_owned(),
            args.priority_class
                .value()
                .map_or(Json::Null, |class| Json::String(class.as_wire().to_owned())),
        ),
        ("budget_states".to_owned(), Json::Integer(args.states)),
    ];
    let success = |admitted: &Admitted<Payload>| {
        let body = start_response(&admitted.payload);
        let shape = body.map(AnswerShape::of);
        let result = body.and_then(|body| body.result.value());
        let mut lines = vec![
            (
                "answer_shape".to_owned(),
                shape.map_or_else(|| "none".to_owned(), |shape| shape.token().to_owned()),
            ),
            ("status".to_owned(), admitted.status.as_wire().to_owned()),
            (
                "task".to_owned(),
                render::string_or_none(
                    body.and_then(|body| body.task.value())
                        .map(|handle| handle.as_str()),
                ),
            ),
            (
                "continuation".to_owned(),
                render::string_or_none(
                    admitted
                        .continuation
                        .as_ref()
                        .map(continuumd::protocol::scalar::ContinuationHandle::as_str),
                ),
            ),
        ];
        lines.extend(result_lines(result));
        lines.extend(render::verdict_lines("verdict", admitted.verdict.as_ref()));
        let fields = vec![
            (
                "answer_shape".to_owned(),
                shape.map_or(Json::Null, |shape| Json::String(shape.token().to_owned())),
            ),
            (
                "status".to_owned(),
                Json::String(admitted.status.as_wire().to_owned()),
            ),
            (
                "task".to_owned(),
                body.and_then(|body| body.task.value())
                    .map_or(Json::Null, |handle| {
                        Json::String(handle.as_str().to_owned())
                    }),
            ),
            (
                "continuation".to_owned(),
                admitted.continuation.as_ref().map_or(Json::Null, |handle| {
                    Json::String(handle.as_str().to_owned())
                }),
            ),
            ("result".to_owned(), result_json(result)),
            (
                "verdict".to_owned(),
                render::verdict_json(admitted.verdict.as_ref()),
            ),
        ];
        (lines, fields)
    };
    render::project(
        &Projection {
            command: "check start",
            operation: START_OPERATION,
            request,
            request_json,
            absent_json: vec![
                ("answer_shape".to_owned(), Json::Null),
                ("status".to_owned(), Json::Null),
                ("task".to_owned(), Json::Null),
                ("continuation".to_owned(), Json::Null),
                ("result".to_owned(), Json::Null),
                ("verdict".to_owned(), Json::Null),
            ],
        },
        outcome,
        success,
        format,
    )
}

fn start_response(payload: &Payload) -> Option<&VerificationStartResponse> {
    match payload {
        Payload::VerificationStart(response) => Some(response),
        _ => None,
    }
}

// --- check result and check await -------------------------------------------------------

/// The parsed, typed arguments of one `check result` invocation.
#[derive(Debug, Clone)]
pub struct ResultArgs {
    /// The task to read. Positional on every invocation (INV-002).
    pub task: TaskHandle,
}

/// The parsed, typed arguments of one `check await` invocation.
#[derive(Debug, Clone)]
pub struct AwaitArgs {
    /// The task to wait on. Positional on every invocation (INV-002).
    pub task: TaskHandle,
    /// How long the wait may take. Required and never defaulted: an unbounded wait is the
    /// one thing this command must not do, and the bound is the caller's to choose.
    pub timeout_ms: u64,
}

/// Run `check result`: read the typed result and render the verdict verbatim.
///
/// # Errors
///
/// [`CliError::Connection`] as [`start`].
pub fn result(
    connection: &mut Connection,
    transport: &mut dyn Transport,
    args: &ResultArgs,
    format: Format,
) -> Result<Rendered, CliError> {
    let request = VerificationResultRequest {
        task: args.task.clone(),
    };
    let outcome = connection.verification_result(transport, &request)?;
    Ok(render_result(args, &outcome, format))
}

/// Run `check await`: wait, within the caller's declared bound, and render what came back.
///
/// > Returns `status = task_started` unchanged when the budget elapses first; waiting never
/// > changes a verdict.
/// >
/// > — the IDL, on `verification.await`
///
/// So the answer to a wait that elapsed and the answer to a wait that did not are the same
/// kind of thing, and this command prints the status token rather than deciding which
/// happened.
///
/// # Errors
///
/// [`CliError::Connection`] as [`start`].
pub fn await_result(
    connection: &mut Connection,
    transport: &mut dyn Transport,
    args: &AwaitArgs,
    format: Format,
) -> Result<Rendered, CliError> {
    let request = VerificationAwaitRequest {
        task: args.task.clone(),
        timeout_ms: Optional::Present(DurationMs::new(args.timeout_ms)),
    };
    let outcome =
        connection.verification_await(transport, &request, wall_budget(args.timeout_ms))?;
    Ok(render_await(args, &outcome, format))
}

/// A `wall_ms`-only budget: the envelope ceiling a bounded wait declares.
///
/// `verification.await` is `@task_starting`, so RFC 0026 requires the envelope's own budget
/// whenever the operation is called at all, and the dimension a *wait* is bounded in is wall
/// time. Both the body's `timeout_ms` and this ceiling are set from the caller's one
/// `--timeout-ms`, mirroring [`crate::wire::Connection::task_resume`]'s identical reading of
/// the same rule.
#[must_use]
pub fn wall_budget(timeout_ms: u64) -> Budget {
    Budget {
        wall_ms: Optional::Present(DurationMs::new(timeout_ms)),
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Absent,
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

/// Project one `verification.result` answer.
#[must_use]
pub fn render_result(args: &ResultArgs, outcome: &Outcome<Payload>, format: Format) -> Rendered {
    verdict_projection(
        "check result",
        RESULT_OPERATION,
        &args.task,
        None,
        outcome,
        format,
    )
}

/// Project one `verification.await` answer — the same projection [`render_result`] gives,
/// because the two operations answer the same declared body and the same verdict clause.
#[must_use]
pub fn render_await(args: &AwaitArgs, outcome: &Outcome<Payload>, format: Format) -> Rendered {
    verdict_projection(
        "check await",
        AWAIT_OPERATION,
        &args.task,
        Some(args.timeout_ms),
        outcome,
        format,
    )
}

/// The projection `check result` and `check await` share.
///
/// One function rather than two, because the two operations declare the same response body
/// (`response VerificationResult;`) and the same `verdict SemanticVerdictValue` clause. Two
/// copies would be two chances to render the same verdict two ways, which is the drift this
/// crate's whole projection discipline exists to prevent.
fn verdict_projection(
    command: &'static str,
    operation: &'static str,
    task: &TaskHandle,
    timeout_ms: Option<u64>,
    outcome: &Outcome<Payload>,
    format: Format,
) -> Rendered {
    let mut request = vec![("task".to_owned(), task.as_str().to_owned())];
    let mut request_json = vec![("task".to_owned(), Json::String(task.as_str().to_owned()))];
    if let Some(timeout_ms) = timeout_ms {
        request.push(("timeout_ms".to_owned(), timeout_ms.to_string()));
        request_json.push(("timeout_ms".to_owned(), Json::Integer(timeout_ms)));
    }
    let success = |admitted: &Admitted<Payload>| {
        let body = verification_result(&admitted.payload);
        let mut lines = vec![
            ("status".to_owned(), admitted.status.as_wire().to_owned()),
            (
                "continuation".to_owned(),
                render::string_or_none(
                    admitted
                        .continuation
                        .as_ref()
                        .map(continuumd::protocol::scalar::ContinuationHandle::as_str),
                ),
            ),
        ];
        lines.extend(render::verdict_lines("verdict", admitted.verdict.as_ref()));
        lines.extend(render::assurance_lines(
            "assurance",
            admitted.assurance.as_ref(),
        ));
        lines.extend(result_lines(body));
        let fields = vec![
            (
                "status".to_owned(),
                Json::String(admitted.status.as_wire().to_owned()),
            ),
            (
                "continuation".to_owned(),
                admitted.continuation.as_ref().map_or(Json::Null, |handle| {
                    Json::String(handle.as_str().to_owned())
                }),
            ),
            (
                "verdict".to_owned(),
                render::verdict_json(admitted.verdict.as_ref()),
            ),
            (
                "assurance".to_owned(),
                render::assurance_json(admitted.assurance.as_ref()),
            ),
            ("result".to_owned(), result_json(body)),
        ];
        (lines, fields)
    };
    render::project(
        &Projection {
            command,
            operation,
            request,
            request_json,
            absent_json: vec![
                ("status".to_owned(), Json::Null),
                ("continuation".to_owned(), Json::Null),
                ("verdict".to_owned(), Json::Null),
                ("assurance".to_owned(), Json::Null),
                ("result".to_owned(), Json::Null),
            ],
        },
        outcome,
        success,
        format,
    )
}

fn verification_result(payload: &Payload) -> Option<&VerificationResult> {
    match payload {
        Payload::VerificationResult(result) | Payload::VerificationAwait(result) => Some(result),
        _ => None,
    }
}

// --- the shared result body ---------------------------------------------------------------

/// One [`VerificationResult`] as `key  value` lines under `result.`, or the same key set
/// reading `none`.
///
/// `fragments` is rendered unabridged and by name because the IDL says what its absence
/// means: "a fragment the campaign did not cover is reported in `omissions`, never implied".
/// A projection that printed a count would make a caller guess which five of the six a
/// finite campaign did not claim, when the answer already says so — here and, for the
/// uncovered ones, in the INV-007 manifest this command prints beside it.
fn result_lines(result: Option<&VerificationResult>) -> render::Lines {
    let Some(result) = result else {
        return vec![
            ("result.task".to_owned(), "none".to_owned()),
            ("result.target.kind".to_owned(), "none".to_owned()),
            ("result.target.id".to_owned(), "none".to_owned()),
            ("result.fragments".to_owned(), "none".to_owned()),
            ("result.evidence".to_owned(), "none".to_owned()),
            ("result.crashpack".to_owned(), "none".to_owned()),
            ("result.context".to_owned(), "none".to_owned()),
            ("result.continuation".to_owned(), "none".to_owned()),
        ];
    };
    let mut lines = vec![
        ("result.task".to_owned(), result.task.as_str().to_owned()),
        (
            "result.target.kind".to_owned(),
            result.target.kind.as_wire().to_owned(),
        ),
        ("result.target.id".to_owned(), result.target.id.clone()),
    ];
    lines.extend(render::handle_lines(
        "result.fragments",
        result.fragments.iter().map(|fragment| fragment.as_wire()),
    ));
    lines.extend(render::handle_lines(
        "result.evidence",
        result.evidence.iter().map(|handle| handle.as_str()),
    ));
    lines.push((
        "result.crashpack".to_owned(),
        render::string_or_none(result.crashpack.value().map(|handle| handle.as_str())),
    ));
    lines.push((
        "result.context".to_owned(),
        render::string_or_none(result.context.value().map(|handle| handle.as_str())),
    ));
    lines.push((
        "result.continuation".to_owned(),
        render::string_or_none(result.continuation.value().map(|handle| handle.as_str())),
    ));
    lines
}

/// The same result body as one JSON object, or [`Json::Null`] when the answer carried none.
fn result_json(result: Option<&VerificationResult>) -> Json {
    let Some(result) = result else {
        return Json::Null;
    };
    Json::object([
        (
            "task".to_owned(),
            Json::String(result.task.as_str().to_owned()),
        ),
        (
            "target".to_owned(),
            Json::object([
                (
                    "kind".to_owned(),
                    Json::String(result.target.kind.as_wire().to_owned()),
                ),
                ("id".to_owned(), Json::String(result.target.id.clone())),
            ])
            .expect("two distinct literal keys never collide"),
        ),
        (
            "fragments".to_owned(),
            render::handle_json(result.fragments.iter().map(|fragment| fragment.as_wire())),
        ),
        (
            "evidence".to_owned(),
            render::handle_json(result.evidence.iter().map(|handle| handle.as_str())),
        ),
        (
            "crashpack".to_owned(),
            result.crashpack.value().map_or(Json::Null, |handle| {
                Json::String(handle.as_str().to_owned())
            }),
        ),
        (
            "context".to_owned(),
            result.context.value().map_or(Json::Null, |handle| {
                Json::String(handle.as_str().to_owned())
            }),
        ),
        (
            "continuation".to_owned(),
            result.continuation.value().map_or(Json::Null, |handle| {
                Json::String(handle.as_str().to_owned())
            }),
        ),
    ])
    .expect("seven distinct literal keys never collide")
}
