//! Evidence for the PR-13 output contract (bn-ybh1z) — the cross-cutting one, held over
//! **every command in the crate at once** rather than per command group.
//!
//! `continuum_cli::contract` is the contract; this file is the golden test that pins it.
//! Its subject is the whole command surface: sixteen renderers × three arms × three formats,
//! driven through one table so that a command added without a row here is a command the
//! contract does not cover, and a command that quietly stops satisfying it fails on the same
//! line as every other.
//!
//! # Criterion → test
//!
//! - **"Terminal output is generated from the JSON result by a pure renderer; a golden test
//!   asserts the two cannot diverge"** →
//!   [`the_terminal_body_of_every_command_re_renders_from_its_own_machine_document`]. Every
//!   `key  value` line of every text output is re-computed from that invocation's `--json`
//!   document alone, by `contract::project_text`, and compared byte for byte.
//! - **"Exit codes distinguish refuted from inconclusive from operational failure, and are
//!   documented"** → [`the_five_exit_classes_are_five_distinct_documented_codes`],
//!   [`a_refuted_verdict_and_an_inconclusive_one_exit_differently_from_a_yes`], and
//!   [`the_exit_code_is_a_property_of_the_answer_and_not_of_the_format`].
//! - **"Every command's critical path is complete with color disabled and no graph
//!   rendering (feeds G8-06)"** → [`no_command_emits_an_escape_a_colour_or_a_box_glyph`] and
//!   [`pretty_is_text_plus_one_label_line_and_nothing_else`].
//! - **INV-003, "`--json` emits the machine result verbatim"** →
//!   [`the_machine_envelope_carries_the_same_key_set_on_every_arm`] and
//!   [`the_contracts_own_keys_are_on_every_answer_of_every_command`].
//!
//! # The fixture
//!
//! Hand-built [`Outcome`] values rather than a live daemon, deliberately: the subject is the
//! *projection*, and the projection has to hold for answers this deployment cannot yet
//! produce — a served `debug.open`, a refuted verdict, a nine-dimension assurance envelope,
//! a populated `next_operations`. Every command group's own file drives its renderers over
//! real frames against a real `Daemon`; this one drives all of them over the full shape of
//! what the wire types admit. Both are needed and neither substitutes for the other.

use continuum_cli::contract::{self, Exit};
use continuum_cli::format::Format;
use continuum_cli::render::Rendered;
use continuum_cli::wire::{Admitted, Outcome, Refusal};
use continuum_cli::{check, context, debug, evidence, explain, repair, snapshot, task};
use continuumd::codec::json::Json;
use continuumd::daemon::family::Payload;
use continuumd::protocol::envelope::{
    ArtifactRef, AssuranceEnvelope, Budget, Cost, Diagnostic, EnvelopeDimension, EpochSet,
    GateOutcome, NextOperation, Omission, ProducedDimension, SemanticVerdictValue, SourceSpan,
    UnsupportedDimension, Verdict,
};
use continuumd::protocol::operations::context::{ContextCompileResponse, ContextExpandResponse};
use continuumd::protocol::operations::debug::{DebugOpenResponse, DebugStateResponse};
use continuumd::protocol::operations::evidence::EvidenceGetResponse;
use continuumd::protocol::operations::repair::{RepairBeginResponse, RepairReviewResponse};
use continuumd::protocol::operations::task::{TaskCancelResponse, TaskResumeResponse};
use continuumd::protocol::operations::verification::VerificationStartResponse;
use continuumd::protocol::operations::workspace::{
    WorkspaceCreateResponse, WorkspaceForkResponse, WorkspaceSealResponse,
};
use continuumd::protocol::scalar::{
    ArtifactHandle, ByteCount, Commitment, ContextHandle, ContinuationHandle, CrashpackHandle,
    DebugHandle, DiffHandle, DurationMs, EpochIdentity, EvidenceHandle, IntentHandle, Opaque,
    OperationName, ProtocolVersion, RepairHandle, RequestId, TaskHandle, Timestamp,
    WorkspaceHandle,
};
use continuumd::protocol::shared::{
    SnapshotComponents, SnapshotEpochs, Target, VerificationResult,
};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::task::{Milestone, TaskRecord};
use continuumd::protocol::vocabulary::{
    AssuranceClass, DiagnosticSeverity, ErrorCode, Fragment, GateName, GateStatus,
    InconclusiveReason, OmissionReason, Portfolio, PriorityClass, RedactionReason, ResultStatus,
    SemanticVerdict, TargetKind, TaskStatus,
};

// --- the table --------------------------------------------------------------------------

/// One command's renderer, on one arm, ready to run in any format.
struct Case {
    /// `"<command> / <arm>"`, for a failure message that names which cell broke.
    name: String,
    /// The command's own name — the label the `pretty` format prepends and nothing else adds.
    command: &'static str,
    /// Whether the daemon answered at all, which is what the exit code is about.
    admitted: bool,
    render: Box<dyn Fn(Format) -> Rendered>,
}

