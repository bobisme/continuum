//! **The output contract.** One machine document per invocation, one pure projection of it
//! for a terminal, one exit-code taxonomy, and no colour anywhere.
//!
//! This module is the cross-cutting half of PR 13 (`START_HERE_IMPLEMENTATION.md`: "golden
//! tests pin JSON, exit codes, and concise terminal output"), owned once here rather than
//! re-decided per command group. Plan §13.4 states the obligation in one sentence — "Exit
//! codes and JSON schemas are documented. Color is never semantically required." — and plan
//! §3.1's five-minute path is the shape it has to hold for: a concise block of `key  value`
//! lines a human reads and an agent parses, with the follow-up commands beside it.
//!
//! # The constitutional stake
//!
//! > No prose-only machine interfaces.
//! >
//! > — INV-003; plan §3.1
//!
//! INV-003 is usually read as "there is a `--json`". That is the weak reading. The strong one
//! — the one this module makes mechanical — is that **prose is a projection of the machine
//! result and never a second truth**: there must be no fact in the terminal output that is
//! not in the machine document, and no rendering of a shared fact that the two channels can
//! disagree about. A CLI whose text output carried one more number than its JSON would be a
//! prose-only interface for exactly that number.
//!
//! # 1. The machine document is the daemon's answer, verbatim
//!
//! `--format json` emits one canonical JSON object per invocation. Its fields are:
//!
//! | Field | Source |
//! |---|---|
//! | `operation` | the registry name the command put in the envelope |
//! | `depth` | [`crate::render::Depth`], derived from the daemon's own [`ErrorCode`] |
//! | *request fields* | the typed request this invocation built, echoed |
//! | *response fields* | the daemon's response body, its verdict, and its assurance envelope |
//! | `request_id` | the identifier the daemon echoed, on both arms |
//! | `cost` | the nine-dimension actual spend, plus its tokenizer identity |
//! | `artifacts` | the `ArtifactRef`s this call published or names, unabridged |
//! | `error` | the daemon's typed [`Refusal`](crate::wire::Refusal), or `null` |
//! | `omissions` | the INV-007 manifest, unabridged, always present |
//! | `next_operations` | the protocol's allowed next steps, unabridged |
//! | `advice` | the conventions doc's envelope key, always present |
//!
//! The last seven are [`RESERVED_KEYS`]. `request_id`, `cost` and `artifacts` are
//! machine-channel only — see [`crate::render::project`] for why that is the contract's own
//! direction and not a convenience.
//!
//! Three rules govern what may appear:
//!
//! - **No dropped fields.** A field the answer carries and this crate does not render is a
//!   fact the daemon stated and the adapter hid. Lists are carried as lists in the machine
//!   channel — `next_operations`, `error.recovery`, `milestones` — because "machine output
//!   has no terminal to overflow".
//! - **Echoed subjects appear once.** A response field that repeats a request field of the
//!   same name — `repair.review`'s `repair`, `task.cancel`'s `task` — is carried under the
//!   request's key. Printing it twice would give one fact two keys and leave a parser to
//!   decide which to believe if they ever differed; this is a naming rule, not an omission,
//!   and every command it applies to says so at its call site.
//! - **No invented fields.** Every key is a request field, a response field, or a *derived
//!   token*. A derived token is admissible only when it is a **total, documented function of
//!   the answer that adds no fact the answer does not already carry**: `depth`
//!   ([`crate::render::Depth`]), `answer_shape` ([`crate::check::AnswerShape`]) and
//!   `outcome` ([`crate::task::CancelOutcome`]) are the three, and each names in its own doc
//!   what it is read off. ([`Exit`] is derived the same way and is not a key: it is the exit
//!   code.) A token that merely restates another token is not admissible, which is why
//!   `context expand`'s former `status: ok|refused` and `task resume`'s former
//!   `refused: bool` were removed by bn-ybh1z: `depth` already says it, typed and finer.
//!
//! # 2. The terminal rendering is a pure function of that document
//!
//! [`project_text`] is that function. Given the machine document and an ordered list of keys,
//! it computes the whole `text`/`pretty` body — every value in it — with no access to the
//! answer, the request, or anything else. The correspondence has **one rule and no
//! exceptions**:
//!
//! - a key is a path: `a.b`, `a[0]`, `a.b[2].c`;
//! - traversal into anything that is not the matching container yields `null`, so a path
//!   through an absent or `null` field is `null` rather than an error;
//! - and the value at a path renders as: `null` → `none`, a bool → `true`/`false`, an integer
//!   → its decimal spelling, a string → itself, an **array → its length**, an **object → its
//!   field count**.
//!
//! [`divergence`] is the mechanical check: it recovers the key order from a rendered text
//! block, re-renders it from the JSON document alone, and compares. `tests/output_contract.rs`
//! applies it to **every command, on every arm, in every format**. Two channels that
//! disagreed about one value — or a text key naming a fact the document does not carry —
//! fail it. That is the acceptance criterion "a golden test asserts the two cannot diverge",
//! stated as strongly as this crate's [`Projection`](crate::render::Projection) design
//! supports: the renderers already compute both channels from one closure over one
//! `Outcome`, and this check proves the results of that one closure agree.
//!
//! ## Reserved keys
//!
//! [`RESERVED_KEYS`] are the contract's own. A command whose request or response declares a
//! field of the same name renders it under a disambiguated key and says so at the call site
//! — `task status`'s `TaskRecord.operation` is `record.operation` (the operation the *task*
//! runs, not the one the command drove), and `context expand`'s `ContextExpandRequest.depth`
//! is `expand_depth` (an expansion distance, not a [`crate::render::Depth`]). Shadowing
//! either would give one key two meanings across commands, and a parser that had to know
//! which command it was reading to know what `depth` meant would be back to prose.
//!
//! # 3. The exit-code taxonomy
//!
//! One number answers one question: **did I get an answer, and if so, was it yes?**
//!
//! | Code | [`Exit`] | Meaning |
//! |---|---|---|
//! | `0` | [`Exit::Success`] | The daemon answered, and no verdict it carried says no. |
//! | `1` | [`Exit::Declined`] | No answer: the request was typedly refused, or did not parse. |
//! | `2` | [`Exit::Fault`] | No answer: the connection failed below the protocol. |
//! | `3` | [`Exit::Refuted`] | An answer, and it is **no** — a decided negative verdict. |
//! | `4` | [`Exit::Inconclusive`] | An answer, and it is **unknown** — INV-008's typed inconclusiveness. |
//!
//! `0`, `1` and `2` keep exactly the meanings `.agents/edict/design/cli-conventions.md`
//! assigns them (success, user error, system error); `3` and `4` extend that table rather
//! than reinterpreting it, which is what lets an existing script that only tests `$? == 0`
//! keep working while a script that wants the distinction can have it.
//!
//! ## Why refuted and inconclusive earn codes and unsupported does not
//!
//! bn-1g7e4 exited `1` for an unsupported surface "alongside every other refusal rather than
//! claiming a fourth code the conventions doc does not define", and put the distinction in
//! the output as [`crate::render::Depth`]. **That stands**, and the line it draws is the one
//! this taxonomy generalizes: the exit code says what happened to the *question*; a typed
//! token in the output says what happened to the *request*.
//!
//! - `unsupported` and `refused` are both "this request got no answer". Which of the two it
//!   was changes what the *caller* should do, not what the *answer* is — and `depth` already
//!   carries it, losslessly, in every format. A number that duplicated a token would be a
//!   second spelling of one fact.
//! - `refuted` and `inconclusive` are not failures at all: the request succeeded and the
//!   answer is a no, or a typed unknown. **Nothing else conveys that to `$?`.** A `check`
//!   that refutes an invariant exiting `0` is the failure mode this taxonomy exists to
//!   remove: it makes `continuum check … && deploy` deploy a refuted build. That is why
//!   bn-ybh1z flipped those pins deliberately — `tests/check_verdict.rs` carries the three
//!   that moved, each naming this reason at the assertion — rather than leaving the
//!   conventions doc's three codes alone.
//!
//! A verdict is classified by [`Exit::of_verdict`], which is total over all four
//! [`Verdict`] members and over `verdict: null`, and which reads the verdict off the answer
//! — never off a cost, a budget, or an assurance class (INV-008: the reason is typed and
//! carried, never reconstructed).
//!
//! # 4. Non-color completeness (G8-06)
//!
//! > accessibility and non-color CLI/UI semantics: no critical workflow requires color or a
//! > rendered graph
//! >
//! > — release gate G8-06 (`notes/plan/docs/52_RELEASE_GATES_REV3.md`, "G8 — Human
//! > usability")
//!
//! This crate emits **no ANSI escape, no colour, no box drawing, and no graph**, in any
//! format, on any arm. That is a property of the code and not of a flag: there is no
//! `--no-color`, no `NO_COLOR` read, and no TTY-conditional styling, because there is nothing
//! to switch off. `Format::Pretty` differs from `Format::Text` by exactly one prepended
//! `command  <name>` line and by nothing else — [`crate::render::project`] is the single
//! place that adds it — so a reader who never sees the pretty format loses a label and no
//! fact. `tests/output_contract.rs` pins all three statements for every command.
//!
//! The one shape a terminal cannot hold — an unbounded nested document — is not *dropped* in
//! the text formats, it is *reported*: a `pack`/`node`/`frontier`/`state` renders its byte
//! length under `pack_bytes` and its like, beside the same document embedded verbatim in the
//! machine channel. Length and document are two keys, both present in JSON, so the text
//! remains a projection under the rule above.

