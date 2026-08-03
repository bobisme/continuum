//! `continuum context expand` — follow a PR-11 expansion handle and print the omitted
//! detail beside the INV-007 manifest that names it.
//!
//! See the crate root doc for why every call this module makes is refused
//! `UnsupportedSemanticFeature` today: `context.expand` is registered on the wire
//! (`continuumd::protocol::registry`) but no `OperationFamily` serves the `context`
//! namespace yet (bn-28jj, PR-11/IMPL-04, open). [`render`] is written and tested against
//! both arms regardless, because a renderer that only handled the refusal it happens to see
//! today would be untested on the day the family lands.

use continuumd::protocol::operations::context::{ContextExpandRequest, ContextExpandResponse};
use continuumd::protocol::scalar::ContextHandle;
use continuumd::protocol::spec::{Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::ExpansionRelation;

use crate::error::CliError;
use crate::format::Format;
use crate::render::{self, Rendered};
use crate::wire::{Connection, Outcome, Transport};

/// The parsed, typed arguments of one `context expand` invocation.
#[derive(Debug, Clone)]
pub struct ExpandArgs {
    /// The Context Pack to expand along `relation`.
    pub context: ContextHandle,
    /// The anchor the expansion is rooted at, verbatim (IDL `anchor: String required`).
    pub anchor: String,
    /// Which of the eleven expansion relations to follow.
    pub relation: ExpansionRelation,
    /// How far to expand; `Absent` leaves the daemon's own default in force.
    pub depth: Optional<u32>,
    /// A `states` ceiling for the expansion; `context.expand` is `@task_starting`, so some
    /// budget always travels on the envelope regardless of whether this is set (see
    /// [`crate::wire::Connection::context_expand`]).
    pub states: Optional<u64>,
}

/// Run `context expand`: build the typed request, make the call, render the answer.
///
/// # Errors
///
/// [`CliError::Connection`] when the wire call itself fails (never for a daemon refusal,
/// which is a rendered answer, not an error).
pub fn expand(
    connection: &mut Connection,
    transport: &mut dyn Transport,
    args: &ExpandArgs,
    format: Format,
) -> Result<Rendered, CliError> {
    let request = ContextExpandRequest {
        context: args.context.clone(),
        anchor: args.anchor.clone(),
        relation: args.relation,
        depth: args.depth,
    };
    let outcome = connection.context_expand(transport, &request, args.states)?;
    Ok(render(args, &outcome, format))
}

/// Project one `context.expand` answer, in whichever [`Format`] was resolved.
///
/// Every arm — [`Outcome::Admitted`] and [`Outcome::Refused`] — carries the omission
/// manifest through unconditionally: there is no branch that omits it, in any format.
#[must_use]
pub fn render(
    args: &ExpandArgs,
    outcome: &Outcome<ContextExpandResponse>,
    format: Format,
) -> Rendered {
    match format {
        Format::Json => render_json(args, outcome),
        Format::Text => render_lines(args, outcome, false),
        Format::Pretty => render_lines(args, outcome, true),
    }
}

fn request_lines(args: &ExpandArgs) -> render::Lines {
    vec![
        ("context".to_owned(), args.context.as_str().to_owned()),
        ("anchor".to_owned(), args.anchor.clone()),
        ("relation".to_owned(), args.relation.as_wire().to_owned()),
        (
            "depth".to_owned(),
            render::number_or_none(args.depth.value().map(|depth| u64::from(*depth))),
        ),
    ]
}

fn render_lines(
    args: &ExpandArgs,
    outcome: &Outcome<ContextExpandResponse>,
    pretty: bool,
) -> Rendered {
    let mut lines = if pretty {
        vec![("command".to_owned(), "context expand".to_owned())]
    } else {
        Vec::new()
    };
    lines.extend(request_lines(args));
    let empty = Vec::new();
    let (status_lines, omissions, exit_code) = match outcome {
        Outcome::Admitted(admitted) => (success_lines(&admitted.payload), &admitted.omissions, 0),
        Outcome::Refused(refusal) => (render::refusal_lines(refusal), &empty, 1),
    };
    lines.push(("status".to_owned(), status_token(outcome)));
    lines.extend(status_lines);
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

fn success_lines(response: &ContextExpandResponse) -> render::Lines {
    vec![
        ("expanded".to_owned(), response.context.as_str().to_owned()),
        ("parent".to_owned(), response.parent.as_str().to_owned()),
        (
            "pack.bytes".to_owned(),
            response.pack.as_bytes().len().to_string(),
        ),
    ]
}

fn status_token(outcome: &Outcome<ContextExpandResponse>) -> String {
    match outcome {
        Outcome::Admitted(_) => "ok".to_owned(),
        Outcome::Refused(_) => "refused".to_owned(),
    }
}

fn render_json(args: &ExpandArgs, outcome: &Outcome<ContextExpandResponse>) -> Rendered {
    use continuumd::codec::json::Json;

    let mut fields = vec![
        (
            "context".to_owned(),
            Json::String(args.context.as_str().to_owned()),
        ),
        ("anchor".to_owned(), Json::String(args.anchor.clone())),
        (
            "relation".to_owned(),
            Json::String(args.relation.as_wire().to_owned()),
        ),
        (
            "depth".to_owned(),
            args.depth
                .value()
                .map_or(Json::Null, |depth| Json::Integer(u64::from(*depth))),
        ),
        ("status".to_owned(), Json::String(status_token(outcome))),
    ];
    let empty = Vec::new();
    let (omissions, exit_code) = match outcome {
        Outcome::Admitted(admitted) => {
            fields.push((
                "expanded".to_owned(),
                Json::String(admitted.payload.context.as_str().to_owned()),
            ));
            fields.push((
                "parent".to_owned(),
                Json::String(admitted.payload.parent.as_str().to_owned()),
            ));
            fields.push(("pack".to_owned(), embedded_pack(&admitted.payload)));
            fields.push(("error".to_owned(), Json::Null));
            (&admitted.omissions, 0)
        }
        Outcome::Refused(refusal) => {
            fields.push(("error".to_owned(), render::refusal_json(refusal)));
            (&empty, 1)
        }
    };
    fields.push(("omissions".to_owned(), render::omissions_json(omissions)));
    Rendered {
        text: render::json_text(&render::envelope_json(fields)),
        exit_code,
    }
}

/// The pack's own bytes, embedded verbatim rather than summarized: `context-pack.schema.json`
/// is JSON-schema-normative, so the `Opaque` a `context.expand` answer carries is already a
/// canonical JSON document (RFC 0037 ID5) and re-parsing it costs no information — unlike the
/// text/pretty renderers above, which report only its length, because a terminal is not a
/// place to print an unbounded nested document.
fn embedded_pack(response: &ContextExpandResponse) -> continuumd::codec::json::Json {
    continuumd::codec::json::Json::parse(response.pack.as_bytes())
        .unwrap_or(continuumd::codec::json::Json::Null)
}