/// The three arms every command is held over.
///
/// Two refusals, not one: `unsupported` is the surface this deployment does not serve and
/// `refused` is a `no` about the request, and the contract's claim is that they share an
/// exit code and differ by a typed token — which a single-refusal matrix could not see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arm {
    Admitted,
    Refused,
    Unsupported,
}

impl Arm {
    const ALL: [Self; 3] = [Self::Admitted, Self::Refused, Self::Unsupported];

    const fn token(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::Refused => "refused",
            Self::Unsupported => "unsupported",
        }
    }
}

fn outcome<T>(arm: Arm, payload: T, verdict: Option<Verdict>) -> Outcome<T> {
    match arm {
        Arm::Admitted => Outcome::Admitted(Admitted {
            request_id: request_id(),
            status: ResultStatus::Ok,
            payload,
            verdict,
            assurance: Some(assurance()),
            task: Some(task_handle()),
            continuation: Some(continuation()),
            omissions: vec![
                omission(OmissionReason::Budget, "context.depth", None),
                omission(
                    OmissionReason::Redaction,
                    "evidence.body",
                    Some("ev_hidden1"),
                ),
            ],
            artifacts: Vec::<ArtifactRef>::new(),
            cost: cost(),
            next_operations: vec![next_operation()],
        }),
        Arm::Refused => Outcome::Refused(refusal(ErrorCode::CapabilityDenied)),
        Arm::Unsupported => Outcome::Refused(refusal(ErrorCode::UnsupportedSemanticFeature)),
    }
}

