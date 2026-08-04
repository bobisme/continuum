//! Evidence for `continuum debug open|state` and `continuum repair begin|review`
//! (bn-1g7e4, PR-13 second command group) — the failure-investigation half.
//!
//! # What these tests are evidence for
//!
//! Both command families drive operations `continuumd` **registers and does not serve**:
//! `continuumd::protocol::registry` carries all eleven `debug` rows and all eight `repair`
//! rows, `continuumd::protocol::operations::{debug,repair}` declares every request and
//! response struct, and there is no `debug`/`repair` `OperationFamily` and no
//! `daemon::family::Arguments` arm. The obligation this bone owes is therefore not "make
//! them work" — PR 19 and PR 20 own that — but **INV-008 at the interface**: the answer is
//! typed, machine-readable, names exactly what is unsupported, and is never a crash, an
//! opaque failure, or a fabricated result.
//!
//! # Clause → test
//!
//! - **"a typed, machine-readable unsupported result naming exactly what is unsupported"**
//!   → [`every_unserved_operation_answers_the_protocols_own_unsupported_code`] (the wire
//!   fact) and [`the_unsupported_depth_and_the_operation_are_named_in_every_format`] (the
//!   rendering, in all three).
//! - **"never a crash, never an opaque failure"** →
//!   [`an_unserved_surface_is_a_rendered_answer_and_never_a_cli_error`], and
//!   [`a_large_observer_string_is_still_a_typed_answer`] for the adversarial input.
//! - **"never a fabricated answer"** →
//!   [`an_unsupported_answer_carries_no_response_body_in_machine_output`] — every response
//!   key is present and `null`, so a parser sees the same key set on both arms and no key
//!   is quietly filled in.
//! - **INV-007 non-suppressible** →
//!   [`the_json_envelope_always_carries_the_omissions_key_on_the_unsupported_arm`].
//! - **INV-002 explicit handles** →
//!   [`no_command_in_this_group_reaches_the_wire_without_its_handle`] — the usage errors,
//!   asserted against a transport that would panic-by-refusal if a frame were ever written.
//! - **the success arms, proven before the families land** →
//!   [`the_debug_open_renderer_prints_the_branch_and_the_frontier`],
//!   [`the_debug_state_renderer_embeds_the_state_verbatim`],
//!   [`the_repair_begin_renderer_prints_the_transaction_handle`],
//!   [`the_repair_review_renderer_prints_every_gate_by_name`]. Same reasoning bn-3tz60 gave
//!   for `context.expand`: a renderer that only handled the refusal it happens to see today
//!   would be untested on the day the family lands.
//!
//! # The fixture
//!
//! One in-process deployment behind a real `Server`/`LocalPair`, with **no families
//! registered at all** — deliberately. A `debug.open` frame is refused by
//! `codec::operations::decode_arguments` before dispatch begins, so which families a
//! deployment happens to serve cannot change this answer, and a fixture that registered
//! some would suggest it could. Duplicated locally rather than imported, for the reason
//! `continuum-mcp/tests/typed_surface.rs`'s identical comment gives: a `tests/*.rs` file is
//! its own crate.

use continuum_cli::format::Format;
use continuum_cli::render::Depth;
use continuum_cli::wire::{Admitted, Connection, LinkError, LocalLink, Outcome, Transport};
use continuum_cli::{cli, debug, repair};
use continuum_value::epoch::ProtocolWindow;
use continuumd::daemon::Daemon;
use continuumd::daemon::identity::Blake3Identity;
use continuumd::protocol::envelope::{ArtifactRef, Cost, GateOutcome, NextOperation, Omission};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, VersionRange, negotiate,
};
use continuumd::protocol::operations::debug::{DebugOpenResponse, DebugStateResponse};
use continuumd::protocol::operations::repair::{RepairBeginResponse, RepairReviewResponse};
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, ArtifactHandle, CapabilityHandle, CrashpackHandle, DebugHandle, DiffHandle,
    EvidenceHandle, Opaque, ProtocolVersion, RepairHandle, RequestId, Timestamp,
};
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, GateName, GateProfile, GateStatus, OmissionReason,
    ResultStatus,
};
use continuumd::transport::{LocalPair, Server};

