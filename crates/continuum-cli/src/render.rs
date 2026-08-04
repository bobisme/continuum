//! Shared rendering: the INV-007 omission manifest, a typed refusal, and the three output
//! formats (`.agents/edict/design/cli-conventions.md`).
//!
//! # The projection contract
//!
//! Every renderer here is total, deterministic, and derived — no invented content, the
//! protocol's own closed-vocabulary tokens (`ProtocolEnum::as_wire`) rather than synonyms,
//! handles in full with no truncation, and every scalar present or explicitly `none`. This
//! is `continuum_benchmark::shell`'s projection contract, stated once there as the shape a
//! human CLI's output is held to; this module is that contract's first real implementation.
//!
//! [`crate::contract`] is the contract itself — the machine document's field rules, the pure
//! projection from it to the terminal body, the exit-code taxonomy, and G8-06's non-colour
//! statement. This module is the machinery that satisfies it, and every key it emits is
//! chosen so that [`crate::contract::divergence`] holds with no per-command exception: a
//! text key is a *path into the machine document*, a count line is a container's own length,
//! and a byte length lives beside its embedded document under a key of its own
//! (`pack_bytes`, never `pack.bytes`, which the document would read as the pack's own
//! `bytes` field).
//!
//! # INV-007 is non-suppressible here, on purpose
//!
//! Unlike `continuum_benchmark::shell::render`, which *summarizes* `omissions` behind a
//! count and an expansion command — a deliberate, documented handicap that module accepts
//! for its own reason (a terminal is not a place to print a structured list nobody asked
//! for) — [`omission_lines`] and [`omissions_json`] always render the full manifest. This
//! crate's commands are not graded on interface bytes, and the acceptance criterion this
//! module exists to satisfy is explicit: the manifest travels *alongside* the answer, in
//! every format, with no flag that removes it. There is no `--quiet-omissions`, and JSON
//! output always carries the `omissions` key, empty array or not.

use continuumd::codec::json::Json;
use continuumd::protocol::envelope::{
    ArtifactRef, AssuranceEnvelope, Budget, Cost, EnvelopeDimension, NextOperation, Omission,
    Redacted, Verdict,
};
use continuumd::protocol::scalar::{ByteCount, DurationMs, Opaque};
use continuumd::protocol::spec::ProtocolEnum;
use continuumd::protocol::vocabulary::ErrorCode;

use crate::wire::{Admitted, Outcome, Refusal};

/// One rendered `key  value` pair, in emission order.
pub type Lines = Vec<(String, String)>;

/// A command's finished output: the text to write to stdout, and the exit code the daemon's
/// answer earns.
///
/// The code is [`crate::contract::Exit`]'s, computed by [`crate::contract::Exit::of`] from
/// the same [`Outcome`] the text is rendered from: `0` when the daemon answered and no
/// verdict it carried says no, `3` when the answer is a decided no, `4` when it is INV-008's
/// typed unknown, `1` when the request was typedly refused, and `2` (from
/// [`crate::error::CliError::exit_code`]) when the connection failed below the protocol.
/// [`crate::contract`]'s module doc is the taxonomy and the reasoning; nothing in this
/// module decides it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// The text to write to stdout.
    pub text: String,
    /// The process exit code this answer earns.
    pub exit_code: i32,
}

/// Join `lines` into text: one `key`, two spaces, `value`, per line
/// (cli-conventions.md, "Two-space delimiters").
#[must_use]
pub fn join_lines(lines: &Lines) -> String {
    let mut text = String::new();
    for (key, value) in lines {
        text.push_str(key);
        text.push_str("  ");
        text.push_str(value);
        text.push('\n');
    }
    text
}

/// A present scalar's wire token, or the literal `none` (discipline: "every scalar is
/// present or explicitly `none`" — `continuum_benchmark::shell`'s rule 4).
#[must_use]
pub fn wire_or_none<T: ProtocolEnum>(value: Option<T>) -> String {
    value.map_or_else(|| "none".to_owned(), |member| member.as_wire().to_owned())
}

/// A present string, or the literal `none`.
#[must_use]
pub fn string_or_none(value: Option<&str>) -> String {
    value.map_or_else(|| "none".to_owned(), str::to_owned)
}

/// A present `u64`, or the literal `none`.
#[must_use]
pub fn number_or_none(value: Option<u64>) -> String {
    value.map_or_else(|| "none".to_owned(), |number| number.to_string())
}

// --- how deep a command actually got ----------------------------------------------------

/// What one answer says about the wire surface the command drove — INV-008's typed
/// inconclusiveness at the level of the *interface* rather than of a verdict.
///
/// # Why this is a third thing beside "admitted" and "refused"
///
/// A caller that gets a `no` needs to know which of two very different `no`s it is: *this
/// request was declined* (a bad handle, a denied capability, a lost compare-and-set) or
/// *this deployment does not serve that operation at all*. The first is about the request
/// and a caller can fix it; the second is about the build and no argument will change it.
/// Collapsing them into one non-zero exit is exactly the "opaque failure" INV-008 forbids,
/// and inventing a success for the second is the "empty success" `rule
/// errors.unsupported_surface` forbids. So the distinction is a token in every output
/// format, beside the operation it is about.
///
/// # It is derived, never asserted
///
/// [`Depth::of`] reads the daemon's own [`ErrorCode`] off the answer. Nothing in this crate
/// carries a list of "operations Phase A does not serve" — such a list would be a second
/// authority over the registry, and would go stale silently the day a family lands. When a
/// `debug` family is registered, the same call is admitted and the same code prints
/// [`Depth::Served`] with no edit here. That is what makes [`Depth::Unsupported`] a
/// *report* rather than a claim.
///
/// # It is not an exit code
///
/// bn-1g7e4 gave this type an `exit_code` and bn-ybh1z took it away, without changing what
/// either of its two refusal arms earns. The taxonomy is [`crate::contract::Exit`]'s, and it
/// classifies something this type cannot see: an *admitted* answer whose verdict says no
/// (`3`) or says "typed unknown" (`4`) is not [`Depth::Served`]'s `0`. `unsupported` and
/// `refused` still share `1`, for the reason bn-1g7e4 gave and [`crate::contract`] restates:
/// the distinction between them is [`Depth::token`], in the output, and a number that
/// duplicated a token would be a second spelling of one fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    /// The daemon ran the operation and answered its declared response body.
    Served,
    /// The daemon answered [`ErrorCode::UnsupportedSemanticFeature`]: this deployment does
    /// not serve the operation the command named (`rule errors.unsupported_surface`).
    Unsupported,
    /// The daemon answered some other typed refusal — a `no` about *this request*, not a
    /// statement about the surface.
    Refused,
}

