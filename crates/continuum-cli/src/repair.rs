//! `continuum repair begin|review` — open and inspect a PR-20 repair transaction over
//! explicit handles.
//!
//! - [`begin`] opens a transaction against a failure the caller names, under a gate profile
//!   the caller names (`repair.begin`).
//! - [`review`] renders the reviewer projection of a transaction the caller names —
//!   semantic diff, every gate's status, and the evidence the gates rest on
//!   (`repair.review`).
//!
//! # Why the gate profile is a required flag and not a default
//!
//! `repair.begin`'s `gate_profile` is `required` in the IDL, and the vocabulary has a
//! member spelled `default`. Those are different things: the field being required means the
//! caller must *choose*, and the member named `default` is one of the four choices. A CLI
//! that filled the field in when the flag was missing would be making a policy decision —
//! which gates a repair is held to — on the caller's behalf and without saying so. So
//! `--gate-profile` is required, and `--gate-profile default` is how a caller asks for the
//! profile named `default`.
//!
//! # What `begin` deliberately does not do
//!
//! It opens a transaction and stops. `repair.apply`, `repair.evaluate`, `repair.promote`,
//! and `repair.reject` are not wired here: `promote` is `@privileged @audit_recorded` and
//! is "a service decision gated by RFC 0032, never an agent assertion (INV-015)", and a
//! one-liner that opened, applied, evaluated, and promoted would be a CLI making that
//! decision in a shell script. Each of those is its own explicit call with its own handle,
//! and the two this bone ships are the two the plan's `continuum repair` line names: open
//! one, look at one.
//!
//! # Phase A depth, stated rather than papered over
//!
//! `continuumd` registers all eight `repair` operations and declares their request and
//! response structs, and serves **none** of them: there is no `repair`
//! [`OperationFamily`](continuumd::daemon::family::OperationFamily) and no
//! [`Arguments`](continuumd::daemon::family::Arguments) arm, so a frame is refused at
//! `codec::operations::decode_arguments` with `ErrorCode::UnsupportedSemanticFeature`
//! (`rule errors.unsupported_surface`). Both commands report
//! [`Depth::Unsupported`](crate::render::Depth::Unsupported) beside the operation that is
//! unsupported, over a real round trip, and their success renderers are unit-tested against
//! the real wire types so that the day PR 20 registers a family they are already proven.

use continuumd::codec::json::Json;
use continuumd::protocol::envelope::GateOutcome;
use continuumd::protocol::operations::repair::{
    RepairBeginRequest, RepairBeginResponse, RepairReviewRequest, RepairReviewResponse,
};
use continuumd::protocol::scalar::{CrashpackHandle, RepairHandle};
use continuumd::protocol::spec::ProtocolEnum;
use continuumd::protocol::vocabulary::GateProfile;

use crate::error::CliError;
use crate::format::Format;
use crate::render::{self, Projection, Rendered};
use crate::wire::{Admitted, Connection, Outcome, Transport};

/// The wire operation `repair begin` drives, in the registry's own spelling.
pub const BEGIN_OPERATION: &str = "repair.begin";

/// The wire operation `repair review` drives, in the registry's own spelling.
pub const REVIEW_OPERATION: &str = "repair.review";

// --- repair begin -------------------------------------------------------------------------

/// The parsed, typed arguments of one `repair begin` invocation.
#[derive(Debug, Clone)]
pub struct BeginArgs {
    /// The failure to open a transaction against. Explicit on every invocation (INV-002).
    pub failure: CrashpackHandle,
    /// The gate profile the transaction is evaluated against — chosen, never defaulted
    /// (see this module's doc).
    pub gate_profile: GateProfile,
}

impl BeginArgs {
    fn request(&self) -> RepairBeginRequest {
        RepairBeginRequest {
            failure: self.failure.clone(),
            gate_profile: self.gate_profile,
        }
    }
}

/// Run `repair begin`: open a transaction against `args.failure`, render the answer.
///
/// # Errors
///
/// [`CliError::Connection`] when the wire call fails. A daemon's typed refusal — including
/// the unsupported-surface refusal this deployment answers today — is a rendered answer,
/// not an error.
pub fn begin(
    connection: &mut Connection,
    transport: &mut dyn Transport,
    args: &BeginArgs,
    format: Format,
) -> Result<Rendered, CliError> {
    let outcome = connection.repair_begin(transport, &args.request())?;
    Ok(render_begin(args, &outcome, format))
}

/// Project one `repair.begin` answer, in whichever [`Format`] was resolved.
#[must_use]
pub fn render_begin(
    args: &BeginArgs,
    outcome: &Outcome<RepairBeginResponse>,
    format: Format,
) -> Rendered {
    let request = vec![
        ("failure".to_owned(), args.failure.as_str().to_owned()),
        (
            "gate_profile".to_owned(),
            args.gate_profile.as_wire().to_owned(),
        ),
    ];
    let request_json = vec![
        (
            "failure".to_owned(),
            Json::String(args.failure.as_str().to_owned()),
        ),
        (
            "gate_profile".to_owned(),
            Json::String(args.gate_profile.as_wire().to_owned()),
        ),
    ];
    let success = |admitted: &Admitted<RepairBeginResponse>| {
        let response = &admitted.payload;
        (
            vec![("repair".to_owned(), response.repair.as_str().to_owned())],
            vec![(
                "repair".to_owned(),
                Json::String(response.repair.as_str().to_owned()),
            )],
        )
    };
    render::project(
        &Projection {
            command: "repair begin",
            operation: BEGIN_OPERATION,
            request,
            request_json,
            absent_json: vec![("repair".to_owned(), Json::Null)],
        },
        outcome,
        success,
        format,
    )
}