use continuumd::codec::json::Json;
use continuumd::protocol::envelope::Verdict;
use continuumd::protocol::vocabulary::{
    ErrorCode, EvaluationVerdict, PolicyDecision, SemanticVerdict, StructuralOutcome,
};

use crate::wire::Outcome;

/// The keys the contract owns on every answer of every command.
///
/// A command's own request or response field of the same name is rendered under a
/// disambiguated key — see this module's doc, "Reserved keys".
pub const RESERVED_KEYS: [&str; 9] = [
    "operation",
    "depth",
    "request_id",
    "cost",
    "artifacts",
    "error",
    "omissions",
    "next_operations",
    "advice",
];

/// The two-space delimiter every `key  value` line uses (cli-conventions.md, "Text format
/// guidelines").
pub const DELIMITER: &str = "  ";

/// The literal a `null` renders as in the text formats — "every scalar is present or
/// explicitly `none`".
pub const NONE: &str = "none";

/// The value every failed path resolution lands on.
///
/// A `static` rather than a temporary because [`value_at`] answers a borrow of the document
/// and a path that leaves it has to borrow *something* that outlives the call — and because
/// "absent" and "the document said null" are deliberately one value here: `a.b` over
/// `{"a": null}` and over `{}` are both `none`, which is what makes the traversal total.
static NULL: Json = Json::Null;