const NOW: &str = "2026-08-01T00:00:00.000Z";

/// Every operation this bone's `debug`/`repair` commands drive, with the command that
/// drives it.
///
/// A table so the tests below sweep all four rather than asserting one and trusting the
/// rest. The strings are read from `continuum_cli`'s own public constants, so a command
/// that changed which operation it sends would fail here rather than silently pass.
const DRIVEN: [&str; 4] = [
    debug::OPEN_OPERATION,
    debug::STATE_OPERATION,
    repair::BEGIN_OPERATION,
    repair::REVIEW_OPERATION,
];

// --- fixture, duplicated locally (see the module doc) --------------------------------------

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 2)
}

fn actor(name: &str) -> ActorId {
    ActorId::new(name).expect("a well-formed actor identity")
}

fn capability(name: &str) -> CapabilityHandle {
    CapabilityHandle::new(name).expect("a well-formed capability handle")
}

fn hello() -> ClientHello {
    ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuum-cli-evidence".to_owned(),
        actor: actor("service:continuumd"),
        capability: capability("cap_root"),
        features: Optional::Absent,
    }
}

fn root_grant() -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: capability("cap_root"),
        actor: actor("service:continuumd"),
        level: AuthorityLevel::Promote,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: 4,
        profile: Optional::Present(CapabilityProfile {
            privileged_operations: Vec::new(),
            denied_operations: Vec::new(),
            data_grants: Vec::new(),
            cross_principal_sharing: true,
        }),
    }
}

/// A deployment serving no families at all — see the module doc for why that is the point.
struct Fixture {
    server: Server,
    pair: LocalPair,
}

impl Fixture {
    fn fresh() -> Self {
        let hello = hello();
        let negotiated = negotiate(
            &[
                ProtocolVersion::new(3, 0),
                ProtocolVersion::new(3, 1),
                version(),
            ],
            ProtocolWindow::new(3),
            ENCODINGS,
            &hello,
        )
        .expect("3.2 is served");
        let daemon = Daemon::builder(Blake3Identity, negotiated, capability("cap_root"))
            .now(Timestamp::new(NOW).expect("a timestamp"))
            .capability(root_grant(), None)
            .build();
        Self {
            server: Server::new(daemon, negotiated),
            pair: LocalPair::new(),
        }
    }

    fn link(&mut self) -> LocalLink<'_> {
        LocalLink::new(&mut self.server, &mut self.pair)
    }
}

fn connection() -> Connection {
    Connection::new(
        version(),
        actor("service:continuumd"),
        capability("cap_root"),
    )
}

fn subject() -> ArtifactHandle {
    ArtifactHandle::new("crash_die_hard_1").expect("a well-formed artifact handle")
}

fn open_args(observer: Option<&str>) -> debug::OpenArgs {
    debug::OpenArgs {
        subject: subject(),
        observer: observer.map(str::to_owned),
    }
}

fn state_args() -> debug::StateArgs {
    debug::StateArgs {
        branch: DebugHandle::new("dbg_branch01").expect("a well-formed debug handle"),
        observer: None,
    }
}

fn begin_args() -> repair::BeginArgs {
    repair::BeginArgs {
        failure: CrashpackHandle::new("crash_die_hard_1").expect("a well-formed crashpack handle"),
        gate_profile: GateProfile::PhaseB,
    }
}

fn review_args() -> repair::ReviewArgs {
    repair::ReviewArgs {
        repair: RepairHandle::new("rt_transaction1").expect("a well-formed repair handle"),
    }
}