// --- repair review ------------------------------------------------------------------------

/// The parsed, typed arguments of one `repair review` invocation.
#[derive(Debug, Clone)]
pub struct ReviewArgs {
    /// The transaction to project. Explicit on every invocation (INV-002).
    pub repair: RepairHandle,
}

impl ReviewArgs {
    fn request(&self) -> RepairReviewRequest {
        RepairReviewRequest {
            repair: self.repair.clone(),
        }
    }
}

/// Run `repair review`: read the reviewer projection of `args.repair`, render it.
///
/// # Errors
///
/// [`CliError::Connection`] as [`begin`].
pub fn review(
    connection: &mut Connection,
    transport: &mut dyn Transport,
    args: &ReviewArgs,
    format: Format,
) -> Result<Rendered, CliError> {
    let outcome = connection.repair_review(transport, &args.request())?;
    Ok(render_review(args, &outcome, format))
}

/// Every gate, by name, with its status and the count of evidence it rests on.
///
/// Every gate the answer carries is printed — never a "3 of 10 passed" summary. A gate
/// profile is a conjunction of named obligations and which ones are outstanding is the
/// whole content of a review; a count would tell a reviewer that something is missing and
/// not what.
fn gate_lines(gates: &[GateOutcome]) -> render::Lines {
    let mut lines = vec![("gates".to_owned(), gates.len().to_string())];
    for (index, gate) in gates.iter().enumerate() {
        lines.push((
            format!("gate[{index}].name"),
            gate.name.as_wire().to_owned(),
        ));
        lines.push((
            format!("gate[{index}].status"),
            gate.status.as_wire().to_owned(),
        ));
        lines.push((
            format!("gate[{index}].evidence"),
            gate.evidence.len().to_string(),
        ));
    }
    lines
}

/// [`gate_lines`], as a JSON array — one object per gate, with its evidence handles in
/// full rather than counted, because machine output has no terminal to overflow.
fn gates_json(gates: &[GateOutcome]) -> Json {
    Json::Array(
        gates
            .iter()
            .map(|gate| {
                Json::object([
                    (
                        "name".to_owned(),
                        Json::String(gate.name.as_wire().to_owned()),
                    ),
                    (
                        "status".to_owned(),
                        Json::String(gate.status.as_wire().to_owned()),
                    ),
                    (
                        "evidence".to_owned(),
                        Json::Array(
                            gate.evidence
                                .iter()
                                .map(|handle| Json::String(handle.as_str().to_owned()))
                                .collect(),
                        ),
                    ),
                ])
                .expect("three distinct literal keys never collide")
            })
            .collect(),
    )
}

/// Project one `repair.review` answer, in whichever [`Format`] was resolved.
#[must_use]
pub fn render_review(
    args: &ReviewArgs,
    outcome: &Outcome<RepairReviewResponse>,
    format: Format,
) -> Rendered {
    let request = vec![("repair".to_owned(), args.repair.as_str().to_owned())];
    let request_json = vec![(
        "repair".to_owned(),
        Json::String(args.repair.as_str().to_owned()),
    )];
    let success = |admitted: &Admitted<RepairReviewResponse>| {
        let response = &admitted.payload;
        let mut lines = vec![(
            "semantic_diff".to_owned(),
            render::string_or_none(response.semantic_diff.value().map(|handle| handle.as_str())),
        )];
        lines.extend(gate_lines(&response.gates));
        lines.push(("evidence".to_owned(), response.evidence.len().to_string()));
        for (index, handle) in response.evidence.iter().enumerate() {
            lines.push((format!("evidence[{index}]"), handle.as_str().to_owned()));
        }
        let fields = vec![
            (
                "semantic_diff".to_owned(),
                response.semantic_diff.value().map_or(Json::Null, |handle| {
                    Json::String(handle.as_str().to_owned())
                }),
            ),
            ("gates".to_owned(), gates_json(&response.gates)),
            (
                "evidence".to_owned(),
                Json::Array(
                    response
                        .evidence
                        .iter()
                        .map(|handle| Json::String(handle.as_str().to_owned()))
                        .collect(),
                ),
            ),
        ];
        (lines, fields)
    };
    render::project(
        &Projection {
            command: "repair review",
            operation: REVIEW_OPERATION,
            request,
            request_json,
            absent_json: vec![
                ("semantic_diff".to_owned(), Json::Null),
                ("gates".to_owned(), Json::Null),
                ("evidence".to_owned(), Json::Null),
            ],
        },
        outcome,
        success,
        format,
    )
}