/// Every command in the crate, on every arm.
///
/// Sixteen renderers. The count is asserted rather than assumed
/// ([`the_table_covers_every_command_the_crate_renders`]) so that a command shipped without
/// a row is a failing test and not a silent gap in the contract.
fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for arm in Arm::ALL {
        // --- snapshot ---------------------------------------------------------------
        push(&mut cases, arm, "snapshot create", {
            let out = outcome(
                arm,
                Payload::WorkspaceCreate(WorkspaceCreateResponse {
                    snapshot: workspace("ws_created1"),
                    sealed: true,
                    diagnostics: vec![diagnostic()],
                }),
                None,
            );
            move |format| snapshot::render_create(&create_args(), &out, format)
        });
        push(&mut cases, arm, "snapshot fork", {
            let out = outcome(
                arm,
                Payload::WorkspaceFork(WorkspaceForkResponse {
                    snapshot: workspace("ws_forked01"),
                    intent: IntentHandle::new("in_ack_v1").expect("an intent handle"),
                    pre_diff: Optional::Present(
                        DiffHandle::new("diff_pre001").expect("a diff handle"),
                    ),
                    diagnostics: vec![diagnostic()],
                }),
                None,
            );
            move |format| snapshot::render_fork(&fork_args(), &out, format)
        });
        push(&mut cases, arm, "snapshot seal", {
            let out = outcome(
                arm,
                Payload::WorkspaceSeal(WorkspaceSealResponse {
                    snapshot: workspace("ws_sealed01"),
                    root_digest: Commitment::new("blake3-256:rootdigest"),
                }),
                Some(structural_verdict()),
            );
            move |format| snapshot::render_seal(&seal_args(), &out, format)
        });

        // --- check ------------------------------------------------------------------
        push(&mut cases, arm, "check start", {
            let out = outcome(
                arm,
                Payload::VerificationStart(VerificationStartResponse {
                    task: Optional::Present(task_handle()),
                    result: Optional::Absent,
                }),
                None,
            );
            move |format| check::render_start(&start_args(), &out, format)
        });
        push(&mut cases, arm, "check result", {
            let out = outcome(
                arm,
                Payload::VerificationResult(verification_result()),
                Some(refuted_verdict()),
            );
            move |format| {
                check::render_result(
                    &check::ResultArgs {
                        task: task_handle(),
                    },
                    &out,
                    format,
                )
            }
        });
        push(&mut cases, arm, "check await", {
            let out = outcome(
                arm,
                Payload::VerificationAwait(verification_result()),
                Some(inconclusive_verdict()),
            );
            move |format| {
                check::render_await(
                    &check::AwaitArgs {
                        task: task_handle(),
                        timeout_ms: 2_500,
                    },
                    &out,
                    format,
                )
            }
        });

        // --- explain ----------------------------------------------------------------
        push(&mut cases, arm, "explain compile", {
            let out = outcome(
                arm,
                Payload::ContextCompile(ContextCompileResponse {
                    context: ContextHandle::new("ctx_compiled1").expect("a context handle"),
                    pack: Opaque::from_bytes(PACK.as_bytes().to_vec()),
                }),
                None,
            );
            move |format| explain::render_compile(&compile_args(), &out, format)
        });

        // --- context ----------------------------------------------------------------
        push(&mut cases, arm, "context expand", {
            let out = outcome(
                arm,
                ContextExpandResponse {
                    context: ContextHandle::new("ctx_child0001").expect("a context handle"),
                    parent: ContextHandle::new("ctx_parent001").expect("a context handle"),
                    pack: Opaque::from_bytes(PACK.as_bytes().to_vec()),
                },
                None,
            );
            move |format| context::render(&expand_args(), &out, format)
        });

        // --- task -------------------------------------------------------------------
        push(&mut cases, arm, "task status", {
            let out = outcome(arm, Payload::TaskStatus(task_record()), None);
            move |format| task::render_status(&task_handle(), &out, format)
        });
        push(&mut cases, arm, "task resume", {
            let out = outcome(
                arm,
                Payload::TaskResume(TaskResumeResponse {
                    task: task_handle(),
                    status: TaskStatus::Running,
                }),
                None,
            );
            move |format| task::render_resume(&continuation(), 4_096, &out, format)
        });
        push(&mut cases, arm, "task cancel", {
            let out = outcome(
                arm,
                Payload::TaskCancel(TaskCancelResponse {
                    task: task_handle(),
                    status: TaskStatus::Cancelled,
                    continuation: Nullable::Value(continuation()),
                    committed_evidence: vec![evidence_handle()],
                }),
                None,
            );
            move |format| task::render_cancel(&task_handle(), &out, format)
        });

        // --- evidence ---------------------------------------------------------------
        push(&mut cases, arm, "evidence show", {
            let out = outcome(
                arm,
                Payload::EvidenceGet(EvidenceGetResponse {
                    node: Nullable::Value(Opaque::from_bytes(
                        br#"{"id":"node-42","kind":"event"}"#.to_vec(),
                    )),
                    edge: Nullable::Null,
                    redacted: Optional::Present(continuumd::protocol::envelope::Redacted {
                        redacted: true,
                        reason: RedactionReason::Purged,
                        commitment: Commitment::new("blake3-256:withheld"),
                        original_class: "evidence".to_owned(),
                    }),
                }),
                None,
            );
            move |format| evidence::render(&show_args(), &out, format)
        });

        // --- debug ------------------------------------------------------------------
        push(&mut cases, arm, "debug open", {
            let out = outcome(
                arm,
                DebugOpenResponse {
                    branch: DebugHandle::new("dbg_branch01").expect("a debug handle"),
                    frontier: Opaque::from_bytes(br#"{"events":["fill"],"step":3}"#.to_vec()),
                },
                None,
            );
            move |format| debug::render_open(&open_args(), &out, format)
        });
        push(&mut cases, arm, "debug state", {
            let out = outcome(
                arm,
                DebugStateResponse {
                    state: Opaque::from_bytes(br#"{"big":1,"small":2}"#.to_vec()),
                },
                None,
            );
            move |format| debug::render_state(&state_args(), &out, format)
        });

        // --- repair -----------------------------------------------------------------
        push(&mut cases, arm, "repair begin", {
            let out = outcome(
                arm,
                RepairBeginResponse {
                    repair: repair_handle(),
                },
                None,
            );
            move |format| repair::render_begin(&begin_args(), &out, format)
        });
        push(&mut cases, arm, "repair review", {
            let out = outcome(
                arm,
                RepairReviewResponse {
                    repair: repair_handle(),
                    semantic_diff: Nullable::Value(
                        DiffHandle::new("diff_review01").expect("a diff handle"),
                    ),
                    gates: vec![GateOutcome {
                        name: GateName::IntentIntegrity,
                        status: GateStatus::Passed,
                        evidence: vec![evidence_handle()],
                    }],
                    evidence: vec![evidence_handle()],
                },
                None,
            );
            move |format| {
                repair::render_review(
                    &repair::ReviewArgs {
                        repair: repair_handle(),
                    },
                    &out,
                    format,
                )
            }
        });
    }
    cases
}

fn push<F: Fn(Format) -> Rendered + 'static>(
    cases: &mut Vec<Case>,
    arm: Arm,
    command: &'static str,
    render: F,
) {
    cases.push(Case {
        name: format!("{command} / {}", arm.token()),
        command,
        admitted: arm == Arm::Admitted,
        render: Box::new(render),
    });
}

/// How many distinct commands the table covers, one row per (command, arm).
const COMMANDS: usize = 16;

// --- the contract, pinned ------------------------------------------------------------------

/// **Acceptance criterion 1.** For every command, on every arm, the terminal body is the
/// projection of that same invocation's machine document — every value in it re-computed
/// from the JSON alone, by `contract::project_text`, and equal byte for byte.
///
/// This is the statement "the two cannot diverge" made mechanical. A renderer that printed a
/// number the machine channel does not carry, a key the document does not have, or a
/// *different* reading of a shared field fails here, on the cell that did it.
#[test]
fn the_terminal_body_of_every_command_re_renders_from_its_own_machine_document() {
    for case in cases() {
        let json = (case.render)(Format::Json);
        let document = Json::parse(json.text.trim_end().as_bytes()).unwrap_or_else(|error| {
            panic!("{}: --json is a canonical document: {error:?}", case.name)
        });
        let text = (case.render)(Format::Text);
        assert_eq!(
            contract::divergence(&text.text, &document),
            None,
            "{}\n--- terminal ---\n{}--- machine ---\n{}",
            case.name,
            text.text,
            json.text
        );
    }
}