/// Drive all four commands once each against `fixture`, returning their renderings in
/// `format`, in [`DRIVEN`] order.
fn drive(fixture: &mut Fixture, format: Format) -> Vec<continuum_cli::render::Rendered> {
    let mut rendered = Vec::new();
    let mut connection = connection();
    {
        let mut link = fixture.link();
        rendered.push(
            debug::open(&mut connection, &mut link, &open_args(None), format)
                .expect("the call reaches a real frame and a real answer"),
        );
    }
    {
        let mut link = fixture.link();
        rendered.push(
            debug::state(&mut connection, &mut link, &state_args(), format)
                .expect("the call reaches a real frame and a real answer"),
        );
    }
    {
        let mut link = fixture.link();
        rendered.push(
            repair::begin(&mut connection, &mut link, &begin_args(), format)
                .expect("the call reaches a real frame and a real answer"),
        );
    }
    {
        let mut link = fixture.link();
        rendered.push(
            repair::review(&mut connection, &mut link, &review_args(), format)
                .expect("the call reaches a real frame and a real answer"),
        );
    }
    rendered
}

// --- the wire fact --------------------------------------------------------------------------

#[test]
fn every_unserved_operation_answers_the_protocols_own_unsupported_code() {
    // The four operations are in the registry — so this is a *served-ness* gap and not a
    // typo — and the daemon answers each with `UnsupportedSemanticFeature`, which is `rule
    // errors.unsupported_surface`'s own code, inside every operation's error union as of
    // protocol 3.2 (`rule errors.common`).
    for operation in DRIVEN {
        assert!(
            registry::operation(operation).is_some(),
            "{operation} is a registered operation, so an unsupported answer is about the \
             deployment and not about the name"
        );
    }

    let mut fixture = Fixture::fresh();
    let mut connection = connection();

    let outcomes: [(&str, ErrorCode); 4] = [
        (debug::OPEN_OPERATION, {
            let mut link = fixture.link();
            code(
                &connection
                    .debug_open(&mut link, &open_request())
                    .expect("the call is answered"),
            )
        }),
        (debug::STATE_OPERATION, {
            let mut link = fixture.link();
            code(
                &connection
                    .debug_state(&mut link, &state_request())
                    .expect("the call is answered"),
            )
        }),
        (repair::BEGIN_OPERATION, {
            let mut link = fixture.link();
            code(
                &connection
                    .repair_begin(&mut link, &begin_request())
                    .expect("the call is answered"),
            )
        }),
        (repair::REVIEW_OPERATION, {
            let mut link = fixture.link();
            code(
                &connection
                    .repair_review(&mut link, &review_request())
                    .expect("the call is answered"),
            )
        }),
    ];
    for (operation, answered) in outcomes {
        assert_eq!(
            answered,
            ErrorCode::UnsupportedSemanticFeature,
            "{operation} must answer the typed unsupported code, never a bare failure"
        );
        assert_eq!(
            Depth::of_code(answered),
            Depth::Unsupported,
            "{operation}: the depth is read off the daemon's code, never asserted"
        );
    }
}

fn code<T>(outcome: &Outcome<T>) -> ErrorCode {
    match outcome {
        Outcome::Refused(refusal) => refusal.code,
        Outcome::Admitted(_) => panic!("no family serves this operation in this deployment"),
    }
}

fn open_request() -> continuumd::protocol::operations::debug::DebugOpenRequest {
    continuumd::protocol::operations::debug::DebugOpenRequest {
        subject: subject(),
        observer: Optional::Absent,
    }
}

fn state_request() -> continuumd::protocol::operations::debug::DebugStateRequest {
    continuumd::protocol::operations::debug::DebugStateRequest {
        branch: state_args().branch,
        observer: Optional::Absent,
    }
}

fn begin_request() -> continuumd::protocol::operations::repair::RepairBeginRequest {
    continuumd::protocol::operations::repair::RepairBeginRequest {
        failure: begin_args().failure,
        gate_profile: GateProfile::PhaseB,
    }
}

fn review_request() -> continuumd::protocol::operations::repair::RepairReviewRequest {
    continuumd::protocol::operations::repair::RepairReviewRequest {
        repair: review_args().repair,
    }
}