// --- the exit-code taxonomy ---------------------------------------------------------------

/// What one invocation's exit code says (this module's doc, §3).
///
/// Five members and five codes, total over everything a command can finish as: the daemon
/// answered and the answer is yes, no, or a typed unknown; or there was no answer, because
/// the request was declined or because the connection failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// `0` — the daemon answered and no verdict it carried says no.
    Success,
    /// `1` — no answer: a typed refusal, or a command line that does not parse.
    Declined,
    /// `2` — no answer: the connection failed below the protocol.
    Fault,
    /// `3` — an answer, and it is a decided negative verdict.
    Refuted,
    /// `4` — an answer, and it is INV-008's typed inconclusiveness.
    Inconclusive,
}

impl Exit {
    /// The process exit code this class earns.
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::Success => 0,
            Self::Declined => 1,
            Self::Fault => 2,
            Self::Refuted => 3,
            Self::Inconclusive => 4,
        }
    }

    /// A stable, kebab-case token for the class — the same string in all three formats,
    /// so a caller that cannot see `$?` (a library, a log, a test) reads the same taxonomy.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Declined => "declined",
            Self::Fault => "fault",
            Self::Refuted => "refuted",
            Self::Inconclusive => "inconclusive",
        }
    }

    /// Every member, in code order — for a caller enumerating the taxonomy, and for the
    /// tests that hold it to five distinct codes and five distinct tokens.
    pub const ALL: [Self; 5] = [
        Self::Success,
        Self::Declined,
        Self::Fault,
        Self::Refuted,
        Self::Inconclusive,
    ];

    /// Classify one answer.
    ///
    /// A refusal is [`Exit::Declined`] whichever [`ErrorCode`] it carries — including
    /// [`ErrorCode::UnsupportedSemanticFeature`], whose distinction is
    /// [`crate::render::Depth`]'s token in the output rather than a code of its own (this
    /// module's doc, "Why refuted and inconclusive earn codes and unsupported does not").
    #[must_use]
    pub fn of<T>(outcome: &Outcome<T>) -> Self {
        match outcome {
            Outcome::Admitted(admitted) => Self::of_verdict(admitted.verdict.as_ref()),
            Outcome::Refused(_) => Self::Declined,
        }
    }

    /// Classify a typed refusal's code alone. Always [`Exit::Declined`]; a function rather
    /// than a constant so the reason lives beside the one call that could want otherwise.
    #[must_use]
    pub const fn of_code(_code: ErrorCode) -> Self {
        Self::Declined
    }

    /// Classify an admitted answer's verdict.
    ///
    /// Total over the four [`Verdict`] members and over `verdict: null`, and derived from
    /// the verdict alone — never from a cost, a budget, or an assurance class. INV-008
    /// requires the inconclusive reason to be *typed and carried*, so an exit code inferred
    /// from "the budget was small" would be manufacturing the fact the rule exists to make
    /// the engine state.
    ///
    /// The four members map by what each one's vocabulary calls a decided negative:
    ///
    /// - [`SemanticVerdict::Refuted`] and [`EvaluationVerdict::Refuted`] are the word itself.
    /// - [`EvaluationVerdict::Deadlock`] is a found defect, which is a no about the property
    ///   the campaign asked after.
    /// - [`PolicyDecision::Block`] is a gate saying no; [`PolicyDecision::Review`] is a gate
    ///   reaching no decision at all, which is the same *shape* as inconclusive — a caller
    ///   has neither a yes nor a no — and is reported as such.
    /// - [`StructuralOutcome::Rejected`] is the one negative among nine structural outcomes;
    ///   `created`, `sealed`, `cancelled` and the rest are successful statements about what
    ///   happened, not verdicts about a question.
    ///
    /// An operation with no `verdict` clause answers `verdict: null` — the IDL's own value,
    /// not an absence this crate invented — and a call that was admitted with nothing to
    /// decide is a success.
    #[must_use]
    pub fn of_verdict(verdict: Option<&Verdict>) -> Self {
        match verdict {
            None => Self::Success,
            Some(Verdict::Semantic(value)) => match value.verdict {
                SemanticVerdict::Established => Self::Success,
                SemanticVerdict::Refuted => Self::Refuted,
                SemanticVerdict::Inconclusive => Self::Inconclusive,
            },
            Some(Verdict::Evaluation(value)) => match value.verdict {
                EvaluationVerdict::Satisfied => Self::Success,
                EvaluationVerdict::Refuted | EvaluationVerdict::Deadlock => Self::Refuted,
                EvaluationVerdict::Inconclusive => Self::Inconclusive,
            },
            Some(Verdict::Policy(value)) => match value.decision {
                PolicyDecision::Allow => Self::Success,
                PolicyDecision::Block => Self::Refuted,
                PolicyDecision::Review => Self::Inconclusive,
            },
            Some(Verdict::Structural(value)) => match value.outcome {
                StructuralOutcome::Rejected => Self::Refuted,
                StructuralOutcome::Created
                | StructuralOutcome::Updated
                | StructuralOutcome::Sealed
                | StructuralOutcome::Accepted
                | StructuralOutcome::Locked
                | StructuralOutcome::Cancelled
                | StructuralOutcome::Unchanged
                | StructuralOutcome::Acknowledged => Self::Success,
            },
        }
    }
}