impl Depth {
    /// A stable, kebab-case token for rendering — the same string in all three formats.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Served => "served",
            Self::Unsupported => "unsupported",
            Self::Refused => "refused",
        }
    }

    /// Read the depth off one answer.
    #[must_use]
    pub fn of<T>(outcome: &Outcome<T>) -> Self {
        match outcome {
            Outcome::Admitted(_) => Self::Served,
            Outcome::Refused(refusal) => Self::of_code(refusal.code),
        }
    }

    /// Read the depth off a refusal's typed code alone.
    #[must_use]
    pub const fn of_code(code: ErrorCode) -> Self {
        match code {
            ErrorCode::UnsupportedSemanticFeature => Self::Unsupported,
            _ => Self::Refused,
        }
    }
}

/// The two lines every command in the failure/promotion group prints first: the wire
/// operation it drove, in the registry's own spelling, and how deep it got.
///
/// The operation name is the one the command put in the envelope — a `&'static str` from
/// the call site, never a string read back off an answer — so it is present and exact on
/// the refusal arm too, which is precisely the arm on which a caller needs to know what was
/// unsupported.
#[must_use]
pub fn depth_lines(operation: &'static str, depth: Depth) -> Lines {
    vec![
        ("operation".to_owned(), operation.to_owned()),
        ("depth".to_owned(), depth.token().to_owned()),
    ]
}

/// [`depth_lines`], as the two JSON fields of the same names.
#[must_use]
pub fn depth_json(operation: &'static str, depth: Depth) -> Vec<(String, Json)> {
    vec![
        ("operation".to_owned(), Json::String(operation.to_owned())),
        ("depth".to_owned(), Json::String(depth.token().to_owned())),
    ]
}

// --- the omission manifest, unabridged --------------------------------------------------

/// The full INV-007 manifest as `key  value` lines: a count, then every omission's
/// `reason`/`subject`/`recoverable_by`, indexed.
///
/// The indexed keys are `omissions[i].…` and not `omission[i].…`: they are paths into the
/// machine document's own `omissions` array, which is what makes the text a projection of it
/// under [`crate::contract`]'s one rule. bn-ybh1z renamed them for that reason.
#[must_use]
pub fn omission_lines(omissions: &[Omission]) -> Lines {
    let mut lines = vec![("omissions".to_owned(), omissions.len().to_string())];
    for (index, omission) in omissions.iter().enumerate() {
        lines.push((
            format!("omissions[{index}].reason"),
            omission.reason.as_wire().to_owned(),
        ));
        lines.push((
            format!("omissions[{index}].subject"),
            omission.subject.clone(),
        ));
        lines.push((
            format!("omissions[{index}].recoverable_by"),
            string_or_none(
                omission
                    .recoverable_by
                    .value()
                    .map(|handle| handle.as_str()),
            ),
        ));
    }
    lines
}

/// The full INV-007 manifest as a JSON array, one object per omission. Always present —
/// `[]` when nothing was omitted, never absent from the envelope.
#[must_use]
pub fn omissions_json(omissions: &[Omission]) -> Json {
    Json::Array(
        omissions
            .iter()
            .map(|omission| {
                Json::object([
                    (
                        "reason".to_owned(),
                        Json::String(omission.reason.as_wire().to_owned()),
                    ),
                    ("subject".to_owned(), Json::String(omission.subject.clone())),
                    (
                        "recoverable_by".to_owned(),
                        match omission.recoverable_by.value() {
                            Some(handle) => Json::String(handle.as_str().to_owned()),
                            None => Json::Null,
                        },
                    ),
                ])
                .expect("three distinct literal keys never collide")
            })
            .collect(),
    )
}

// --- the shared projection of one answer --------------------------------------------------

/// Everything a projection needs that is not the answer itself.
///
/// A struct rather than five positional parameters: the rendering paths take the same facts
/// and a positional list would be five chances to swap two `&'static str`s the compiler
/// cannot tell apart.
#[derive(Debug, Clone)]
pub struct Projection {
    /// The command's own name, printed in [`Format::Pretty`](crate::format::Format::Pretty)
    /// only.
    pub command: &'static str,
    /// The wire operation, printed in every format on every arm.
    pub operation: &'static str,
    /// The request's own fields, echoed so the answer is self-describing.
    pub request: Lines,
    /// [`Projection::request`], as JSON fields.
    pub request_json: Vec<(String, Json)>,
    /// The response fields, all explicitly `null`, for the refusal arm — so the machine
    /// envelope carries the same key set whichever arm it took, and a parser never has to
    /// branch on which keys exist.
    pub absent_json: Vec<(String, Json)>,
}