// --- the rendering ----------------------------------------------------------------------------

#[test]
fn the_unsupported_depth_and_the_operation_are_named_in_every_format() {
    let mut fixture = Fixture::fresh();
    for format in [Format::Text, Format::Pretty, Format::Json] {
        for (rendered, operation) in drive(&mut fixture, format).iter().zip(DRIVEN) {
            assert!(
                rendered.text.contains(Depth::Unsupported.token()),
                "{format:?} names the depth for {operation}:\n{}",
                rendered.text
            );
            assert!(
                rendered.text.contains(operation),
                "{format:?} names exactly which operation is unsupported:\n{}",
                rendered.text
            );
            assert!(
                rendered
                    .text
                    .contains(ErrorCode::UnsupportedSemanticFeature.as_wire()),
                "{format:?} carries the protocol's own wire token, not a synonym, for \
                 {operation}:\n{}",
                rendered.text
            );
        }
    }
}

#[test]
fn an_unserved_surface_is_a_rendered_answer_and_never_a_cli_error() {
    // Exit `1` — a refusal, the same code every other typed `no` earns — and a body, rather
    // than the exit `2` a connection failure earns or a panic. The machine-readable
    // distinction between "this request was declined" and "this deployment serves no such
    // operation" is the `depth` token, not the number.
    let mut fixture = Fixture::fresh();
    for rendered in drive(&mut fixture, Format::Text) {
        assert_eq!(rendered.exit_code, 1, "{}", rendered.text);
        assert!(!rendered.text.is_empty());
    }
}

#[test]
fn an_unsupported_answer_carries_no_response_body_in_machine_output() {
    // Nothing is fabricated: every response key the success arm would fill is present and
    // `null`. Present, so a parser has one key set to read whichever arm it got; `null`, so
    // there is no value to mistake for an answer.
    let mut fixture = Fixture::fresh();
    let rendered = drive(&mut fixture, Format::Json);
    let expected: [&[&str]; 4] = [
        &[
            "\"branch\":null",
            "\"frontier\":null",
            "\"frontier_bytes\":null",
        ],
        &["\"state\":null", "\"state_bytes\":null"],
        &["\"repair\":null"],
        &[
            "\"semantic_diff\":null",
            "\"gates\":null",
            "\"evidence\":null",
        ],
    ];
    for ((rendered, keys), operation) in rendered.iter().zip(expected).zip(DRIVEN) {
        for key in keys {
            assert!(
                rendered.text.contains(key),
                "{operation}: {key} is present and null:\n{}",
                rendered.text
            );
        }
        assert!(
            rendered.text.contains("\"depth\":\"unsupported\""),
            "{operation}:\n{}",
            rendered.text
        );
        assert!(
            rendered
                .text
                .contains(&format!("\"operation\":\"{operation}\"")),
            "{operation}:\n{}",
            rendered.text
        );
    }
}

#[test]
fn the_json_envelope_always_carries_the_omissions_key_on_the_unsupported_arm() {
    let mut fixture = Fixture::fresh();
    for rendered in drive(&mut fixture, Format::Json) {
        assert!(
            rendered.text.contains("\"omissions\":["),
            "the INV-007 manifest key is present on every arm:\n{}",
            rendered.text
        );
        assert!(
            rendered.text.contains("\"advice\":[]"),
            "the conventions doc's envelope key is present too:\n{}",
            rendered.text
        );
    }
}