// --- the pure projection ------------------------------------------------------------------

/// One key of a text line, resolved against the machine document.
///
/// The path grammar and the rendering rule are this module's doc, §2 — one rule, no
/// exceptions. Traversal is total: a segment that does not match the container it lands on
/// yields [`Json::Null`], which renders [`NONE`], so `a.b.c` over `{"a": null}` is `none`
/// rather than a failure.
#[must_use]
pub fn value_at<'a>(document: &'a Json, key: &str) -> &'a Json {
    let mut cursor = document;
    for segment in key.split('.') {
        let (name, indices) = split_indices(segment);
        cursor = match cursor {
            Json::Object(fields) => fields.get(name).unwrap_or(&NULL),
            _ => &NULL,
        };
        for index in indices {
            cursor = match cursor {
                Json::Array(items) => items.get(index).unwrap_or(&NULL),
                _ => &NULL,
            };
        }
    }
    cursor
}

/// Split `name[1][2]` into its name and its indices. A malformed bracket group is part of
/// the name, which resolves to nothing and renders [`NONE`] — total, like the rest of the
/// grammar.
fn split_indices(segment: &str) -> (&str, Vec<usize>) {
    let Some(open) = segment.find('[') else {
        return (segment, Vec::new());
    };
    let (name, rest) = segment.split_at(open);
    let mut indices = Vec::new();
    let mut cursor = rest;
    while let Some(stripped) = cursor.strip_prefix('[') {
        let Some((digits, tail)) = stripped.split_once(']') else {
            return (segment, Vec::new());
        };
        let Ok(index) = digits.parse::<usize>() else {
            return (segment, Vec::new());
        };
        indices.push(index);
        cursor = tail;
    }
    if cursor.is_empty() {
        (name, indices)
    } else {
        (segment, Vec::new())
    }
}