/// Render one answer whose success body is projected by `success`.
///
/// Shared by `evidence show`, `debug open|state`, and `repair begin|review` because their
/// shape is identical and their bodies are not: the operation and the [`Depth`] first, the
/// request echoed, then either the response body or the typed refusal, then the INV-007
/// manifest — unabridged, on both arms, in all three formats — and the count of allowed
/// next operations.
///
/// `success` answers a pair: the `key  value` lines for the text formats, and the JSON
/// fields for the machine one. One closure rather than two so that a command cannot ship a
/// text projection of a field it forgot to put in JSON.
///
/// It is handed the whole [`Admitted`] answer rather than only its payload, because half of
/// what a caller needs is on the *envelope* and not in the response body: the
/// `ok`/`task_started`/`task_suspended` status, the task and continuation handles, and the
/// typed verdict and assurance envelope. A closure that could see only the body would force
/// a command that renders those to leave [`project`] and re-implement the shared shape.
///
/// # Three envelope facts the machine channel carries and the terminal does not
///
/// `request_id`, `cost` and `artifacts` are on every answer's envelope and reached no format
/// at all before bn-ybh1z — dropped fields, in [`crate::contract`]'s sense, on all sixteen
/// commands at once. They are added here, in the one place, and to the machine channel only:
///
/// - the **request identifier** is what correlates this invocation with a daemon log, which
///   is a machine's question;
/// - **`cost`** is nine dimensions plus a tokenizer identity, and printing ten lines of it
///   under every `snapshot seal` would make plan §13.4's "terse by default" untrue while
///   telling a human at a terminal nothing they asked for;
/// - **`artifacts`** is the list of `ArtifactRef`s the call published or names, in full.
///
/// That direction is the contract's, not a convenience: prose must be a *selection* of the
/// machine document (INV-003), never a superset of it, so a key the machine channel carries
/// and the terminal does not is exactly the allowed asymmetry — the same standing `advice`
/// has always had. `--format json` is where a caller reads them.
pub fn project<T, F>(
    projection: &Projection,
    outcome: &Outcome<T>,
    success: F,
    format: crate::format::Format,
) -> Rendered
where
    F: Fn(&Admitted<T>) -> (Lines, Vec<(String, Json)>),
{
    let depth = Depth::of(outcome);
    let empty = Vec::new();
    let omissions = match outcome {
        Outcome::Admitted(admitted) => &admitted.omissions,
        Outcome::Refused(_) => &empty,
    };
    let next = match outcome {
        Outcome::Admitted(admitted) => &admitted.next_operations,
        Outcome::Refused(refusal) => &refusal.recovery,
    };

    let exit_code = crate::contract::Exit::of(outcome).code();

    if format == crate::format::Format::Json {
        let mut fields = depth_json(projection.operation, depth);
        fields.extend(projection.request_json.iter().cloned());
        match outcome {
            Outcome::Admitted(admitted) => {
                fields.extend(success(admitted).1);
                fields.push((
                    "request_id".to_owned(),
                    Json::String(admitted.request_id.as_str().to_owned()),
                ));
                fields.push(("cost".to_owned(), cost_json(&admitted.cost)));
                fields.push(("artifacts".to_owned(), artifacts_json(&admitted.artifacts)));
                fields.push(("error".to_owned(), Json::Null));
            }
            Outcome::Refused(refusal) => {
                fields.extend(projection.absent_json.iter().cloned());
                fields.push((
                    "request_id".to_owned(),
                    Json::String(refusal.request_id.as_str().to_owned()),
                ));
                // A refusal carries no cost clause and publishes nothing: `null` and `[]`
                // are the daemon's own readings, not this crate's guesses.
                fields.push(("cost".to_owned(), Json::Null));
                fields.push(("artifacts".to_owned(), Json::Array(Vec::new())));
                fields.push(("error".to_owned(), refusal_json(refusal)));
            }
        }
        fields.push(("omissions".to_owned(), omissions_json(omissions)));
        fields.push(("next_operations".to_owned(), next_operations_json(next)));
        return Rendered {
            text: json_text(&envelope_json(fields)),
            exit_code,
        };
    }

    let mut lines = if format == crate::format::Format::Pretty {
        vec![("command".to_owned(), projection.command.to_owned())]
    } else {
        Vec::new()
    };
    lines.extend(depth_lines(projection.operation, depth));
    lines.extend(projection.request.iter().cloned());
    match outcome {
        Outcome::Admitted(admitted) => lines.extend(success(admitted).0),
        Outcome::Refused(refusal) => lines.extend(refusal_lines(refusal)),
    }
    lines.extend(omission_lines(omissions));
    lines.push(next_operations_line(next));
    Rendered {
        text: join_lines(&lines),
        exit_code,
    }
}

// --- opaque fields, carried verbatim ------------------------------------------------------

/// An `Opaque` field's carried value, embedded verbatim rather than summarized.
///
/// `rule encoding.opaque_payloads` fixes what such a field holds: "carried verbatim as a
/// canonical value of the negotiated encoding". So re-parsing it costs no information and
/// loses no ordering — the value was canonical before it was embedded and is canonical
/// after — and a machine reading this crate's JSON gets the document itself rather than a
/// length it would have to make a second call to resolve.
///
/// [`Json::Null`] means the field was absent. A *present* field that does not parse also
/// reads `null` here, which is why every caller in this crate prints
/// [`embedded_bytes`] beside it: the pair `(null, 512)` is structurally distinguishable
/// from `(null, null)`, so an unparseable body is visible rather than silently identical to
/// an absent one.
#[must_use]
pub fn embedded(opaque: Option<&Opaque>) -> Json {
    opaque.map_or(Json::Null, |value| {
        Json::parse(value.as_bytes()).unwrap_or(Json::Null)
    })
}

/// The byte length of an `Opaque` field, or [`Json::Null`] when the field is absent.
#[must_use]
pub fn embedded_bytes(opaque: Option<&Opaque>) -> Json {
    opaque.map_or(Json::Null, |value| {
        Json::Integer(value.as_bytes().len() as u64)
    })
}

/// The byte length of an `Opaque` field as a line value, or the literal `none`.
#[must_use]
pub fn embedded_bytes_line(opaque: Option<&Opaque>) -> String {
    number_or_none(opaque.map(|value| value.as_bytes().len() as u64))
}

// --- a typed refusal ------------------------------------------------------------------

/// A refusal's `key  value` lines: the typed code first, then detail, retryability,
/// recovery count, continuation, and non-resumable reason — every field RFC 0026 declares
/// on `Error`, present or explicitly `none`.
#[must_use]
pub fn refusal_lines(refusal: &Refusal) -> Lines {
    vec![
        ("error.code".to_owned(), refusal.code.as_wire().to_owned()),
        ("error.detail".to_owned(), refusal.detail.clone()),
        ("error.retryable".to_owned(), refusal.retryable.to_string()),
        (
            "error.recovery".to_owned(),
            refusal.recovery.len().to_string(),
        ),
        (
            "error.continuation".to_owned(),
            string_or_none(refusal.continuation.as_ref().map(|handle| handle.as_str())),
        ),
        (
            "error.non_resumable_reason".to_owned(),
            string_or_none(refusal.non_resumable_reason.as_deref()),
        ),
    ]
}

/// A refusal as a JSON object — the same six fields [`refusal_lines`] renders, keyed for
/// machine reading.
///
/// `recovery` is the **list**, not the count the text formats print. RFC 0026 makes it "the
/// only recovery channel: a list of allowed operations with pre-filled arguments", so a
/// machine channel that carried a length would drop the one field a caller recovers *with* —
/// and the text's count is still a projection of it, because an array renders its length
/// ([`crate::contract`]'s one rule). bn-ybh1z fixed this; before it, the list reached no
/// caller in any format.
#[must_use]
pub fn refusal_json(refusal: &Refusal) -> Json {
    Json::object([
        (
            "code".to_owned(),
            Json::String(refusal.code.as_wire().to_owned()),
        ),
        ("detail".to_owned(), Json::String(refusal.detail.clone())),
        ("retryable".to_owned(), Json::Bool(refusal.retryable)),
        (
            "recovery".to_owned(),
            next_operations_json(&refusal.recovery),
        ),
        (
            "continuation".to_owned(),
            match &refusal.continuation {
                Some(handle) => Json::String(handle.as_str().to_owned()),
                None => Json::Null,
            },
        ),
        (
            "non_resumable_reason".to_owned(),
            match &refusal.non_resumable_reason {
                Some(reason) => Json::String(reason.clone()),
                None => Json::Null,
            },
        ),
    ])
    .expect("six distinct literal keys never collide")
}