/// The whole `debug open --json` document, byte for byte, on the arm this deployment
/// reaches.
///
/// Pinned in full rather than by `contains`, because the acceptance criterion is the *shape*
/// of the machine answer and a substring assertion cannot see a key that was added, a key
/// that was dropped, or an ordering that stopped being canonical.
#[test]
fn the_unsupported_json_document_is_pinned_byte_for_byte() {
    let mut fixture = Fixture::fresh();
    let mut connection = connection();
    let mut link = fixture.link();
    let rendered = debug::open(&mut connection, &mut link, &open_args(None), Format::Json)
        .expect("the call is answered");
    assert_eq!(
        rendered.text,
        concat!(
            r#"{"advice":[],"branch":null,"depth":"unsupported","#,
            r#""error":{"code":"UnsupportedSemanticFeature","continuation":null,"#,
            r#""detail":"the request body is not the shape this operation declares","#,
            r#""non_resumable_reason":null,"recovery":0,"retryable":false},"#,
            r#""frontier":null,"frontier_bytes":null,"observer":null,"omissions":[],"#,
            r#""operation":"debug.open","subject":"crash_die_hard_1"}"#,
            "\n"
        ),
        "the machine envelope is canonical JSON with a fixed key set"
    );
    assert_eq!(rendered.exit_code, 1);
}

/// The whole `debug open` text rendering, line for line.
#[test]
fn the_unsupported_text_rendering_is_pinned_line_for_line() {
    let mut fixture = Fixture::fresh();
    let mut connection = connection();
    let mut link = fixture.link();
    let rendered = debug::open(&mut connection, &mut link, &open_args(None), Format::Text)
        .expect("the call is answered");
    assert_eq!(
        rendered.text,
        concat!(
            "operation  debug.open\n",
            "depth  unsupported\n",
            "subject  crash_die_hard_1\n",
            "observer  none\n",
            "error.code  UnsupportedSemanticFeature\n",
            "error.detail  the request body is not the shape this operation declares\n",
            "error.retryable  false\n",
            "error.recovery  0\n",
            "error.continuation  none\n",
            "error.non_resumable_reason  none\n",
            "omissions  0\n",
            "next_operations  0\n",
        )
    );
}

/// The whole `repair review` text rendering, line for line — the second of the two
/// commands whose response key set differs, pinned so a change to either is visible.
#[test]
fn the_unsupported_repair_review_text_rendering_is_pinned_line_for_line() {
    let mut fixture = Fixture::fresh();
    let mut connection = connection();
    let mut link = fixture.link();
    let rendered = repair::review(&mut connection, &mut link, &review_args(), Format::Text)
        .expect("the call is answered");
    assert_eq!(
        rendered.text,
        concat!(
            "operation  repair.review\n",
            "depth  unsupported\n",
            "repair  rt_transaction1\n",
            "error.code  UnsupportedSemanticFeature\n",
            "error.detail  the request body is not the shape this operation declares\n",
            "error.retryable  false\n",
            "error.recovery  0\n",
            "error.continuation  none\n",
            "error.non_resumable_reason  none\n",
            "omissions  0\n",
            "next_operations  0\n",
        )
    );
    assert_eq!(rendered.exit_code, 1);
}

/// `pretty` differs from `text` by exactly one line — the command's own name — and by
/// nothing else. INV-007's manifest and the typed reason are in both, because there is no
/// format in which they are optional.
#[test]
fn pretty_adds_the_command_name_and_removes_nothing() {
    let mut fixture = Fixture::fresh();
    let text = {
        let mut connection = connection();
        let mut link = fixture.link();
        debug::open(&mut connection, &mut link, &open_args(None), Format::Text)
            .expect("the call is answered")
    };
    let pretty = {
        let mut connection = connection();
        let mut link = fixture.link();
        debug::open(&mut connection, &mut link, &open_args(None), Format::Pretty)
            .expect("the call is answered")
    };
    assert_eq!(pretty.text, format!("command  debug open\n{}", text.text));
    assert_eq!(pretty.exit_code, text.exit_code);
}

// --- adversarial input ----------------------------------------------------------------------

