//! `continuum context expand` — follow a PR-11 expansion handle and print the omitted
//! detail beside the INV-007 manifest that names it.
//!
//! Live as of bn-28jj (PR-11/IMPL-04): `continuumd::daemon::context::ContextFamily` serves
//! the `context` namespace, so the success arm of [`render`] is the arm a real daemon
//! reaches. It was written and tested against both arms before that family existed —
//! "a renderer that only handled the refusal it happens to see today would be untested on
//! the day the family lands" — and bn-28jj was that day: `tests/context_expand.rs` drives
//! the same renderer over a real frame and a real answer.
//!
//! # The registry mapping, stated
//!
//! | Command | Wire operation |
//! |---|---|
//! | `context expand` | `context.expand` |
//!
//! # What bn-ybh1z changed here
//!
//! This command shipped before the shared output contract did, and rendered its own
//! envelope: no `operation`, no [`crate::render::Depth`], a CLI-authored `status: ok|refused`
//! token, a JSON refusal arm whose key set collapsed to `error` alone, and a `pack.bytes`
//! line the machine channel had no counterpart for. All four are gone. It now goes through
//! [`crate::render::project`] like every other command, so the operation and the typed depth
//! are on every arm, the key set is the same whichever arm it took, and `pack_bytes` sits
//! beside the embedded `pack` in both channels. `status` was not replaced: `depth` says the
//! same thing, typed, and finer — it distinguishes a refusal about this request from one
//! about the surface, which `refused` could not.
//!
//! The IDL's `depth` — the expansion *distance* — is echoed as `expand_depth`, because
//! `depth` is one of [`crate::contract::RESERVED_KEYS`] and one key with two meanings across
//! commands is what the contract exists to prevent.

use continuumd::codec::json::Json;
use continuumd::protocol::operations::context::{ContextExpandRequest, ContextExpandResponse};
use continuumd::protocol::scalar::ContextHandle;
use continuumd::protocol::spec::{Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::ExpansionRelation;

use crate::error::CliError;
use crate::format::Format;
use crate::render::{self, Projection, Rendered};
use crate::wire::{Admitted, Connection, Outcome, Transport};

/// The wire operation `context expand` drives, in the registry's own spelling.
pub const OPERATION: &str = "context.expand";

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
/// manifest through unconditionally: there is no branch that omits it, in any format. The
/// child pack is embedded verbatim in the machine channel and reported by length in the text
/// ones (`pack_bytes`, present in both): `context-pack.schema.json` is JSON-schema-normative,
/// so the `Opaque` a `context.expand` answer carries is already a canonical JSON document
/// (RFC 0037 ID5) and re-parsing it costs no information, while a terminal is not a place to
/// print an unbounded nested document.
#[must_use]
pub fn render(
    args: &ExpandArgs,
    outcome: &Outcome<ContextExpandResponse>,
    format: Format,
) -> Rendered {
    let request = vec![
        ("context".to_owned(), args.context.as_str().to_owned()),
        ("anchor".to_owned(), args.anchor.clone()),
        ("relation".to_owned(), args.relation.as_wire().to_owned()),
        (
            "expand_depth".to_owned(),
            render::number_or_none(args.depth.value().map(|depth| u64::from(*depth))),
        ),
    ];
    let request_json = vec![
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
            "expand_depth".to_owned(),
            args.depth
                .value()
                .map_or(Json::Null, |depth| Json::Integer(u64::from(*depth))),
        ),
    ];
    let success = |admitted: &Admitted<ContextExpandResponse>| {
        let response = &admitted.payload;
        let pack = Some(&response.pack);
        (
            vec![
                ("expanded".to_owned(), response.context.as_str().to_owned()),
                ("parent".to_owned(), response.parent.as_str().to_owned()),
                ("pack_bytes".to_owned(), render::embedded_bytes_line(pack)),
            ],
            vec![
                (
                    "expanded".to_owned(),
                    Json::String(response.context.as_str().to_owned()),
                ),
                (
                    "parent".to_owned(),
                    Json::String(response.parent.as_str().to_owned()),
                ),
                ("pack".to_owned(), render::embedded(pack)),
                ("pack_bytes".to_owned(), render::embedded_bytes(pack)),
            ],
        )
    };
    render::project(
        &Projection {
            command: "context expand",
            operation: OPERATION,
            request,
            request_json,
            absent_json: vec![
                ("expanded".to_owned(), Json::Null),
                ("parent".to_owned(), Json::Null),
                ("pack".to_owned(), Json::Null),
                ("pack_bytes".to_owned(), Json::Null),
            ],
        },
        outcome,
        success,
        format,
    )
}