// --- budget and cost, the nine shared dimensions ------------------------------------------

/// One dimension name, in the fixed order `Budget`/`Cost` both declare (SD-12, "one
/// budget/cost dimension list").
const DIMENSIONS: [&str; 9] = [
    "wall_ms",
    "cpu_ms",
    "memory_bytes",
    "states",
    "solver_ms",
    "proof_ms",
    "tokens",
    "candidates",
    "bytes",
];

/// A requested [`Budget`]'s nine dimensions as `key  value` lines under `prefix`, each a
/// number or the literal `none` — "a dimension the engine does not measure is absent, never
/// zero" (`Cost`'s own doc; the same reading applies to a ceiling nobody declared).
#[must_use]
pub fn budget_lines(prefix: &str, budget: &Budget) -> Lines {
    let values = [
        budget.wall_ms.value().copied().map(DurationMs::millis),
        budget.cpu_ms.value().copied().map(DurationMs::millis),
        budget.memory_bytes.value().copied().map(ByteCount::bytes),
        budget.states.value().copied(),
        budget.solver_ms.value().copied().map(DurationMs::millis),
        budget.proof_ms.value().copied().map(DurationMs::millis),
        budget.tokens.value().copied(),
        budget.candidates.value().copied(),
        budget.bytes.value().copied().map(ByteCount::bytes),
    ];
    DIMENSIONS
        .iter()
        .zip(values)
        .map(|(name, value)| (format!("{prefix}.{name}"), number_or_none(value)))
        .collect()
}

/// A [`Budget`]'s nine dimensions as a JSON object under `prefix`'s keys, `null` where
/// absent.
#[must_use]
pub fn budget_json(budget: &Budget) -> Json {
    let values = [
        budget.wall_ms.value().copied().map(DurationMs::millis),
        budget.cpu_ms.value().copied().map(DurationMs::millis),
        budget.memory_bytes.value().copied().map(ByteCount::bytes),
        budget.states.value().copied(),
        budget.solver_ms.value().copied().map(DurationMs::millis),
        budget.proof_ms.value().copied().map(DurationMs::millis),
        budget.tokens.value().copied(),
        budget.candidates.value().copied(),
        budget.bytes.value().copied().map(ByteCount::bytes),
    ];
    Json::object(
        DIMENSIONS
            .iter()
            .zip(values)
            .map(|(name, value)| ((*name).to_owned(), value.map_or(Json::Null, Json::Integer))),
    )
    .expect("the nine dimension names are distinct")
}

/// A reported [`Cost`]'s nine dimensions as `key  value` lines under `prefix`, plus
/// `{prefix}.tokenizer_id` — required exactly when `tokens` is reported.
#[must_use]
pub fn cost_lines(prefix: &str, cost: &Cost) -> Lines {
    let values = [
        cost.wall_ms.value().copied().map(DurationMs::millis),
        cost.cpu_ms.value().copied().map(DurationMs::millis),
        cost.memory_bytes.value().copied().map(ByteCount::bytes),
        cost.states.value().copied(),
        cost.solver_ms.value().copied().map(DurationMs::millis),
        cost.proof_ms.value().copied().map(DurationMs::millis),
        cost.tokens.value().copied(),
        cost.candidates.value().copied(),
        cost.bytes.value().copied().map(ByteCount::bytes),
    ];
    let mut lines: Lines = DIMENSIONS
        .iter()
        .zip(values)
        .map(|(name, value)| (format!("{prefix}.{name}"), number_or_none(value)))
        .collect();
    lines.push((
        format!("{prefix}.tokenizer_id"),
        string_or_none(cost.tokenizer_id.value().map(String::as_str)),
    ));
    lines
}

/// A reported [`Cost`]'s nine dimensions as a JSON object, plus `tokenizer_id`.
#[must_use]
pub fn cost_json(cost: &Cost) -> Json {
    let values = [
        cost.wall_ms.value().copied().map(DurationMs::millis),
        cost.cpu_ms.value().copied().map(DurationMs::millis),
        cost.memory_bytes.value().copied().map(ByteCount::bytes),
        cost.states.value().copied(),
        cost.solver_ms.value().copied().map(DurationMs::millis),
        cost.proof_ms.value().copied().map(DurationMs::millis),
        cost.tokens.value().copied(),
        cost.candidates.value().copied(),
        cost.bytes.value().copied().map(ByteCount::bytes),
    ];
    let mut fields: Vec<(String, Json)> = DIMENSIONS
        .iter()
        .zip(values)
        .map(|(name, value)| ((*name).to_owned(), value.map_or(Json::Null, Json::Integer)))
        .collect();
    fields.push((
        "tokenizer_id".to_owned(),
        match cost.tokenizer_id.value() {
            Some(id) => Json::String(id.clone()),
            None => Json::Null,
        },
    ));
    Json::object(fields).expect("the ten field names are distinct")
}

// --- the typed verdict, carried rather than re-derived ------------------------------------

/// One [`Verdict`], flattened into the seven keys every variant renders under.
///
/// Seven keys on every arm — including the four that only one variant can fill — because a
/// caller reading a verdict must not have to know which union member arrived before it can
/// parse the answer. The *kind* says which member it is; the others say `none` where that
/// member declares no such field, which is a different statement from "the daemon left it
/// out" and is exactly the distinction [`Verdict`]'s union tagging already makes on the wire.
///
/// Nothing here is computed from anything else. `inconclusive_reason` in particular is read
/// straight off `SemanticVerdictValue`/`EvaluationVerdictValue`, never inferred from the
/// verdict member or from a budget: INV-008 requires the reason to be *typed and carried*,
/// and a client that reconstructed "it must be `ResourceExhausted` because the budget was
/// small" would be manufacturing the one fact the rule exists to make the engine state.
struct VerdictFacts {
    kind: &'static str,
    verdict: Option<String>,
    inconclusive_reason: Option<String>,
    assurance_class: Option<String>,
    decision: Option<String>,
    outcome: Option<String>,
    gates: Option<u64>,
}