/// A 64 KiB observer string — built by one `repeat`, so the fixture is linear in the size it
/// names and no test doubles a buffer to reach it.
#[test]
fn a_large_observer_string_is_still_a_typed_answer() {
    const OBSERVER_BYTES: usize = 64 * 1024;
    let observer = "o".repeat(OBSERVER_BYTES);
    assert_eq!(observer.len(), OBSERVER_BYTES);

    let mut fixture = Fixture::fresh();
    let mut connection = connection();
    let mut link = fixture.link();
    let rendered = debug::open(
        &mut connection,
        &mut link,
        &open_args(Some(&observer)),
        Format::Json,
    )
    .expect("a large argument is encoded, sent, and answered like any other");
    assert_eq!(rendered.exit_code, 1);
    assert!(rendered.text.contains("\"depth\":\"unsupported\""));
    // The observer travelled verbatim and came back in the echoed request, undamaged and
    // untruncated: a projection that silently shortened it would be inventing content.
    assert!(rendered.text.contains(&observer));
}

// --- INV-002: no command reaches the wire without its handle ----------------------------------

/// A transport that fails every exchange, so any test asserting a usage error also asserts
/// that no frame was written: if a command reached the wire, the error would be a
/// `Connection` (exit `2`) rather than a `Usage` (exit `1`).
struct Deaf;

impl Transport for Deaf {
    fn exchange(&mut self, _frame: &[u8]) -> Result<Vec<u8>, LinkError> {
        Err(LinkError::NoAnswer)
    }
}

fn args(words: &[&str]) -> Vec<String> {
    let mut line: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
    line.extend(
        ["--actor", "service:continuumd", "--capability", "cap_root"]
            .iter()
            .map(|word| (*word).to_owned()),
    );
    line
}

#[test]
fn no_command_in_this_group_reaches_the_wire_without_its_handle() {
    let lines: [&[&str]; 5] = [
        &["debug", "open"],
        &["debug", "state"],
        &["repair", "begin", "--gate-profile", "phase-b"],
        &["repair", "review"],
        &["evidence", "show"],
    ];
    for line in lines {
        let mut transport = Deaf;
        let error = cli::run(&args(line), &mut transport, false)
            .expect_err("a command with no handle never reaches the wire");
        assert_eq!(
            error.exit_code(),
            1,
            "{line:?} is a usage error, not a wire failure"
        );
    }
}

#[test]
fn a_handle_of_the_wrong_class_is_a_usage_error_and_never_reaches_the_wire() {
    // Each line names a well-formed handle of the *wrong* class for the command. The
    // scalar's own pattern refuses it, so the mistake is caught before an envelope exists —
    // which is what keeps a mis-typed handle from becoming a daemon-side refusal a caller
    // would have to interpret.
    let lines: [&[&str]; 4] = [
        &["debug", "state", "rt_transaction1"],
        &[
            "repair",
            "begin",
            "rt_transaction1",
            "--gate-profile",
            "default",
        ],
        &["repair", "review", "crash_die_hard_1"],
        &["evidence", "show", "dbg_branch01"],
    ];
    for line in lines {
        let mut transport = Deaf;
        let error = cli::run(&args(line), &mut transport, false)
            .expect_err("a handle of the wrong class never reaches the wire");
        assert_eq!(error.exit_code(), 1, "{line:?}");
    }
}

#[test]
fn repair_begin_requires_an_explicit_gate_profile() {
    // The IDL declares `gate_profile` required and the vocabulary has a member spelled
    // `default`; those are different things, and filling the field in would be the CLI
    // choosing which gates a repair is held to.
    let mut transport = Deaf;
    let error = cli::run(
        &args(&["repair", "begin", "crash_die_hard_1"]),
        &mut transport,
        false,
    )
    .expect_err("the profile is not defaulted");
    assert_eq!(error.exit_code(), 1);

    let mut transport = Deaf;
    let error = cli::run(
        &args(&[
            "repair",
            "begin",
            "crash_die_hard_1",
            "--gate-profile",
            "phase-z",
        ]),
        &mut transport,
        false,
    )
    .expect_err("an unknown profile is a usage error, not a wire call");
    assert_eq!(error.exit_code(), 1);
}

// --- the success arms, proven before the families land ----------------------------------------