/// The check above can fail, on a real command's real output.
///
/// A golden test that could not distinguish two different renderings would prove nothing
/// about either, so this perturbs one — a byte-length key renamed to the dotted spelling it
/// carried before bn-ybh1z, which is the exact drift the rename fixed — and asserts the
/// check reports it, naming the key.
#[test]
fn the_projection_check_reports_a_real_renderers_drift() {
    let out = outcome(
        Arm::Admitted,
        Payload::EvidenceGet(EvidenceGetResponse {
            node: Nullable::Value(Opaque::from_bytes(br#"{"id":"node-42"}"#.to_vec())),
            edge: Nullable::Null,
            redacted: Optional::Absent,
        }),
        None,
    );
    let document = Json::parse(
        evidence::render(&show_args(), &out, Format::Json)
            .text
            .trim_end()
            .as_bytes(),
    )
    .expect("a canonical document");
    let text = evidence::render(&show_args(), &out, Format::Text).text;
    assert_eq!(contract::divergence(&text, &document), None);

    // `node.bytes` reads the *node document's* own `bytes` field, which it does not have —
    // so the terminal would claim a length the machine channel never stated.
    let drifted = text.replace("node_bytes  ", "node.bytes  ");
    assert_ne!(drifted, text, "the perturbation changed something");
    let reported = contract::divergence(&drifted, &document).expect("the drift is reported");
    assert!(reported.contains("node.bytes"), "{reported}");
}

/// **Acceptance criterion 3, first half.** `pretty` is `text` with one prepended label line
/// and nothing else — no colour, no table, no reflow, no dropped field.
///
/// The label is the one string in this crate that is not a projection of the machine
/// document, it exists in exactly one format, and it carries no fact: it names the command
/// the reader just typed. Everything a human sees at a terminal is therefore everything an
/// agent sees in a pipe, which is what G8-06 asks of a critical path.
#[test]
fn pretty_is_text_plus_one_label_line_and_nothing_else() {
    for case in cases() {
        let text = (case.render)(Format::Text);
        let pretty = (case.render)(Format::Pretty);
        assert_eq!(
            pretty.text,
            format!("command  {}\n{}", case.command, text.text),
            "{}",
            case.name
        );
    }
}

/// **Acceptance criterion 3, second half (G8-06).** No output of any command, in any format,
/// on any arm, carries an ANSI escape, a C0 control other than the line feed that ends a
/// line, or a box-drawing glyph.
///
/// A property of the code and not of a flag: this crate has no `--no-color`, reads no
/// `NO_COLOR`, and branches on no TTY when it renders, because there is nothing to switch
/// off. The test is what keeps that true — a `\x1b[32m` added to one renderer fails here
/// rather than in a Phase F accessibility audit.
#[test]
fn no_command_emits_an_escape_a_colour_or_a_box_glyph() {
    for case in cases() {
        for format in [Format::Text, Format::Pretty, Format::Json] {
            let rendered = (case.render)(format);
            for character in rendered.text.chars() {
                assert!(
                    character == '\n' || !character.is_control(),
                    "{} in {format:?} emits control character {character:?}",
                    case.name
                );
                assert!(
                    !is_box_drawing(character),
                    "{} in {format:?} emits box-drawing glyph {character:?}",
                    case.name
                );
            }
        }
    }
}

/// U+2500–U+257F (box drawing) and U+2580–U+259F (block elements) — the glyphs a table or a
/// rendered graph would be drawn from.
fn is_box_drawing(character: char) -> bool {
    ('\u{2500}'..='\u{259f}').contains(&character)
}

/// **Acceptance criterion 2, the taxonomy itself.** Five classes, five distinct codes, five
/// distinct tokens, and the three the conventions doc already fixed keep their numbers.
#[test]
fn the_five_exit_classes_are_five_distinct_documented_codes() {
    assert_eq!(Exit::Success.code(), 0);
    assert_eq!(Exit::Declined.code(), 1);
    assert_eq!(Exit::Fault.code(), 2);
    assert_eq!(Exit::Refuted.code(), 3);
    assert_eq!(Exit::Inconclusive.code(), 4);

    let mut codes: Vec<i32> = Exit::ALL.iter().map(|class| class.code()).collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), Exit::ALL.len());
}