impl VerdictFacts {
    /// Read one answer's verdict field. `None` is the IDL's own `verdict: null`.
    fn of(verdict: Option<&Verdict>) -> Self {
        let empty = Self {
            kind: "none",
            verdict: None,
            inconclusive_reason: None,
            assurance_class: None,
            decision: None,
            outcome: None,
            gates: None,
        };
        match verdict {
            None => empty,
            Some(Verdict::Semantic(value)) => Self {
                kind: "semantic",
                verdict: Some(value.verdict.as_wire().to_owned()),
                inconclusive_reason: value
                    .inconclusive_reason
                    .value()
                    .map(|reason| reason.as_wire().to_owned()),
                assurance_class: Some(value.assurance_class.as_wire().to_owned()),
                ..empty
            },
            Some(Verdict::Evaluation(value)) => Self {
                kind: "evaluation",
                verdict: Some(value.verdict.as_wire().to_owned()),
                inconclusive_reason: value
                    .inconclusive_reason
                    .value()
                    .map(|reason| reason.as_wire().to_owned()),
                assurance_class: Some(value.assurance_class.as_wire().to_owned()),
                ..empty
            },
            Some(Verdict::Policy(value)) => Self {
                kind: "policy",
                decision: Some(value.decision.as_wire().to_owned()),
                gates: Some(value.gates.len() as u64),
                ..empty
            },
            Some(Verdict::Structural(value)) => Self {
                kind: "structural",
                outcome: Some(value.outcome.as_wire().to_owned()),
                ..empty
            },
        }
    }
}

/// A result's typed verdict as `key  value` lines under `prefix`.
///
/// Total over the four union members and over `verdict: null`; every key present on every
/// arm, present or explicitly `none`.
///
/// The union tag is `{prefix}.kind` and not the bare `{prefix}` it was before bn-ybh1z: in
/// the machine document the verdict is an *object* whose tag is the field `kind`, and the
/// bare prefix therefore reads that object's field count under [`crate::contract`]'s one
/// rule. Naming the tag by its own key is what lets the same text be re-rendered from the
/// document with no per-command exception.
#[must_use]
pub fn verdict_lines(prefix: &str, verdict: Option<&Verdict>) -> Lines {
    let facts = VerdictFacts::of(verdict);
    vec![
        (format!("{prefix}.kind"), facts.kind.to_owned()),
        (
            format!("{prefix}.verdict"),
            string_or_none(facts.verdict.as_deref()),
        ),
        (
            format!("{prefix}.inconclusive_reason"),
            string_or_none(facts.inconclusive_reason.as_deref()),
        ),
        (
            format!("{prefix}.assurance_class"),
            string_or_none(facts.assurance_class.as_deref()),
        ),
        (
            format!("{prefix}.decision"),
            string_or_none(facts.decision.as_deref()),
        ),
        (
            format!("{prefix}.outcome"),
            string_or_none(facts.outcome.as_deref()),
        ),
        (format!("{prefix}.gates"), number_or_none(facts.gates)),
    ]
}

/// A result's typed verdict as a JSON object — the same seven fields [`verdict_lines`]
/// renders, `null` where the arriving member declares no such field.
#[must_use]
pub fn verdict_json(verdict: Option<&Verdict>) -> Json {
    let facts = VerdictFacts::of(verdict);
    let text = |value: Option<String>| value.map_or(Json::Null, Json::String);
    Json::object([
        ("kind".to_owned(), Json::String(facts.kind.to_owned())),
        ("verdict".to_owned(), text(facts.verdict)),
        (
            "inconclusive_reason".to_owned(),
            text(facts.inconclusive_reason),
        ),
        ("assurance_class".to_owned(), text(facts.assurance_class)),
        ("decision".to_owned(), text(facts.decision)),
        ("outcome".to_owned(), text(facts.outcome)),
        (
            "gates".to_owned(),
            facts.gates.map_or(Json::Null, Json::Integer),
        ),
    ])
    .expect("seven distinct literal keys never collide")
}

// --- the nine-dimension assurance envelope ------------------------------------------------

/// The nine dimension names, in the order `AssuranceEnvelope` declares them (plan B11;
/// SD-12 holds this list identical to `schemas/assurance-result.schema.json`'s).
const ASSURANCE_DIMENSIONS: [&str; 9] = [
    "bounds",
    "faults",
    "fairness",
    "values",
    "schedules",
    "memory_model",
    "observer",
    "proof_status",
    "unknowns",
];

/// The nine dimensions of one envelope, in [`ASSURANCE_DIMENSIONS`] order.
fn dimensions(assurance: &AssuranceEnvelope) -> [&EnvelopeDimension; 9] {
    [
        &assurance.bounds,
        &assurance.faults,
        &assurance.fairness,
        &assurance.values,
        &assurance.schedules,
        &assurance.memory_model,
        &assurance.observer,
        &assurance.proof_status,
        &assurance.unknowns,
    ]
}

/// One dimension's four facts: which union member it is, and the fields that member carries.
fn dimension_facts(
    dimension: &EnvelopeDimension,
) -> (&'static str, Option<&str>, Option<&str>, Option<&str>) {
    match dimension {
        EnvelopeDimension::Produced(produced) => (
            "produced",
            Some(&produced.engine),
            Some(&produced.summary),
            None,
        ),
        EnvelopeDimension::Unsupported(unsupported) => {
            ("unsupported", None, None, Some(&unsupported.reason))
        }
    }
}

/// The assurance envelope as `key  value` lines under `prefix`: a dimension count, then each
/// dimension's kind, producing engine, summary, and typed unsupported reason.
///
/// Four keys per dimension on both union arms, on purpose — the same reasoning
/// [`crate::evidence`]'s `Redacted` stub is rendered by. "Every dimension MUST name a
/// producing engine or carry a typed `Unsupported(reason)` […] Hiding uncertainty to save
/// tokens is prohibited" (`rule envelope.assurance_required`), so this rendering is long
/// because the rule is: a compressed projection that dropped the six unsupported dimensions
/// would be the exact omission the rule names.
#[must_use]
pub fn assurance_lines(prefix: &str, assurance: Option<&AssuranceEnvelope>) -> Lines {
    let Some(assurance) = assurance else {
        return vec![(prefix.to_owned(), "none".to_owned())];
    };
    let mut lines = vec![(prefix.to_owned(), ASSURANCE_DIMENSIONS.len().to_string())];
    for (name, dimension) in ASSURANCE_DIMENSIONS.iter().zip(dimensions(assurance)) {
        let (kind, engine, summary, reason) = dimension_facts(dimension);
        lines.push((format!("{prefix}.{name}.kind"), kind.to_owned()));
        lines.push((format!("{prefix}.{name}.engine"), string_or_none(engine)));
        lines.push((format!("{prefix}.{name}.summary"), string_or_none(summary)));
        lines.push((format!("{prefix}.{name}.reason"), string_or_none(reason)));
    }
    lines
}