fn admitted<T>(payload: T, omissions: Vec<Omission>) -> Outcome<T> {
    Outcome::Admitted(Admitted {
        request_id: RequestId::new("req_cli000001").expect("a request id"),
        status: ResultStatus::Ok,
        payload,
        verdict: None,
        assurance: None,
        task: None,
        continuation: None,
        omissions,
        artifacts: Vec::<ArtifactRef>::new(),
        cost: Cost {
            wall_ms: Optional::Absent,
            cpu_ms: Optional::Absent,
            memory_bytes: Optional::Absent,
            states: Optional::Absent,
            solver_ms: Optional::Absent,
            proof_ms: Optional::Absent,
            tokens: Optional::Absent,
            candidates: Optional::Absent,
            bytes: Optional::Absent,
            tokenizer_id: Optional::Absent,
        },
        next_operations: Vec::<NextOperation>::new(),
    })
}

fn budget_omission(subject: &str) -> Omission {
    Omission {
        reason: OmissionReason::Budget,
        subject: subject.to_owned(),
        recoverable_by: Optional::Absent,
    }
}

#[test]
fn the_debug_open_renderer_prints_the_branch_and_the_frontier() {
    let outcome = admitted(
        DebugOpenResponse {
            branch: DebugHandle::new("dbg_branch01").expect("a debug handle"),
            frontier: Opaque::from_bytes(br#"{"events":["fill"],"step":3}"#.to_vec()),
        },
        vec![budget_omission("debug.frontier.depth")],
    );

    for format in [Format::Text, Format::Pretty] {
        let rendered = debug::render_open(&open_args(None), &outcome, format);
        assert_eq!(rendered.exit_code, 0, "an admitted answer exits zero");
        assert!(rendered.text.contains("depth  served"), "{}", rendered.text);
        // The handle is printed in full, never truncated.
        assert!(rendered.text.contains("dbg_branch01"), "{}", rendered.text);
        assert!(rendered.text.contains("omissions  1"), "{}", rendered.text);
        assert!(
            rendered.text.contains("debug.frontier.depth"),
            "the manifest is unabridged in {format:?}:\n{}",
            rendered.text
        );
    }

    let json = debug::render_open(&open_args(None), &outcome, Format::Json);
    assert_eq!(json.exit_code, 0);
    assert!(json.text.contains("\"depth\":\"served\""), "{}", json.text);
    // The frontier is embedded verbatim — the value itself, not a length a caller would
    // have to make a second call to resolve — with its length beside it.
    assert!(json.text.contains("\"step\":3"), "{}", json.text);
    assert!(json.text.contains("\"frontier_bytes\":28"), "{}", json.text);
    assert!(json.text.contains("\"error\":null"), "{}", json.text);
}

#[test]
fn the_debug_state_renderer_embeds_the_state_verbatim() {
    let outcome = admitted(
        DebugStateResponse {
            state: Opaque::from_bytes(br#"{"big":1,"small":2}"#.to_vec()),
        },
        Vec::new(),
    );
    let text = debug::render_state(&state_args(), &outcome, Format::Text);
    assert_eq!(text.exit_code, 0);
    assert!(text.text.contains("state.bytes  19"), "{}", text.text);
    assert!(text.text.contains("omissions  0"), "{}", text.text);

    let json = debug::render_state(&state_args(), &outcome, Format::Json);
    assert!(json.text.contains("\"small\":2"), "{}", json.text);
    assert!(json.text.contains("\"state_bytes\":19"), "{}", json.text);
}

#[test]
fn the_repair_begin_renderer_prints_the_transaction_handle() {
    let outcome = admitted(
        RepairBeginResponse {
            repair: RepairHandle::new("rt_transaction1").expect("a repair handle"),
        },
        Vec::new(),
    );
    let text = repair::render_begin(&begin_args(), &outcome, Format::Text);
    assert_eq!(text.exit_code, 0);
    assert!(
        text.text.contains("repair  rt_transaction1"),
        "{}",
        text.text
    );
    // The profile the caller chose is echoed in the protocol's own wire spelling.
    assert!(
        text.text.contains(GateProfile::PhaseB.as_wire()),
        "{}",
        text.text
    );

    let json = repair::render_begin(&begin_args(), &outcome, Format::Json);
    assert!(
        json.text.contains("\"repair\":\"rt_transaction1\""),
        "{}",
        json.text
    );
    assert!(
        json.text.contains(&format!(
            "\"gate_profile\":\"{}\"",
            GateProfile::PhaseB.as_wire()
        )),
        "{}",
        json.text
    );
}

#[test]
fn the_repair_review_renderer_prints_every_gate_by_name() {
    let gate = |name, status, evidence: &[&str]| GateOutcome {
        name,
        status,
        evidence: evidence
            .iter()
            .map(|handle| EvidenceHandle::new(handle).expect("an evidence handle"))
            .collect(),
    };
    let outcome = admitted(
        RepairReviewResponse {
            repair: RepairHandle::new("rt_transaction1").expect("a repair handle"),
            semantic_diff: Nullable::Value(
                DiffHandle::new("diff_review01").expect("a diff handle"),
            ),
            gates: vec![
                gate(GateName::BaseReplay, GateStatus::Passed, &["ev_replay01"]),
                gate(GateName::IntentIntegrity, GateStatus::Failed, &[]),
                gate(GateName::DefectMutants, GateStatus::NotYetEnforced, &[]),
            ],
            evidence: vec![EvidenceHandle::new("ev_replay01").expect("an evidence handle")],
        },
        Vec::new(),
    );

    let text = repair::render_review(&review_args(), &outcome, Format::Text);
    assert_eq!(text.exit_code, 0);
    assert!(text.text.contains("gates  3"), "{}", text.text);
    // Every gate by name and status — never "1 of 3 passed", which would tell a reviewer
    // that something is outstanding and not what.
    for name in [
        GateName::BaseReplay,
        GateName::IntentIntegrity,
        GateName::DefectMutants,
    ] {
        assert!(text.text.contains(name.as_wire()), "{}", text.text);
    }
    for status in [
        GateStatus::Passed,
        GateStatus::Failed,
        GateStatus::NotYetEnforced,
    ] {
        assert!(text.text.contains(status.as_wire()), "{}", text.text);
    }
    assert!(
        text.text.contains("semantic_diff  diff_review01"),
        "{}",
        text.text
    );
    assert!(
        text.text.contains("evidence[0]  ev_replay01"),
        "{}",
        text.text
    );

    let json = repair::render_review(&review_args(), &outcome, Format::Json);
    assert!(
        json.text.contains(
            r#""gates":[{"evidence":["ev_replay01"],"name":"base_replay","status":"passed"}"#
        ),
        "the gates travel as objects with their evidence handles in full:\n{}",
        json.text
    );
    assert!(
        json.text.contains(r#""semantic_diff":"diff_review01""#),
        "{}",
        json.text
    );
}

/// A review whose transaction has no semantic diff yet states the absence by name.
#[test]
fn an_absent_semantic_diff_is_named_rather_than_omitted() {
    let outcome = admitted(
        RepairReviewResponse {
            repair: RepairHandle::new("rt_transaction1").expect("a repair handle"),
            semantic_diff: Nullable::Null,
            gates: Vec::new(),
            evidence: Vec::new(),
        },
        Vec::new(),
    );
    let text = repair::render_review(&review_args(), &outcome, Format::Text);
    assert!(
        text.text.contains("semantic_diff  none"),
        "a clean absence is stated, never left to silence:\n{}",
        text.text
    );
    assert!(text.text.contains("gates  0"), "{}", text.text);

    let json = repair::render_review(&review_args(), &outcome, Format::Json);
    assert!(
        json.text.contains(r#""semantic_diff":null"#),
        "{}",
        json.text
    );
    assert!(json.text.contains(r#""gates":[]"#), "{}", json.text);
}