/// One JSON value as a text-format line value.
///
/// The whole rendering rule: `null` → `none`, a bool → `true`/`false`, an integer → its
/// decimal spelling, a string → itself, an array → its length, an object → its field count.
///
/// A container renders as a *count* rather than as a notation, for the reason
/// [`crate::render::assurance_lines`] and [`crate::render::omission_lines`] already give: a
/// count says a container is there and how big it is, and the members are printed by their
/// own keys beside it. Inventing a `{…}` spelling in a `key  value` line would be prose.
#[must_use]
pub fn render_value(value: &Json) -> String {
    match value {
        Json::Null => NONE.to_owned(),
        Json::Bool(flag) => flag.to_string(),
        Json::Integer(number) => number.to_string(),
        Json::String(text) => text.clone(),
        Json::Array(items) => items.len().to_string(),
        Json::Object(fields) => fields.len().to_string(),
    }
}

/// **The pure renderer.** The whole terminal body, computed from the machine document and an
/// ordered key list and from nothing else.
///
/// This is INV-003's strong reading as a function signature: there is no parameter here
/// through which a fact could reach the terminal without passing through the machine
/// document first.
#[must_use]
pub fn project_text<K: AsRef<str>>(document: &Json, keys: &[K]) -> String {
    let mut text = String::new();
    for key in keys {
        let key = key.as_ref();
        text.push_str(key);
        text.push_str(DELIMITER);
        text.push_str(&render_value(value_at(document, key)));
        text.push('\n');
    }
    text
}

/// The `key  value` pairs of a rendered text block, in emission order.
///
/// A line with no [`DELIMITER`], or whose key would contain a space, is read as a
/// continuation of the previous value rather than as a new pair — a `detail` string is
/// declared by RFC 0026 to be stable and non-interpolated but not to be single-line, and a
/// parser that silently produced a bogus key from one would make [`divergence`] report the
/// wrong thing. The recovery is deliberately conservative: when in doubt this returns a text
/// that will not re-render, so the check fails loudly instead of passing vacuously.
#[must_use]
pub fn parse_lines(text: &str) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for line in text.strip_suffix('\n').unwrap_or(text).split('\n') {
        match line.split_once(DELIMITER) {
            Some((key, value)) if !key.is_empty() && !key.contains(' ') => {
                pairs.push((key.to_owned(), value.to_owned()));
            }
            _ => match pairs.last_mut() {
                Some((_, value)) => {
                    value.push('\n');
                    value.push_str(line);
                }
                None => pairs.push((String::new(), line.to_owned())),
            },
        }
    }
    pairs
}