/// **Acceptance criterion 2, the distinction that matters.** A refuted verdict and an
/// inconclusive one exit differently from each other and from a yes — on an answer that was
/// *admitted* in all three cases, which is exactly the distinction no `Depth` token and no
/// refusal code can carry.
///
/// The failure this removes: `continuum check result <task> && deploy` deploying a build
/// whose invariant was refuted, because the command "succeeded".
#[test]
fn a_refuted_verdict_and_an_inconclusive_one_exit_differently_from_a_yes() {
    let render = |verdict: Option<Verdict>| {
        let out = outcome(
            Arm::Admitted,
            Payload::VerificationResult(verification_result()),
            verdict,
        );
        check::render_result(
            &check::ResultArgs {
                task: task_handle(),
            },
            &out,
            Format::Text,
        )
        .exit_code
    };
    assert_eq!(render(Some(established_verdict())), 0);
    assert_eq!(render(Some(refuted_verdict())), 3);
    assert_eq!(render(Some(inconclusive_verdict())), 4);
    // An operation with no `verdict` clause answers `verdict: null`, which is a success and
    // not an unknown: there was nothing to decide.
    assert_eq!(render(None), 0);
}

/// **Acceptance criterion 2, the operational half.** Both refusal arms earn `1`, and the
/// distinction between them is the typed `depth` token in every format — the reading
/// bn-1g7e4 chose and bn-ybh1z ratified.
#[test]
fn both_refusal_arms_exit_one_and_differ_by_a_typed_token() {
    for case in cases() {
        if case.admitted {
            continue;
        }
        let text = (case.render)(Format::Text);
        assert_eq!(text.exit_code, 1, "{}", case.name);
        let expected = if case.name.ends_with("unsupported") {
            "depth  unsupported\n"
        } else {
            "depth  refused\n"
        };
        assert!(
            text.text.contains(expected),
            "{} names its depth: {}",
            case.name,
            text.text
        );
    }
}

/// The exit code is a property of the *answer*, so choosing a format never changes it.
#[test]
fn the_exit_code_is_a_property_of_the_answer_and_not_of_the_format() {
    for case in cases() {
        let text = (case.render)(Format::Text);
        let pretty = (case.render)(Format::Pretty);
        let json = (case.render)(Format::Json);
        assert_eq!(text.exit_code, pretty.exit_code, "{}", case.name);
        assert_eq!(text.exit_code, json.exit_code, "{}", case.name);
    }
}

/// **INV-003, "no dropped fields".** The machine envelope has the same key set whichever arm
/// the answer took, so a parser never branches on which keys exist before it can read one.
#[test]
fn the_machine_envelope_carries_the_same_key_set_on_every_arm() {
    let mut by_command: std::collections::BTreeMap<&'static str, Vec<Vec<String>>> =
        std::collections::BTreeMap::new();
    for case in cases() {
        let json = (case.render)(Format::Json);
        let document =
            Json::parse(json.text.trim_end().as_bytes()).expect("--json is a canonical document");
        let Json::Object(fields) = document else {
            panic!("{}: the envelope is always an object", case.name);
        };
        by_command
            .entry(case.command)
            .or_default()
            .push(fields.keys().cloned().collect());
    }
    for (command, arms) in by_command {
        let first = &arms[0];
        for other in &arms[1..] {
            assert_eq!(first, other, "{command} answers one key set on every arm");
        }
    }
}

/// **INV-003, the envelope.** Every answer of every command carries the contract's own keys,
/// in every format that has them: the operation and depth in all three, the omission
/// manifest and the allowed next operations in all three, and `error`/`advice` in the
/// machine one.
#[test]
fn the_contracts_own_keys_are_on_every_answer_of_every_command() {
    for case in cases() {
        let document = Json::parse((case.render)(Format::Json).text.trim_end().as_bytes())
            .expect("--json is a canonical document");
        let Json::Object(fields) = &document else {
            panic!("{}: the envelope is always an object", case.name);
        };
        for key in contract::RESERVED_KEYS {
            assert!(fields.contains_key(key), "{}: {key} is present", case.name);
        }
        // INV-007's manifest and RFC 0026's recovery channel are lists in the machine
        // channel, never counts: a caller reads the omission or the next step itself.
        assert!(
            matches!(fields.get("omissions"), Some(Json::Array(_))),
            "{}: omissions is an array",
            case.name
        );
        assert!(
            matches!(fields.get("next_operations"), Some(Json::Array(_))),
            "{}: next_operations is an array",
            case.name
        );
        assert!(
            matches!(fields.get("advice"), Some(Json::Array(_))),
            "{}: advice is an array",
            case.name
        );

        let text = (case.render)(Format::Text);
        for key in ["operation", "depth", "omissions", "next_operations"] {
            assert!(
                text.text.contains(&format!("\n{key}  "))
                    || text.text.starts_with(&format!("{key}  ")),
                "{}: the text format names {key}:\n{}",
                case.name,
                text.text
            );
        }
    }
}

/// The table is the contract's coverage, so it says how many commands it covers and is held
/// to it. A command added to the crate without a row here would leave the contract
/// unenforced for exactly that command, which is the per-command drift this bone exists to
/// end.
#[test]
fn the_table_covers_every_command_the_crate_renders() {
    let commands: std::collections::BTreeSet<&'static str> =
        cases().iter().map(|case| case.command).collect();
    assert_eq!(commands.len(), COMMANDS, "{commands:?}");
    assert_eq!(cases().len(), COMMANDS * Arm::ALL.len());
    // The eight noun groups `cli::run` dispatches are all represented.
    for noun in continuum_cli::cli::NOUNS {
        assert!(
            commands.iter().any(|command| command.starts_with(noun)),
            "{noun} has at least one command in the table"
        );
    }
}