/// The assurance envelope as a JSON object of nine dimension objects, or [`Json::Null`] when
/// the answer carried none.
#[must_use]
pub fn assurance_json(assurance: Option<&AssuranceEnvelope>) -> Json {
    let Some(assurance) = assurance else {
        return Json::Null;
    };
    let text = |value: Option<&str>| value.map_or(Json::Null, |text| Json::String(text.to_owned()));
    Json::object(
        ASSURANCE_DIMENSIONS
            .iter()
            .zip(dimensions(assurance))
            .map(|(name, dimension)| {
                let (kind, engine, summary, reason) = dimension_facts(dimension);
                (
                    (*name).to_owned(),
                    Json::object([
                        ("kind".to_owned(), Json::String(kind.to_owned())),
                        ("engine".to_owned(), text(engine)),
                        ("summary".to_owned(), text(summary)),
                        ("reason".to_owned(), text(reason)),
                    ])
                    .expect("four distinct literal keys never collide"),
                )
            }),
    )
    .expect("the nine dimension names are distinct")
}

/// A list of handles as `{prefix}  N` followed by `{prefix}[i]  handle`, unabridged.
///
/// The same count-then-detail shape [`omission_lines`] uses, for the same reason: a count
/// alone tells a caller that something exists and not what, and a truncation would be a
/// projection inventing a boundary the answer does not have.
#[must_use]
pub fn handle_lines<'a>(prefix: &str, handles: impl IntoIterator<Item = &'a str>) -> Lines {
    let handles: Vec<&str> = handles.into_iter().collect();
    let mut lines = vec![(prefix.to_owned(), handles.len().to_string())];
    for (index, handle) in handles.iter().enumerate() {
        lines.push((format!("{prefix}[{index}]"), (*handle).to_owned()));
    }
    lines
}

/// A list of handles as a JSON array of strings.
#[must_use]
pub fn handle_json<'a>(handles: impl IntoIterator<Item = &'a str>) -> Json {
    Json::Array(
        handles
            .into_iter()
            .map(|handle| Json::String(handle.to_owned()))
            .collect(),
    )
}

/// The count of allowed next operations, rendered the same way on every answer regardless
/// of arm.
#[must_use]
pub fn next_operations_line(next_operations: &[NextOperation]) -> (String, String) {
    (
        "next_operations".to_owned(),
        next_operations.len().to_string(),
    )
}

/// The artifacts one answer published or names, as a JSON array — one object each, with the
/// typed [`Redacted`] stub carried rather than flattened.
///
/// Unabridged and by name: "a redacted value is reported, never silently dropped", and an
/// artifact list summarized to a count would make a caller ask again for the handles the
/// answer already carried. Machine-channel only — see [`project`] for why.
#[must_use]
pub fn artifacts_json(artifacts: &[ArtifactRef]) -> Json {
    Json::Array(
        artifacts
            .iter()
            .map(|artifact| {
                Json::object([
                    ("kind".to_owned(), Json::String(artifact.kind.clone())),
                    (
                        "handle".to_owned(),
                        Json::String(artifact.handle.as_str().to_owned()),
                    ),
                    (
                        "commitment".to_owned(),
                        match artifact.commitment.value() {
                            Some(commitment) => Json::String(commitment.as_str().to_owned()),
                            None => Json::Null,
                        },
                    ),
                    (
                        "redacted".to_owned(),
                        match artifact.redacted.value() {
                            Some(stub) => redacted_json(stub),
                            None => Json::Null,
                        },
                    ),
                ])
                .expect("four distinct literal keys never collide")
            })
            .collect(),
    )
}