/// Why a rendered text block is **not** the projection of a machine document, or [`None`]
/// when it is.
///
/// The mechanical form of this module's §2: recover the key order from `text`, re-render it
/// from `document` alone with [`project_text`], and compare byte for byte. A returned
/// sentence names the first key that disagreed and both readings, because a bare `false`
/// would tell a maintainer that something diverged and not what.
///
/// `text` is the `text`/`pretty` body of one invocation; `document` is the parsed body of the
/// same invocation's `--format json`. The two are produced by one renderer from one
/// `Outcome`, so a divergence here is always a projection bug and never a race.
#[must_use]
pub fn divergence(text: &str, document: &Json) -> Option<String> {
    let pairs = parse_lines(text);
    for (key, value) in &pairs {
        let rendered = render_value(value_at(document, key));
        if &rendered != value {
            return Some(format!(
                "key {key:?} reads {value:?} in the terminal and {rendered:?} in the machine \
                 document; the terminal rendering must be a pure function of the document \
                 (INV-003, bn-ybh1z)"
            ));
        }
    }
    let keys: Vec<&str> = pairs.iter().map(|(key, _)| key.as_str()).collect();
    let reprojected = project_text(document, &keys);
    (reprojected != text)
        .then(|| format!("the terminal body does not re-render from the document:\n{reprojected}"))
}

#[cfg(test)]
mod tests {
    use continuumd::protocol::envelope::{
        EvaluationVerdictValue, PolicyVerdictValue, SemanticVerdictValue, StructuralVerdictValue,
    };
    use continuumd::protocol::spec::Optional;
    use continuumd::protocol::vocabulary::{AssuranceClass, InconclusiveReason};

    use super::*;

    fn document() -> Json {
        // Canonical bytes: no insignificant whitespace, keys in ascending code-point order
        // — the encoding `continuumd::codec` reads and this crate writes.
        Json::parse(
            br#"{"assurance":{"bounds":{"kind":"produced"}},"count":7,"flag":false,"handles":["ev_one","ev_two"],"missing":null,"name":"ws_one","nested":{"deep":{"leaf":"here"}}}"#,
        )
        .expect("a well-formed fixture document")
    }

    #[test]
    fn every_exit_class_has_its_own_code_and_its_own_token() {
        let codes: Vec<i32> = Exit::ALL.iter().map(|class| class.code()).collect();
        assert_eq!(codes, vec![0, 1, 2, 3, 4]);
        let mut tokens: Vec<&str> = Exit::ALL.iter().map(|class| class.token()).collect();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(tokens.len(), Exit::ALL.len(), "five distinct tokens");
    }

    #[test]
    fn a_refusal_is_declined_whichever_code_it_carries() {
        // Including the unsupported surface: the distinction is `Depth`'s token in the
        // output, not a code of its own (this module's doc).
        for code in [
            ErrorCode::UnsupportedSemanticFeature,
            ErrorCode::CapabilityDenied,
            ErrorCode::StaleSnapshot,
            ErrorCode::BudgetExhausted,
        ] {
            assert_eq!(Exit::of_code(code), Exit::Declined, "{code:?}");
            assert_eq!(Exit::of_code(code).code(), 1);
        }
    }

    #[test]
    fn a_decided_no_and_a_typed_unknown_earn_different_codes_from_a_yes() {
        let semantic = |verdict, reason| {
            Verdict::Semantic(SemanticVerdictValue {
                verdict,
                inconclusive_reason: reason,
                assurance_class: AssuranceClass::Bounded,
            })
        };
        assert_eq!(
            Exit::of_verdict(Some(&semantic(
                SemanticVerdict::Established,
                Optional::Absent
            ))),
            Exit::Success
        );
        assert_eq!(
            Exit::of_verdict(Some(&semantic(SemanticVerdict::Refuted, Optional::Absent))),
            Exit::Refuted
        );
        assert_eq!(
            Exit::of_verdict(Some(&semantic(
                SemanticVerdict::Inconclusive,
                Optional::Present(InconclusiveReason::ResourceExhausted),
            ))),
            Exit::Inconclusive
        );
        // The three codes are distinct, which is the whole point: `check … && deploy` must
        // not deploy a refuted build.
        assert_eq!(
            (
                Exit::Success.code(),
                Exit::Refuted.code(),
                Exit::Inconclusive.code()
            ),
            (0, 3, 4)
        );
    }