/// The three envelope facts that reached no format at all before bn-ybh1z — the request
/// identifier, the nine-dimension cost, and the artifacts the call published — are on every
/// command's machine document, with their values and not a placeholder.
///
/// They are machine-channel only, on purpose (`continuum_cli::render::project`), and that
/// asymmetry is the contract's allowed direction: prose is a selection of the machine
/// document, never a superset of it.
#[test]
fn the_envelope_cost_artifacts_and_request_id_reach_the_machine_channel() {
    for case in cases() {
        let document = Json::parse((case.render)(Format::Json).text.trim_end().as_bytes())
            .expect("--json is a canonical document");
        assert_eq!(
            contract::render_value(contract::value_at(&document, "request_id")),
            "req_cli000001",
            "{}: the identifier the daemon echoed, on both arms",
            case.name
        );
        if case.admitted {
            // The measured dimensions carry their numbers and the unmeasured ones read
            // `null` — "a dimension the engine does not measure is absent, never zero".
            assert_eq!(
                contract::render_value(contract::value_at(&document, "cost.states")),
                "573",
                "{}",
                case.name
            );
            assert_eq!(
                contract::render_value(contract::value_at(&document, "cost.cpu_ms")),
                "none",
                "{}",
                case.name
            );
            assert_eq!(
                contract::render_value(contract::value_at(&document, "cost.tokenizer_id")),
                "none",
                "{}",
                case.name
            );
        } else {
            // A refusal carries no cost clause and publishes nothing.
            assert!(
                contract::value_at(&document, "cost").is_null(),
                "{}",
                case.name
            );
        }
        assert!(
            matches!(contract::value_at(&document, "artifacts"), Json::Array(_)),
            "{}: artifacts is a list on both arms",
            case.name
        );
        // …and none of the three leaks into the terminal body, which stays terse. Compared
        // by whole line: `task status` renders the *record's* cost, under `record.cost.…`,
        // which is a response field and not this envelope key.
        let text = (case.render)(Format::Text);
        let keys: Vec<&str> = text
            .text
            .lines()
            .filter_map(|line| line.split("  ").next())
            .collect();
        for key in ["request_id", "cost", "cost.states", "artifacts"] {
            assert!(
                !keys.contains(&key),
                "{}: {key:?} is machine-channel only\n{}",
                case.name,
                text.text
            );
        }
    }
}

/// A refusal's `recovery` list and an answer's `next_operations` list reach the machine
/// channel as lists — the fix bn-ybh1z made to a projection that carried only their lengths,
/// in the one channel with no terminal to overflow. RFC 0026 calls `recovery` "the only
/// recovery channel"; a length is not one.
#[test]
fn the_recovery_and_next_operation_lists_travel_as_lists() {
    let out = outcome(
        Arm::Admitted,
        DebugOpenResponse {
            branch: DebugHandle::new("dbg_branch01").expect("a debug handle"),
            frontier: Opaque::from_bytes(b"{}".to_vec()),
        },
        None,
    );
    let document = Json::parse(
        debug::render_open(&open_args(), &out, Format::Json)
            .text
            .trim_end()
            .as_bytes(),
    )
    .expect("a canonical document");
    let next = contract::value_at(&document, "next_operations[0]");
    assert_eq!(
        contract::render_value(contract::value_at(
            &document,
            "next_operations[0].operation"
        )),
        "task.status"
    );
    assert!(matches!(next, Json::Object(_)));
    // The pre-filled arguments are the document itself, not a length a caller would have to
    // make a second call to resolve — with the length beside it.
    assert_eq!(
        contract::render_value(contract::value_at(
            &document,
            "next_operations[0].arguments.task"
        )),
        "task_frozen01"
    );

    let refused = Outcome::<DebugOpenResponse>::Refused(refusal(ErrorCode::CapabilityDenied));
    let document = Json::parse(
        debug::render_open(&open_args(), &refused, Format::Json)
            .text
            .trim_end()
            .as_bytes(),
    )
    .expect("a canonical document");
    assert!(matches!(
        contract::value_at(&document, "error.recovery"),
        Json::Array(_)
    ));
    assert_eq!(
        contract::render_value(contract::value_at(&document, "error.recovery[0].operation")),
        "task.status"
    );
}

// --- fixture material ---------------------------------------------------------------------