/// One typed [`Redacted`] stub as a JSON object — the four fields RFC 0026 declares, so that
/// "withheld" stays distinguishable from "absent" structurally wherever a stub can ride.
#[must_use]
pub fn redacted_json(stub: &Redacted) -> Json {
    Json::object([
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
    .expect("four distinct literal keys never collide")
}

/// The allowed next operations as a JSON array — one object each, with the pre-filled
/// arguments embedded verbatim and the typed rationale beside them.
///
/// The list, not the count [`next_operations_line`] prints. `next_operations` is the
/// protocol's own "what may I do from here", and plan §3.1's five-minute path *is* that list
/// rendered ("`debug  continuum debug crash_7m3…`"); a machine channel that carried only a
/// length would make a caller ask again for the thing the answer already contained.
/// bn-ybh1z added it: before that this key existed in the text formats only, which is the
/// prose-only interface INV-003 forbids, in its smallest form.
#[must_use]
pub fn next_operations_json(next_operations: &[NextOperation]) -> Json {
    Json::Array(
        next_operations
            .iter()
            .map(|next| {
                Json::object([
                    (
                        "operation".to_owned(),
                        Json::String(next.operation.as_str().to_owned()),
                    ),
                    ("arguments".to_owned(), embedded(Some(&next.arguments))),
                    (
                        "arguments_bytes".to_owned(),
                        embedded_bytes(Some(&next.arguments)),
                    ),
                    (
                        "rationale".to_owned(),
                        match next.rationale.value() {
                            Some(rationale) => Json::String(rationale.clone()),
                            None => Json::Null,
                        },
                    ),
                ])
                .expect("four distinct literal keys never collide")
            })
            .collect(),
    )
}

/// Wrap a JSON envelope's named record beside the fields every machine answer in this crate
/// carries: `advice` (per convention, always present, empty when there is none) is appended
/// automatically.
#[must_use]
pub fn envelope_json(mut fields: Vec<(String, Json)>) -> Json {
    fields.push(("advice".to_owned(), Json::Array(Vec::new())));
    Json::object(fields).expect("callers pass distinct keys")
}

/// The canonical UTF-8 rendering of a JSON document, newline-terminated for a terminal.
#[must_use]
pub fn json_text(document: &Json) -> String {
    let mut text = String::from_utf8(document.to_canonical_bytes())
        .expect("canonical JSON is UTF-8 by construction");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use continuumd::protocol::envelope::Omission;
    use continuumd::protocol::scalar::ArtifactHandle;
    use continuumd::protocol::spec::Optional;
    use continuumd::protocol::vocabulary::OmissionReason;

    use super::*;

    fn omission(subject: &str, recoverable: Option<&str>) -> Omission {
        Omission {
            reason: OmissionReason::Unsupported,
            subject: subject.to_owned(),
            recoverable_by: match recoverable {
                Some(handle) => Optional::Present(
                    ArtifactHandle::new(handle).expect("a well-formed artifact handle"),
                ),
                None => Optional::Absent,
            },
        }
    }

    #[test]
    fn an_empty_manifest_still_renders_its_zero_count() {
        let lines = omission_lines(&[]);
        assert_eq!(lines, vec![("omissions".to_owned(), "0".to_owned())]);
        assert_eq!(omissions_json(&[]), Json::Array(Vec::new()));
    }

    #[test]
    fn every_omission_is_named_reason_subject_and_recoverable_by() {
        let omissions = vec![
            omission("task.milestones", None),
            omission("budget.wall_ms", Some("ev_abc123")),
        ];
        let lines = omission_lines(&omissions);
        assert_eq!(lines[0], ("omissions".to_owned(), "2".to_owned()));
        assert!(lines.contains(&("omissions[0].reason".to_owned(), "unsupported".to_owned())));
        assert!(lines.contains(&(
            "omissions[0].subject".to_owned(),
            "task.milestones".to_owned()
        )));
        assert!(lines.contains(&("omissions[0].recoverable_by".to_owned(), "none".to_owned())));
        assert!(lines.contains(&(
            "omissions[1].recoverable_by".to_owned(),
            "ev_abc123".to_owned()
        )));

        let Json::Array(items) = omissions_json(&omissions) else {
            panic!("omissions_json always answers an array");
        };
        assert_eq!(items.len(), 2);

        // Every text key is a path into the machine document, so the manifest's own text
        // re-renders from its JSON with nothing left over (bn-ybh1z, INV-003).
        let document = Json::object([("omissions".to_owned(), omissions_json(&omissions))])
            .expect("one key never collides");
        assert_eq!(
            crate::contract::divergence(&join_lines(&lines), &document),
            None
        );
    }

    #[test]
    fn join_lines_uses_a_two_space_delimiter() {
        let lines = vec![("task".to_owned(), "task_abc123".to_owned())];
        assert_eq!(join_lines(&lines), "task  task_abc123\n");
    }

    #[test]
    fn only_the_unsupported_code_reads_as_an_unsupported_surface() {
        // The one code `rule errors.unsupported_surface` names maps to `Unsupported`;
        // every other typed refusal is a `no` about the request, not about the build.
        assert_eq!(
            Depth::of_code(ErrorCode::UnsupportedSemanticFeature),
            Depth::Unsupported
        );
        for other in [
            ErrorCode::CapabilityDenied,
            ErrorCode::MalformedRequest,
            ErrorCode::StaleSnapshot,
            ErrorCode::BudgetExhausted,
            ErrorCode::InsufficientEvidence,
        ] {
            assert_eq!(Depth::of_code(other), Depth::Refused, "{other:?}");
        }
    }

    /// The exit code is [`crate::contract::Exit`]'s, and this type no longer owns one —
    /// bn-ybh1z. What did not change is that both refusal arms earn `1`: the distinction
    /// between them is [`Depth::token`], in the output, where a caller reads it without
    /// inferring anything from a number.
    #[test]
    fn both_refusal_depths_still_earn_the_same_declined_code() {
        use crate::contract::Exit;

        assert_eq!(
            Exit::of_code(ErrorCode::UnsupportedSemanticFeature).code(),
            1
        );
        assert_eq!(Exit::of_code(ErrorCode::CapabilityDenied).code(), 1);
        assert_eq!(Depth::Unsupported.token(), "unsupported");
        assert_eq!(Depth::Refused.token(), "refused");
    }

    #[test]
    fn the_depth_tokens_are_three_distinct_stable_strings() {
        let tokens = [
            Depth::Served.token(),
            Depth::Unsupported.token(),
            Depth::Refused.token(),
        ];
        assert_eq!(tokens, ["served", "unsupported", "refused"]);
    }

    #[test]
    fn an_opaque_field_reports_its_document_and_its_length_together() {
        let opaque = Opaque::from_bytes(br#"{"a":1}"#.to_vec());
        assert_eq!(embedded_bytes(Some(&opaque)), Json::Integer(7));
        assert_eq!(embedded_bytes_line(Some(&opaque)), "7");
        let Json::Object(fields) = embedded(Some(&opaque)) else {
            panic!("a canonical JSON object embeds as one");
        };
        assert_eq!(fields.len(), 1);

        // Absent reads `null`/`none` in both channels, so the pair `(null, null)` is the
        // only spelling of "the field was not there".
        assert_eq!(embedded(None), Json::Null);
        assert_eq!(embedded_bytes(None), Json::Null);
        assert_eq!(embedded_bytes_line(None), "none");

        // A present field that does not parse reads `null` in the document channel and a
        // length in the other — visibly different from an absent one.
        let broken = Opaque::from_bytes(b"not a document".to_vec());
        assert_eq!(embedded(Some(&broken)), Json::Null);
        assert_eq!(embedded_bytes(Some(&broken)), Json::Integer(14));
    }

    #[test]
    fn a_verdict_renders_the_same_seven_keys_whichever_union_member_arrived() {
        use continuumd::protocol::envelope::{
            PolicyVerdictValue, SemanticVerdictValue, StructuralVerdictValue,
        };
        use continuumd::protocol::vocabulary::{
            AssuranceClass, InconclusiveReason, PolicyDecision, SemanticVerdict, StructuralOutcome,
        };

        let semantic = Verdict::Semantic(SemanticVerdictValue {
            verdict: SemanticVerdict::Inconclusive,
            inconclusive_reason: Optional::Present(InconclusiveReason::ResourceExhausted),
            assurance_class: AssuranceClass::Bounded,
        });
        let policy = Verdict::Policy(PolicyVerdictValue {
            decision: PolicyDecision::Review,
            gates: Vec::new(),
        });
        let structural = Verdict::Structural(StructuralVerdictValue {
            outcome: StructuralOutcome::Sealed,
        });

        // One key set on every arm, so a parser never branches on which member arrived.
        let keys = |verdict: Option<&Verdict>| -> Vec<String> {
            verdict_lines("verdict", verdict)
                .into_iter()
                .map(|(key, _)| key)
                .collect()
        };
        let expected = keys(None);
        assert_eq!(expected.len(), 7);
        for member in [&semantic, &policy, &structural] {
            assert_eq!(keys(Some(member)), expected);
        }

        // INV-008's typed reason survives verbatim, in the protocol's own spelling.
        let lines = verdict_lines("verdict", Some(&semantic));
        assert!(lines.contains(&("verdict.kind".to_owned(), "semantic".to_owned())));
        assert!(lines.contains(&(
            "verdict.inconclusive_reason".to_owned(),
            InconclusiveReason::ResourceExhausted.as_wire().to_owned()
        )));
        assert!(lines.contains(&(
            "verdict.assurance_class".to_owned(),
            AssuranceClass::Bounded.as_wire().to_owned()
        )));
        // …and the fields that member does not declare read `none`, not a guess.
        assert!(lines.contains(&("verdict.decision".to_owned(), "none".to_owned())));
        assert!(lines.contains(&("verdict.outcome".to_owned(), "none".to_owned())));

        // A decided verdict carries no reason, and says so.
        let decided = Verdict::Semantic(SemanticVerdictValue {
            verdict: SemanticVerdict::Refuted,
            inconclusive_reason: Optional::Absent,
            assurance_class: AssuranceClass::Validated,
        });
        assert!(
            verdict_lines("verdict", Some(&decided))
                .contains(&("verdict.inconclusive_reason".to_owned(), "none".to_owned()))
        );

        // The machine channel carries the same seven, `null` where the member has none.
        let Json::Object(fields) = verdict_json(Some(&policy)) else {
            panic!("a verdict renders as an object");
        };
        assert_eq!(fields.len(), 7);
        assert_eq!(
            fields.get("decision"),
            Some(&Json::String("review".to_owned()))
        );
        assert_eq!(fields.get("verdict"), Some(&Json::Null));
        assert_eq!(fields.get("gates"), Some(&Json::Integer(0)));

        // …and the two channels cannot disagree: the seven text keys are paths into the
        // seven-field object, `verdict.kind` included (bn-ybh1z).
        for member in [None, Some(&semantic), Some(&policy), Some(&structural)] {
            let document = Json::object([("verdict".to_owned(), verdict_json(member))])
                .expect("one key never collides");
            assert_eq!(
                crate::contract::divergence(
                    &join_lines(&verdict_lines("verdict", member)),
                    &document
                ),
                None,
                "{member:?}"
            );
        }

        // `verdict: null` is the IDL's own answer for an operation with no verdict clause.
        assert_eq!(
            verdict_json(None),
            Json::object([
                ("kind".to_owned(), Json::String("none".to_owned())),
                ("verdict".to_owned(), Json::Null),
                ("inconclusive_reason".to_owned(), Json::Null),
                ("assurance_class".to_owned(), Json::Null),
                ("decision".to_owned(), Json::Null),
                ("outcome".to_owned(), Json::Null),
                ("gates".to_owned(), Json::Null),
            ])
            .expect("seven distinct keys")
        );
    }

    #[test]
    fn every_assurance_dimension_renders_four_keys_on_both_union_arms() {
        use continuumd::protocol::envelope::{
            AssuranceEnvelope, EnvelopeDimension, ProducedDimension, UnsupportedDimension,
        };

        let produced = || {
            EnvelopeDimension::Produced(ProducedDimension {
                engine: "continuum-engine-reference".to_owned(),
                summary: "exhaustive-finite".to_owned(),
            })
        };
        let unsupported = || {
            EnvelopeDimension::Unsupported(UnsupportedDimension {
                reason: "no-fault-model".to_owned(),
            })
        };
        let envelope = AssuranceEnvelope {
            bounds: produced(),
            faults: unsupported(),
            fairness: unsupported(),
            values: produced(),
            schedules: unsupported(),
            memory_model: unsupported(),
            observer: unsupported(),
            proof_status: unsupported(),
            unknowns: produced(),
        };

        let lines = assurance_lines("assurance", Some(&envelope));
        // One count line plus four per dimension: "hiding uncertainty to save tokens is
        // prohibited", so the six unsupported dimensions cost as many lines as the three
        // produced ones.
        assert_eq!(lines.len(), 1 + 4 * 9);
        assert_eq!(lines[0], ("assurance".to_owned(), "9".to_owned()));
        assert!(lines.contains(&(
            "assurance.bounds.engine".to_owned(),
            "continuum-engine-reference".to_owned()
        )));
        assert!(lines.contains(&("assurance.bounds.reason".to_owned(), "none".to_owned())));
        assert!(lines.contains(&(
            "assurance.faults.reason".to_owned(),
            "no-fault-model".to_owned()
        )));
        assert!(lines.contains(&("assurance.faults.engine".to_owned(), "none".to_owned())));

        // An answer with no envelope says so in one line, and reads `null` in JSON.
        assert_eq!(
            assurance_lines("assurance", None),
            vec![("assurance".to_owned(), "none".to_owned())]
        );
        assert_eq!(assurance_json(None), Json::Null);

        // The count line is the object's own field count and the `none` line is its `null`,
        // so both arms re-render from the machine document with no exception (bn-ybh1z).
        for member in [None, Some(&envelope)] {
            let document = Json::object([("assurance".to_owned(), assurance_json(member))])
                .expect("one key never collides");
            assert_eq!(
                crate::contract::divergence(
                    &join_lines(&assurance_lines("assurance", member)),
                    &document
                ),
                None
            );
        }

        let Json::Object(dimensions) = assurance_json(Some(&envelope)) else {
            panic!("an envelope renders as an object");
        };
        assert_eq!(dimensions.len(), 9);
        for dimension in dimensions.values() {
            let Json::Object(fields) = dimension else {
                panic!("each dimension is an object");
            };
            assert_eq!(fields.len(), 4, "one uniform key set on both arms");
        }
    }

    #[test]
    fn a_handle_list_renders_its_count_and_then_every_member() {
        let lines = handle_lines("evidence", ["ev_one", "ev_two"]);
        assert_eq!(
            lines,
            vec![
                ("evidence".to_owned(), "2".to_owned()),
                ("evidence[0]".to_owned(), "ev_one".to_owned()),
                ("evidence[1]".to_owned(), "ev_two".to_owned()),
            ]
        );
        assert_eq!(
            handle_lines("evidence", Vec::<&str>::new()),
            vec![("evidence".to_owned(), "0".to_owned())]
        );
        assert_eq!(
            handle_json(["ev_one"]),
            Json::Array(vec![Json::String("ev_one".to_owned())])
        );
    }

    #[test]
    fn the_two_depth_channels_carry_the_same_two_facts() {
        let lines = depth_lines("debug.open", Depth::Unsupported);
        assert_eq!(
            lines,
            vec![
                ("operation".to_owned(), "debug.open".to_owned()),
                ("depth".to_owned(), "unsupported".to_owned()),
            ]
        );
        let fields = depth_json("debug.open", Depth::Unsupported);
        assert_eq!(
            fields,
            vec![
                (
                    "operation".to_owned(),
                    Json::String("debug.open".to_owned())
                ),
                ("depth".to_owned(), Json::String("unsupported".to_owned())),
            ]
        );
    }
}