    #[test]
    fn the_other_three_verdict_members_and_a_null_verdict_are_classified_too() {
        assert_eq!(Exit::of_verdict(None), Exit::Success);
        for (outcome, expected) in [
            (StructuralOutcome::Sealed, Exit::Success),
            (StructuralOutcome::Cancelled, Exit::Success),
            (StructuralOutcome::Rejected, Exit::Refuted),
        ] {
            assert_eq!(
                Exit::of_verdict(Some(&Verdict::Structural(StructuralVerdictValue {
                    outcome
                }))),
                expected,
                "{outcome:?}"
            );
        }
        for (decision, expected) in [
            (PolicyDecision::Allow, Exit::Success),
            (PolicyDecision::Block, Exit::Refuted),
            (PolicyDecision::Review, Exit::Inconclusive),
        ] {
            assert_eq!(
                Exit::of_verdict(Some(&Verdict::Policy(PolicyVerdictValue {
                    decision,
                    gates: Vec::new(),
                }))),
                expected,
                "{decision:?}"
            );
        }
        for (verdict, expected) in [
            (EvaluationVerdict::Satisfied, Exit::Success),
            (EvaluationVerdict::Refuted, Exit::Refuted),
            (EvaluationVerdict::Deadlock, Exit::Refuted),
            (EvaluationVerdict::Inconclusive, Exit::Inconclusive),
        ] {
            assert_eq!(
                Exit::of_verdict(Some(&Verdict::Evaluation(EvaluationVerdictValue {
                    verdict,
                    inconclusive_reason: Optional::Absent,
                    assurance_class: AssuranceClass::Bounded,
                }))),
                expected,
                "{verdict:?}"
            );
        }
    }

    #[test]
    fn the_rendering_rule_is_total_over_every_json_shape() {
        let document = document();
        for (key, expected) in [
            ("name", "ws_one"),
            ("count", "7"),
            ("flag", "false"),
            ("missing", NONE),
            ("absent", NONE),
            // An array renders its length; its members render by their own keys.
            ("handles", "2"),
            ("handles[0]", "ev_one"),
            ("handles[1]", "ev_two"),
            ("handles[2]", NONE),
            // An object renders its field count.
            ("assurance", "1"),
            ("assurance.bounds", "1"),
            ("assurance.bounds.kind", "produced"),
            ("nested.deep.leaf", "here"),
            // Traversal is total: through a null, through a scalar, through an absence.
            ("missing.anything", NONE),
            ("name.anything", NONE),
            ("handles[0].anything", NONE),
        ] {
            assert_eq!(render_value(value_at(&document, key)), expected, "{key}");
        }
    }

    #[test]
    fn a_text_block_that_is_the_projection_reports_no_divergence() {
        let document = document();
        let keys = ["name", "count", "handles", "handles[0]", "missing"];
        let text = project_text(&document, &keys);
        assert_eq!(
            text,
            "name  ws_one\ncount  7\nhandles  2\nhandles[0]  ev_one\nmissing  none\n"
        );
        assert_eq!(divergence(&text, &document), None);
        assert_eq!(
            parse_lines(&text)
                .iter()
                .map(|(key, _)| key.as_str())
                .collect::<Vec<_>>(),
            keys
        );
    }

    #[test]
    fn the_check_can_fail_and_says_which_key_and_both_readings() {
        let document = document();
        // A value the document does not support: the terminal claiming one handle where the
        // machine channel carries two is exactly the drift this check exists to catch.
        let divergent = "name  ws_one\nhandles  1\n";
        let reported = divergence(divergent, &document).expect("a divergence is reported");
        assert!(reported.contains("handles"), "{reported}");
        assert!(reported.contains("\"1\""), "{reported}");
        assert!(reported.contains("\"2\""), "{reported}");

        // A key naming a fact the document does not carry at all fails too — that is the
        // prose-only interface INV-003 forbids, in its smallest form.
        let invented = "name  ws_one\nverdict  refuted\n";
        assert!(divergence(invented, &document).is_some());
    }

    #[test]
    fn a_multi_line_value_does_not_become_a_bogus_key() {
        let pairs = parse_lines("error.detail  first\nsecond line\nomissions  0\n");
        assert_eq!(
            pairs,
            vec![
                ("error.detail".to_owned(), "first\nsecond line".to_owned()),
                ("omissions".to_owned(), "0".to_owned()),
            ]
        );
    }

    #[test]
    fn the_reserved_keys_are_the_contract_s_own_and_are_distinct() {
        let mut sorted = RESERVED_KEYS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), RESERVED_KEYS.len());
        assert!(RESERVED_KEYS.contains(&"operation"));
        assert!(RESERVED_KEYS.contains(&"depth"));
    }
}