/// A conforming Context Pack, canonical, for the two commands that project one.
const PACK: &str = r#"{"assurance":{"class":"bounded","envelope":{}},"content_budget":{"bytes":4096,"nodes":12},"content_hash":"blake3-256:packplaceholder","context_id":"ctx_explained00001","evidence":["ev_failure1"],"expansions":[{"anchor":"node-42","relation":"source_span"}],"guarantees":["ReplayPreserving"],"inconclusive_reason":"ResourceExhausted","intent":"in_ack_v1","omissions":[{"count":2,"expandable":true,"expansion":{"anchor":"node-42","relation":"source_span"},"kind":"source","reason":"budget"}],"parent":null,"question":"why did AckImpliesDurable fail?","redactions":[{"commitment":"blake3-256:withheld1","original_class":"evidence","reason":"purged","redacted":true}],"replay":"crash_demo1","schema_epoch":1,"schema_id":"https://continuum.dev/schema/context-pack.json","selected":[{"artifact":"ev_ack1","id":"node-42","kind":"event","summary":"reply published before stable write"}],"semantic_epoch":"sem3-r3-demo","snapshot":"ws_demo1","verdict":"inconclusive"}"#;

fn request_id() -> RequestId {
    RequestId::new("req_cli000001").expect("a request id")
}

fn task_handle() -> TaskHandle {
    TaskHandle::new("task_frozen01").expect("a task handle")
}

fn continuation() -> ContinuationHandle {
    ContinuationHandle::new("cont_parked01").expect("a continuation handle")
}

fn evidence_handle() -> EvidenceHandle {
    EvidenceHandle::new("ev_committed1").expect("an evidence handle")
}

fn repair_handle() -> RepairHandle {
    RepairHandle::new("rt_transaction1").expect("a repair handle")
}

fn workspace(token: &str) -> WorkspaceHandle {
    WorkspaceHandle::new(token).expect("a workspace handle")
}

fn omission(reason: OmissionReason, subject: &str, recoverable: Option<&str>) -> Omission {
    Omission {
        reason,
        subject: subject.to_owned(),
        recoverable_by: match recoverable {
            Some(handle) => {
                Optional::Present(ArtifactHandle::new(handle).expect("an artifact handle"))
            }
            None => Optional::Absent,
        },
    }
}

