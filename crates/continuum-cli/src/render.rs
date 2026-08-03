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
use continuumd::protocol::envelope::{Budget, Cost, NextOperation, Omission};
use continuumd::protocol::scalar::{ByteCount, DurationMs};
use continuumd::protocol::spec::ProtocolEnum;

use crate::wire::Refusal;

/// One rendered `key  value` pair, in emission order.
pub type Lines = Vec<(String, String)>;

/// A command's finished output: the text to write to stdout, and the exit code
/// (cli-conventions.md, "Exit Codes") the daemon's answer earns.
///
/// `0` for [`crate::wire::Outcome::Admitted`], `1` for
/// [`crate::wire::Outcome::Refused`] — a well-formed request the daemon typedly declined is
/// closer to "the operation did not succeed" than to either bucket the convention names for
/// a *failure to ask*, and `1` (user error) is the nearer of the two: `2` is reserved for a
/// connection that failed below the protocol (see [`crate::error::CliError::exit_code`]),
/// which is a different failure than a daemon that answered.
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

// --- the omission manifest, unabridged --------------------------------------------------

/// The full INV-007 manifest as `key  value` lines: a count, then every omission's
/// `reason`/`subject`/`recoverable_by`, indexed.
#[must_use]
pub fn omission_lines(omissions: &[Omission]) -> Lines {
    let mut lines = vec![("omissions".to_owned(), omissions.len().to_string())];
    for (index, omission) in omissions.iter().enumerate() {
        lines.push((
            format!("omission[{index}].reason"),
            omission.reason.as_wire().to_owned(),
        ));
        lines.push((
            format!("omission[{index}].subject"),
            omission.subject.clone(),
        ));
        lines.push((
            format!("omission[{index}].recoverable_by"),
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
            Json::Integer(refusal.recovery.len() as u64),
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

/// The count of allowed next operations, rendered the same way on every answer regardless
/// of arm.
#[must_use]
pub fn next_operations_line(next_operations: &[NextOperation]) -> (String, String) {
    (
        "next_operations".to_owned(),
        next_operations.len().to_string(),
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
        assert!(lines.contains(&("omission[0].reason".to_owned(), "unsupported".to_owned())));
        assert!(lines.contains(&(
            "omission[0].subject".to_owned(),
            "task.milestones".to_owned()
        )));
        assert!(lines.contains(&("omission[0].recoverable_by".to_owned(), "none".to_owned())));
        assert!(lines.contains(&(
            "omission[1].recoverable_by".to_owned(),
            "ev_abc123".to_owned()
        )));

        let Json::Array(items) = omissions_json(&omissions) else {
            panic!("omissions_json always answers an array");
        };
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn join_lines_uses_a_two_space_delimiter() {
        let lines = vec![("task".to_owned(), "task_abc123".to_owned())];
        assert_eq!(join_lines(&lines), "task  task_abc123\n");
    }
}