/// One allowed next operation, with its arguments pre-filled — the shape RFC 0026 declares
/// and plan §3.1's five-minute path renders.
fn next_operation() -> NextOperation {
    NextOperation {
        operation: OperationName::new("task.status").expect("a registry name"),
        arguments: Opaque::from_bytes(br#"{"task":"task_frozen01"}"#.to_vec()),
        rationale: Optional::Present("read the record before resuming".to_owned()),
    }
}

fn refusal(code: ErrorCode) -> Refusal {
    Refusal {
        request_id: request_id(),
        code,
        detail: "the request body is not the shape this operation declares".to_owned(),
        retryable: false,
        recovery: vec![next_operation()],
        continuation: Some(continuation()),
        non_resumable_reason: None,
        // The fixture's codes declare no `Error.data` shape, so the typed specifics read
        // `None` and the contract renders `error.data  none` (RFC 0026 F19, bn-3jrtz).
        data: None,
    }
}

fn cost() -> Cost {
    Cost {
        wall_ms: Optional::Present(DurationMs::new(12)),
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Present(ByteCount::new(4_096)),
        states: Optional::Present(573),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
        tokenizer_id: Optional::Absent,
    }
}

fn budget() -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(4_096),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

/// A mixed envelope: produced dimensions beside typed unsupported ones, so the projection is
/// exercised on both union arms of all nine.
fn assurance() -> AssuranceEnvelope {
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
    AssuranceEnvelope {
        bounds: produced(),
        faults: unsupported(),
        fairness: unsupported(),
        values: produced(),
        schedules: unsupported(),
        memory_model: unsupported(),
        observer: unsupported(),
        proof_status: unsupported(),
        unknowns: produced(),
    }
}

fn semantic(verdict: SemanticVerdict, reason: Optional<InconclusiveReason>) -> Verdict {
    Verdict::Semantic(SemanticVerdictValue {
        verdict,
        inconclusive_reason: reason,
        assurance_class: AssuranceClass::Bounded,
    })
}

fn established_verdict() -> Verdict {
    semantic(SemanticVerdict::Established, Optional::Absent)
}

fn refuted_verdict() -> Verdict {
    semantic(SemanticVerdict::Refuted, Optional::Absent)
}

fn inconclusive_verdict() -> Verdict {
    semantic(
        SemanticVerdict::Inconclusive,
        Optional::Present(InconclusiveReason::ResourceExhausted),
    )
}

fn structural_verdict() -> Verdict {
    Verdict::Structural(continuumd::protocol::envelope::StructuralVerdictValue {
        outcome: continuumd::protocol::vocabulary::StructuralOutcome::Sealed,
    })
}

fn diagnostic() -> Diagnostic {
    Diagnostic {
        severity: DiagnosticSeverity::Warning,
        code: "overlay.shadowed".to_owned(),
        detail: "an overlay shadows a declared component".to_owned(),
        span: Optional::Present(SourceSpan {
            file: "src/protocol.rs".to_owned(),
            start_line: 12,
            start_column: 3,
            end_line: 12,
            end_column: 41,
        }),
    }
}

fn verification_result() -> VerificationResult {
    VerificationResult {
        task: task_handle(),
        target: Target {
            kind: TargetKind::Property,
            id: "AckImpliesDurable".to_owned(),
        },
        fragments: vec![Fragment::Finite, Fragment::Temporal],
        evidence: vec![evidence_handle()],
        crashpack: Optional::Present(
            CrashpackHandle::new("crash_die_hard_1").expect("a crashpack handle"),
        ),
        context: Optional::Absent,
        continuation: Optional::Present(continuation()),
    }
}

fn task_record() -> TaskRecord {
    TaskRecord {
        task: task_handle(),
        operation: OperationName::new("verification.start").expect("a registry name"),
        status: TaskStatus::Suspended,
        snapshot: Nullable::Value(workspace("ws_sealed01")),
        intent: Nullable::Value(IntentHandle::new("in_ack_v1").expect("an intent handle")),
        failed_reason: Optional::Absent,
        continuation: Optional::Present(continuation()),
        non_resumable_reason: Optional::Absent,
        budget: budget(),
        cost: cost(),
        epochs: EpochSet {
            protocol: ProtocolVersion::new(3, 2),
            semantic: Nullable::Value(
                EpochIdentity::new("sem3-r3-demo").expect("an epoch identity"),
            ),
            intent: Nullable::Null,
            evidence: Nullable::Null,
            proof: Nullable::Null,
            corpus: Nullable::Null,
            engine: Nullable::Value(
                EpochIdentity::new("engine-reference-1").expect("an epoch identity"),
            ),
        },
        priority_class: PriorityClass::Interactive,
        milestones: vec![Milestone {
            name: "frontier-explored".to_owned(),
            at: Timestamp::new("2026-01-01T00:00:00.000Z").expect("a timestamp"),
        }],
        committed_evidence: vec![evidence_handle()],
    }
}

// --- the parsed arguments each renderer takes ----------------------------------------------

fn components() -> SnapshotComponents {
    SnapshotComponents {
        files: vec![Commitment::new("blake3-256:onefile")],
        cml_modules: Vec::new(),
        rust_extraction: Vec::new(),
        domain_packs: Vec::new(),
        dependencies: Vec::new(),
        epochs: SnapshotEpochs {
            semantic: EpochIdentity::new("sem3-r3-demo").expect("an epoch identity"),
            proof: EpochIdentity::new("proof-1").expect("an epoch identity"),
            toolchain: Optional::Absent,
        },
        intent: IntentHandle::new("in_ack_v1").expect("an intent handle"),
        correspondence: Vec::new(),
        proof_environment: Vec::new(),
        configuration: Vec::new(),
        file_components: Optional::Absent,
    }
}

fn create_args() -> snapshot::CreateArgs {
    snapshot::CreateArgs {
        components: components(),
        overlay: Vec::new(),
        seal: true,
    }
}

fn fork_args() -> snapshot::ForkArgs {
    snapshot::ForkArgs {
        base: workspace("ws_base00001"),
        overlay: Vec::new(),
        patches: Vec::new(),
    }
}

fn seal_args() -> snapshot::SealArgs {
    snapshot::SealArgs {
        snapshot: workspace("ws_base00001"),
    }
}

fn start_args() -> check::StartArgs {
    check::StartArgs {
        snapshot: workspace("ws_sealed01"),
        target: Target {
            kind: TargetKind::Property,
            id: "AckImpliesDurable".to_owned(),
        },
        portfolio: Portfolio::Interactive,
        priority_class: Optional::Present(PriorityClass::Interactive),
        states: 4_096,
    }
}

fn compile_args() -> explain::CompileArgs {
    explain::CompileArgs {
        evidence_root: ArtifactHandle::new("ev_failure1").expect("an artifact handle"),
        question: "why did AckImpliesDurable fail?".to_owned(),
        audience: Optional::Absent,
        guarantees: vec!["ReplayPreserving".to_owned()],
        bytes: 4_096,
    }
}

fn expand_args() -> context::ExpandArgs {
    context::ExpandArgs {
        context: ContextHandle::new("ctx_parent001").expect("a context handle"),
        anchor: "node-42".to_owned(),
        relation: continuumd::protocol::vocabulary::ExpansionRelation::SourceSpan,
        depth: Optional::Present(2),
        states: Optional::Absent,
    }
}

fn show_args() -> evidence::ShowArgs {
    evidence::ShowArgs {
        evidence: evidence_handle(),
        inline: true,
    }
}

fn open_args() -> debug::OpenArgs {
    debug::OpenArgs {
        subject: ArtifactHandle::new("crash_die_hard_1").expect("an artifact handle"),
        observer: Some("observer-a".to_owned()),
    }
}

fn state_args() -> debug::StateArgs {
    debug::StateArgs {
        branch: DebugHandle::new("dbg_branch01").expect("a debug handle"),
        observer: None,
    }
}

fn begin_args() -> repair::BeginArgs {
    repair::BeginArgs {
        failure: CrashpackHandle::new("crash_die_hard_1").expect("a crashpack handle"),
        gate_profile: continuumd::protocol::vocabulary::GateProfile::Default,
    }
}
