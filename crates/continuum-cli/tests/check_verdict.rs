//! Evidence for `continuum check start|result|await` (bn-3rqvm, PR-13 first command group) —
//! the verify half, driven through `continuum_cli::check`'s own command functions over a real
//! `LocalPair` boundary against a real, provisioned `Daemon` running the TV-009 Die Hard port.
//!
//! Every verdict asserted here was produced by `continuum-engine-reference` and travelled a
//! real frame. Nothing in this file constructs a `SemanticVerdictValue`.
//!
//! # Clause → test
//!
//! - **both answer shapes, both pinned** →
//!   [`a_fresh_start_is_task_shaped_and_a_repeat_start_is_result_shaped`]. The first submit
//!   answers `task_started` with a task handle; a second submit of the same campaign under a
//!   different idempotency key answers the *cached result*, `status = ok`, with the
//!   `VerificationResult` inline and no task field. Both are rendered, and which arrived is a
//!   token derived from the body.
//! - **`task_started` names the answer shape, not an execution event** →
//!   [`a_task_shaped_answer_claims_nothing_about_whether_work_began`] — the rendering carries
//!   the status token and the handle and no word about starting, and the *suspended* variant
//!   prints the continuation rather than waiting on it.
//! - **the verdict, verbatim, with INV-008's typed reason** →
//!   [`a_closed_campaign_renders_the_engines_own_refutation`] and
//!   [`a_bounded_campaign_renders_inconclusive_with_the_typed_reason_never_a_verdict_of_its_own`].
//!   The second is the INV-008 case: budget exhaustion arrives as `inconclusive` +
//!   `ResourceExhausted` + `bounded`, and every one of those three tokens survives rendering
//!   in all three formats.
//! - **the nine-dimension assurance envelope, unabridged** →
//!   [`every_assurance_dimension_names_an_engine_or_a_typed_unsupported_reason`].
//! - **the bounded wait never becomes an unbounded one** →
//!   [`await_answers_within_the_callers_own_bound_and_prints_the_continuation`].
//! - **refusal path** → [`a_result_read_of_a_task_this_daemon_does_not_hold_is_a_typed_denial`].
//! - **the machine contract, pinned** → [`the_denial_json_document_is_pinned_byte_for_byte`].
//! - **INV-002 explicit handles** → [`no_check_verb_reaches_the_wire_without_its_handle`].
//!
//! # The fixture
//!
//! `tests/task_lifecycle.rs`'s deployment verbatim in shape: the Die Hard port registered as
//! a model, its accepted intent, and a sealed snapshot. Duplicated locally rather than
//! imported, for the reason `continuum-mcp/tests/typed_surface.rs`'s identical comment gives:
//! a `tests/*.rs` file is its own crate.

use continuum_cli::check::{self, AnswerShape, AwaitArgs, ResultArgs, StartArgs};
use continuum_cli::contract::Exit;
use continuum_cli::format::Format;
use continuum_cli::render::Rendered;
use continuum_cli::wire::{Connection, LinkError, LocalLink, Transport};
use continuum_cli::{cli, task};
use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::publication::ContentIdentifier;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::{Blake3Identity, intent_to_wire};
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationRequest};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::workspace::WorkspaceCreateRequest;
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, EpochIdentity, IntentHandle, Opaque, OperationName,
    ProtocolVersion, RequestId, TaskHandle, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{FileComponent, SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{
    AssuranceClass, AuthorityLevel, Encoding, ErrorCode, Fragment, InconclusiveReason, Portfolio,
    ResultStatus, SemanticVerdict, TargetKind,
};
use continuumd::transport::{LocalPair, Server};

const MODULE: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");
const CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");
const CONTRACT: &str = include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");
const MODULE_PATH: &str = "DieHard.ctm";
const NOW: &str = "2026-08-01T00:00:00.000Z";

/// Below TV-009's frozen sixteen reachable states: forces a park with a continuation.
const PARK_BUDGET: u64 = 4;
/// Above it: closes the campaign outright.
const CLOSING_BUDGET: u64 = 64;

/// A well-formed `task_` handle no deployment in this file ever creates.
const ABSENT: &str = "task_neverstarted1";

/// Every operation this bone's `check` commands drive, read from the crate's own public
/// constants so a command that changed which frame it sends fails here.
const DRIVEN: [&str; 3] = [
    check::START_OPERATION,
    check::RESULT_OPERATION,
    check::AWAIT_OPERATION,
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

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
}

fn epochs() -> EpochSet {
    EpochSet {
        protocol: version(),
        semantic: Nullable::Value(epoch("semantic-1")),
        intent: Nullable::Value(epoch("intent-1")),
        evidence: Nullable::Null,
        proof: Nullable::Value(epoch("proof-1")),
        corpus: Nullable::Null,
        engine: Nullable::Value(epoch("engine-reference-1")),
    }
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

fn grant(handle: &str, who: &str, level: AuthorityLevel) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: capability(handle),
        actor: actor(who),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: 3,
        profile: Optional::Absent,
    }
}

fn root_grant() -> CapabilityDescriptor {
    CapabilityDescriptor {
        delegation_depth: 4,
        profile: Optional::Present(CapabilityProfile {
            privileged_operations: vec![OperationName::new("intent.accept").expect("a name")],
            denied_operations: Vec::new(),
            data_grants: Vec::new(),
            cross_principal_sharing: true,
        }),
        ..grant("cap_root", "service:continuumd", AuthorityLevel::Promote)
    }
}

/// One provisioned deployment behind a byte boundary, holding one sealed Die Hard snapshot.
struct Fixture {
    server: Server,
    pair: LocalPair,
    snapshot: WorkspaceHandle,
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

        let root = Some(capability("cap_root"));
        let mut daemon = Daemon::builder(Blake3Identity, negotiated, capability("cap_root"))
            .epochs(epochs())
            .now(Timestamp::new(NOW).expect("a timestamp"))
            .capability(root_grant(), None)
            .capability(
                grant("cap_builder", "agent:builder", AuthorityLevel::Propose),
                root.clone(),
            )
            .capability(
                grant("cap_runner", "agent:runner", AuthorityLevel::Execute),
                root,
            )
            .family(WorkspaceFamily)
            .family(IntentFamily)
            .family(TaskFamily)
            .family(VerificationFamily)
            .build();

        let intent = accept_intent(&mut daemon);
        let components = stage(&mut daemon, intent);
        let snapshot = seal(&mut daemon, components);

        Self {
            server: Server::new(daemon, negotiated),
            pair: LocalPair::new(),
            snapshot,
        }
    }

    fn link(&mut self) -> LocalLink<'_> {
        LocalLink::new(&mut self.server, &mut self.pair)
    }
}

fn setup_envelope(
    who: &str,
    cap: &str,
    operation: &'static str,
    request_id: &str,
    budget: Optional<Budget>,
) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request_id).expect("a well-formed request id"),
        idempotency_key: Optional::Present(format!("idem-{request_id}")),
        actor: actor(who),
        capability: capability(cap),
        operation: OperationName::new(operation).expect("a registry name"),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

fn accept_intent(daemon: &mut Daemon) -> IntentHandle {
    let contract =
        IntentContract::decode(CONTRACT.trim_end().as_bytes()).expect("the fixture decodes");
    let stored = ContentIdentifier::identify(
        &Blake3Identity,
        ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    let intent = intent_to_wire(&stored).expect("an `in_` handle");
    daemon.state_mut().put_intent(
        intent.clone(),
        IntentRecord {
            contract,
            status: RegistryStatus::Proposed,
            supersedes: None,
            superseded_by: None,
            acceptance: None,
        },
    );

    let mut fields = std::collections::BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "service:continuumd"),
        ("capability", "revise-intent"),
        ("signature", "sig-die-hard-v1"),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", NOW),
    ] {
        fields.insert(key.to_owned(), Json::String(value.to_owned()));
    }
    let outcome = daemon.dispatch(&OperationRequest {
        envelope: setup_envelope(
            "service:continuumd",
            "cap_root",
            "intent.accept",
            "req_accept",
            Optional::Absent,
        ),
        arguments: Arguments::IntentAccept(IntentAcceptRequest {
            proposal: intent.clone(),
            acceptance: Opaque::from_bytes(Json::Object(fields).to_canonical_bytes()),
            bundle: Optional::Absent,
        }),
    });
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        outcome.envelope.error
    );
    intent
}

fn stage(daemon: &mut Daemon, intent: IntentHandle) -> SnapshotComponents {
    let mut files: Vec<Commitment> = Vec::new();
    let mut placements = Vec::new();
    for (path, content) in [(MODULE_PATH, MODULE), ("README.md", "# TV-009\n")] {
        let commitment = daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new(path).expect("a workspace path"),
                content.as_bytes().to_vec(),
            )
            .expect("staging names its content");
        placements.push(FileComponent {
            path: path.to_owned(),
            commitment: commitment.clone(),
        });
        files.push(commitment);
    }
    let configuration = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("default.model.toml").expect("a workspace path"),
            CONFIG.as_bytes().to_vec(),
        )
        .expect("staging names its content");
    daemon.state_mut().models_mut().register(
        model_source(&Blake3Identity, [(MODULE_PATH, MODULE.as_bytes())])
            .expect("blake3 names the module set"),
        diehard::model().expect("the port builds"),
    );
    SnapshotComponents {
        files,
        cml_modules: Vec::new(),
        rust_extraction: Vec::new(),
        domain_packs: Vec::new(),
        dependencies: Vec::new(),
        epochs: SnapshotEpochs {
            semantic: epoch("semantic-1"),
            proof: epoch("proof-1"),
            toolchain: Optional::Absent,
        },
        intent,
        correspondence: Vec::new(),
        proof_environment: Vec::new(),
        configuration: vec![configuration],
        file_components: Optional::Present(placements),
    }
}

/// Create and seal the snapshot the campaigns run over. Fixture setup, so it is dispatched
/// directly against the `Daemon` — it is not part of this bone's `check` surface.
fn seal(daemon: &mut Daemon, components: SnapshotComponents) -> WorkspaceHandle {
    let outcome = daemon.dispatch(&OperationRequest {
        envelope: setup_envelope(
            "agent:builder",
            "cap_builder",
            "workspace.create",
            "req_create",
            Optional::Absent,
        ),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components,
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    });
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        outcome.envelope.error
    );
    let Payload::WorkspaceCreate(response) = outcome.payload else {
        panic!("a create answers a create payload");
    };
    response.snapshot
}

fn fresh_connection() -> Connection {
    Connection::new(version(), actor("agent:runner"), capability("cap_runner"))
}

fn start_args(fixture: &Fixture, states: u64) -> StartArgs {
    StartArgs {
        snapshot: fixture.snapshot.clone(),
        target: Target {
            kind: TargetKind::AllClaims,
            id: "DieHard".to_owned(),
        },
        portfolio: Portfolio::Interactive,
        priority_class: Optional::Absent,
        states,
    }
}

/// Run one command over the wire on a fresh connection, and return what it rendered.
fn run<F>(fixture: &mut Fixture, call: F) -> Rendered
where
    F: FnOnce(
        &mut Connection,
        &mut LocalLink<'_>,
    ) -> Result<Rendered, continuum_cli::error::CliError>,
{
    let mut connection = fresh_connection();
    let mut link = fixture.link();
    call(&mut connection, &mut link).expect("the call reaches a real frame and a real answer")
}

/// The value of one `key  value` line a rendering printed.
fn line(rendered: &Rendered, key: &str) -> String {
    rendered
        .text
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key}  ")))
        .unwrap_or_else(|| panic!("no {key:?} line in:\n{}", rendered.text))
        .to_owned()
}

/// Submit a campaign and return the task handle the CLI *printed*.
fn submit(fixture: &mut Fixture, states: u64) -> TaskHandle {
    let args = start_args(fixture, states);
    let rendered = run(fixture, |connection, link| {
        check::start(connection, link, &args, Format::Text)
    });
    assert_eq!(rendered.exit_code, 0, "{}", rendered.text);
    TaskHandle::new(&line(&rendered, "task")).expect("the printed handle is well formed")
}

// --- the evidence -------------------------------------------------------------------------

#[test]
fn each_verb_names_the_registry_operation_it_drives() {
    for operation in DRIVEN {
        assert!(
            registry::operation(operation).is_some(),
            "{operation} is a registered operation"
        );
    }
    assert_eq!(
        DRIVEN,
        [
            "verification.start",
            "verification.result",
            "verification.await"
        ],
        "the mapping this group documents is the mapping it sends"
    );
}

#[test]
fn a_fresh_start_is_task_shaped_and_a_repeat_start_is_result_shaped() {
    // The two answer shapes `verification.start` declares, both live, both rendered, from one
    // fixture. The second submit is the *cached* lane: the same campaign identity resolves to
    // a completed task, so the daemon answers with the result instead of a task — "present
    // instead of `task` when a cached result is returned".
    let mut fixture = Fixture::fresh();
    let args = start_args(&fixture, CLOSING_BUDGET);

    let first = run(&mut fixture, |connection, link| {
        check::start(connection, link, &args, Format::Text)
    });
    assert_eq!(first.exit_code, 0, "{}", first.text);
    assert_eq!(line(&first, "answer_shape"), AnswerShape::Task.token());
    assert_eq!(line(&first, "status"), ResultStatus::TaskStarted.as_wire());
    assert!(line(&first, "task").starts_with("task_"), "{}", first.text);
    assert_eq!(
        line(&first, "result.task"),
        "none",
        "a task-shaped answer carries no result body, and says so:\n{}",
        first.text
    );
    // `verification.start` declares no `verdict` clause, so the answer carries none — stated
    // rather than left blank, because a caller must not read an absent verdict as a verdict.
    assert_eq!(line(&first, "verdict.kind"), "none");

    // A second submit under a distinct idempotency key: same campaign identity, completed, so
    // the cached lane answers.
    let second = {
        let mut connection =
            fresh_connection().with_idempotency_key(Some("second-submit".to_owned()));
        let mut link = fixture.link();
        check::start(&mut connection, &mut link, &args, Format::Text).expect("the call is answered")
    };
    assert_eq!(second.exit_code, 0, "{}", second.text);
    assert_eq!(line(&second, "answer_shape"), AnswerShape::Result.token());
    assert_eq!(line(&second, "status"), ResultStatus::Ok.as_wire());
    assert_eq!(
        line(&second, "task"),
        "none",
        "the cached lane answers with a result, not a task:\n{}",
        second.text
    );
    assert_eq!(line(&second, "result.task"), line(&first, "task"));
    assert_eq!(
        line(&second, "result.target.kind"),
        TargetKind::AllClaims.as_wire()
    );
    assert_eq!(
        line(&second, "result.fragments"),
        "1",
        "the campaign covers the Finite fragment and claims nothing about the other five"
    );
    assert!(
        second.text.contains(&format!(
            "result.fragments[0]  {}",
            Fragment::Finite.as_wire()
        )),
        "{}",
        second.text
    );
    // …and the five it does not cover are in the INV-007 manifest beside it, never implied.
    for uncovered in [
        Fragment::Symbolic,
        Fragment::Temporal,
        Fragment::Probabilistic,
        Fragment::Theorem,
        Fragment::Runtime,
    ] {
        assert!(
            second.text.contains(uncovered.as_wire()),
            "{} is named in the manifest:\n{}",
            uncovered.as_wire(),
            second.text
        );
    }

    // Both shapes are pinned in the machine channel too.
    let mut fixture = Fixture::fresh();
    let json_first = run(&mut fixture, |connection, link| {
        check::start(connection, link, &args, Format::Json)
    });
    assert!(
        json_first.text.contains("\"answer_shape\":\"task-shaped\""),
        "{}",
        json_first.text
    );
    assert!(
        json_first.text.contains("\"result\":null"),
        "{}",
        json_first.text
    );
    let json_second = {
        let mut connection =
            fresh_connection().with_idempotency_key(Some("second-submit".to_owned()));
        let mut link = fixture.link();
        check::start(&mut connection, &mut link, &args, Format::Json).expect("the call is answered")
    };
    assert!(
        json_second
            .text
            .contains("\"answer_shape\":\"result-shaped\""),
        "{}",
        json_second.text
    );
    assert!(
        json_second.text.contains("\"task\":null"),
        "{}",
        json_second.text
    );
    assert!(
        json_second.text.contains("\"fragments\":[\"Finite\"]"),
        "{}",
        json_second.text
    );
}

#[test]
fn a_task_shaped_answer_claims_nothing_about_whether_work_began() {
    // RFC 0026's A12 disposition: `task_started` means "the answer is task-shaped, not
    // result-shaped", never "new work began at this call". So the rendering carries the
    // status token and the handle and no word about starting, running, or launching — and
    // the parked variant prints its continuation rather than waiting on it, which is the
    // "never silently wait forever" half of the same decision.
    let mut fixture = Fixture::fresh();
    let args = start_args(&fixture, PARK_BUDGET);
    let rendered = run(&mut fixture, |connection, link| {
        check::start(connection, link, &args, Format::Text)
    });
    assert_eq!(rendered.exit_code, 0, "{}", rendered.text);
    assert_eq!(
        line(&rendered, "status"),
        ResultStatus::TaskSuspended.as_wire()
    );
    assert!(
        line(&rendered, "continuation").starts_with("cont_"),
        "a suspension prints the typed continuation:\n{}",
        rendered.text
    );
    for claim in ["started work", "running", "in progress", "launched"] {
        assert!(
            !rendered.text.to_lowercase().contains(claim),
            "the rendering claims {claim:?} about execution:\n{}",
            rendered.text
        );
    }
}

#[test]
fn a_closed_campaign_renders_the_engines_own_refutation() {
    let mut fixture = Fixture::fresh();
    let task = submit(&mut fixture, CLOSING_BUDGET);
    let args = ResultArgs { task };

    for format in [Format::Text, Format::Pretty] {
        let mut connection = fresh_connection();
        let mut link = fixture.link();
        let rendered =
            check::result(&mut connection, &mut link, &args, format).expect("the call is answered");
        // `3`, not `0`: the daemon answered and the answer is **no**. bn-ybh1z flipped this
        // pin deliberately — a refuted campaign exiting `0` is what makes
        // `continuum check result … && deploy` deploy a refuted build
        // (`continuum_cli::contract`, "the exit-code taxonomy").
        assert_eq!(
            rendered.exit_code,
            Exit::Refuted.code(),
            "{format:?}:\n{}",
            rendered.text
        );
        assert_eq!(line(&rendered, "verdict.kind"), "semantic");
        assert_eq!(
            line(&rendered, "verdict.verdict"),
            SemanticVerdict::Refuted.as_wire()
        );
        assert_eq!(
            line(&rendered, "verdict.inconclusive_reason"),
            "none",
            "a decided verdict carries no INV-008 reason, and says so"
        );
        assert_eq!(
            line(&rendered, "verdict.assurance_class"),
            AssuranceClass::Validated.as_wire()
        );
        assert_eq!(line(&rendered, "result.continuation"), "none");
    }

    let mut connection = fresh_connection();
    let mut link = fixture.link();
    let json = check::result(&mut connection, &mut link, &args, Format::Json)
        .expect("the call is answered");
    assert!(json.text.contains("\"depth\":\"served\""), "{}", json.text);
    assert!(
        json.text.contains(&format!(
            "\"verdict\":\"{}\"",
            SemanticVerdict::Refuted.as_wire()
        )),
        "{}",
        json.text
    );
    assert!(
        json.text.contains("\"inconclusive_reason\":null"),
        "{}",
        json.text
    );
}

#[test]
fn a_bounded_campaign_renders_inconclusive_with_the_typed_reason_never_a_verdict_of_its_own() {
    // INV-008, end to end. Budget exhaustion is *not* a verdict: it arrives as `inconclusive`
    // with the engine's own typed `ResourceExhausted` beside it, and the rendering carries all
    // three tokens — verdict, reason, assurance class — in every format, without re-deriving
    // any of them. Nothing in `continuum_cli` computes "the budget was small, so it must be
    // exhausted"; the reason is read off the answer.
    let mut fixture = Fixture::fresh();
    let task = submit(&mut fixture, PARK_BUDGET);
    let args = ResultArgs { task };

    for format in [Format::Text, Format::Pretty, Format::Json] {
        let mut connection = fresh_connection();
        let mut link = fixture.link();
        let rendered =
            check::result(&mut connection, &mut link, &args, format).expect("the call is answered");
        // `4`: INV-008's typed unknown is neither a yes nor a no, and the taxonomy gives it
        // a code of its own so a caller need not parse the verdict to tell (bn-ybh1z).
        assert_eq!(
            rendered.exit_code,
            Exit::Inconclusive.code(),
            "{format:?}:\n{}",
            rendered.text
        );
        for token in [
            SemanticVerdict::Inconclusive.as_wire(),
            InconclusiveReason::ResourceExhausted.as_wire(),
            AssuranceClass::Bounded.as_wire(),
        ] {
            assert!(
                rendered.text.contains(token),
                "{token} survives rendering in {format:?}:\n{}",
                rendered.text
            );
        }
        // Budget exhaustion never appears as the verdict itself.
        assert!(
            !rendered.text.contains("\"verdict\":\"BudgetExhausted\""),
            "{}",
            rendered.text
        );
    }

    let mut connection = fresh_connection();
    let mut link = fixture.link();
    let text = check::result(&mut connection, &mut link, &args, Format::Text)
        .expect("the call is answered");
    assert_eq!(
        line(&text, "verdict.inconclusive_reason"),
        InconclusiveReason::ResourceExhausted.as_wire()
    );
    assert!(
        line(&text, "result.continuation").starts_with("cont_"),
        "a bounded result names the continuation its frontier is parked behind:\n{}",
        text.text
    );
}

#[test]
fn every_assurance_dimension_names_an_engine_or_a_typed_unsupported_reason() {
    // `rule envelope.assurance_required`: all nine dimensions, each naming a producing engine
    // or a typed `Unsupported(reason)`. "Hiding uncertainty to save tokens is prohibited", so
    // this projection prints the six unsupported dimensions as loudly as the three produced
    // ones.
    let mut fixture = Fixture::fresh();
    let task = submit(&mut fixture, CLOSING_BUDGET);
    let args = ResultArgs { task };

    let mut connection = fresh_connection();
    let mut link = fixture.link();
    let text = check::result(&mut connection, &mut link, &args, Format::Text)
        .expect("the call is answered");
    assert_eq!(line(&text, "assurance"), "9");
    for dimension in [
        "bounds",
        "faults",
        "fairness",
        "values",
        "schedules",
        "memory_model",
        "observer",
        "proof_status",
        "unknowns",
    ] {
        let kind = line(&text, &format!("assurance.{dimension}.kind"));
        assert!(
            kind == "produced" || kind == "unsupported",
            "{dimension} is one of the two union members: {kind}"
        );
        if kind == "produced" {
            assert_ne!(
                line(&text, &format!("assurance.{dimension}.engine")),
                "none"
            );
            assert_eq!(
                line(&text, &format!("assurance.{dimension}.reason")),
                "none"
            );
        } else {
            assert_eq!(
                line(&text, &format!("assurance.{dimension}.engine")),
                "none"
            );
            assert_ne!(
                line(&text, &format!("assurance.{dimension}.reason")),
                "none"
            );
        }
    }
    assert!(
        text.text.contains("no-fault-model"),
        "an unsupported dimension's typed reason travels verbatim:\n{}",
        text.text
    );

    let mut connection = fresh_connection();
    let mut link = fixture.link();
    let json = check::result(&mut connection, &mut link, &args, Format::Json)
        .expect("the call is answered");
    assert!(
        json.text
            .contains(r#""faults":{"engine":null,"kind":"unsupported","reason":"no-fault-model","summary":null}"#),
        "each dimension is an object with one uniform key set:\n{}",
        json.text
    );
}

#[test]
fn await_answers_within_the_callers_own_bound_and_prints_the_continuation() {
    // `verification.await` is the protocol's bounded wait, and the bound is the caller's. A
    // parked campaign answers `task_suspended` with the continuation, so the command that
    // *could* have waited forever instead prints what a caller needs to resume — and the
    // verdict is unchanged by the waiting, exactly as the IDL says.
    let mut fixture = Fixture::fresh();
    let task = submit(&mut fixture, PARK_BUDGET);
    let awaited = AwaitArgs {
        task: task.clone(),
        timeout_ms: 25,
    };
    let mut connection = fresh_connection();
    let mut link = fixture.link();
    let rendered = check::await_result(&mut connection, &mut link, &awaited, Format::Text)
        .expect("the call is answered");
    // Waiting never changes a verdict, and the exit code is the verdict's: a parked campaign
    // is inconclusive, so the wait that reported it exits `4` (bn-ybh1z).
    assert_eq!(
        rendered.exit_code,
        Exit::Inconclusive.code(),
        "{}",
        rendered.text
    );
    assert_eq!(line(&rendered, "operation"), check::AWAIT_OPERATION);
    assert_eq!(line(&rendered, "timeout_ms"), "25");
    assert_eq!(
        line(&rendered, "status"),
        ResultStatus::TaskSuspended.as_wire()
    );
    assert!(
        line(&rendered, "continuation").starts_with("cont_"),
        "{}",
        rendered.text
    );

    // Waiting never changes a verdict: the same verdict `check result` reads is the one
    // `check await` reports.
    let mut connection = fresh_connection();
    let mut link = fixture.link();
    let read = check::result(
        &mut connection,
        &mut link,
        &ResultArgs { task },
        Format::Text,
    )
    .expect("the call is answered");
    assert_eq!(
        line(&read, "verdict.verdict"),
        line(&rendered, "verdict.verdict")
    );
    assert_eq!(
        line(&read, "verdict.inconclusive_reason"),
        line(&rendered, "verdict.inconclusive_reason")
    );
}

#[test]
fn a_result_read_of_a_task_this_daemon_does_not_hold_is_a_typed_denial() {
    let mut fixture = Fixture::fresh();
    let args = ResultArgs {
        task: TaskHandle::new(ABSENT).expect("a well-formed task handle"),
    };
    for format in [Format::Text, Format::Pretty, Format::Json] {
        let mut connection = fresh_connection();
        let mut link = fixture.link();
        let rendered =
            check::result(&mut connection, &mut link, &args, format).expect("the call is answered");
        assert_eq!(rendered.exit_code, 1, "{format:?}:\n{}", rendered.text);
        assert!(
            rendered
                .text
                .contains(ErrorCode::CapabilityDenied.as_wire()),
            "{format:?}:\n{}",
            rendered.text
        );
        for oracle in ["not found", "not-found", "no such", "does not exist"] {
            assert!(
                !rendered.text.to_lowercase().contains(oracle),
                "an existence oracle leaked into {format:?}:\n{}",
                rendered.text
            );
        }
    }
}

/// The whole `check result --json` document for a denial, byte for byte.
#[test]
fn the_denial_json_document_is_pinned_byte_for_byte() {
    let mut fixture = Fixture::fresh();
    let args = ResultArgs {
        task: TaskHandle::new(ABSENT).expect("a well-formed task handle"),
    };
    let mut connection = fresh_connection();
    let mut link = fixture.link();
    let rendered = check::result(&mut connection, &mut link, &args, Format::Json)
        .expect("the call is answered");
    assert_eq!(
        rendered.text,
        concat!(
            r#"{"advice":[],"artifacts":[],"assurance":null,"continuation":null,"#,
            r#""cost":null,"depth":"refused","#,
            r#""error":{"code":"CapabilityDenied","continuation":null,"#,
            r#""detail":"the presented capability does not admit this operation","#,
            r#""non_resumable_reason":null,"recovery":[],"retryable":false},"#,
            r#""next_operations":[],"omissions":[],"operation":"verification.result","#,
            r#""request_id":"req_cli000001","result":null,"#,
            r#""status":null,"task":"task_neverstarted1","verdict":null}"#,
            "\n"
        ),
        "the machine envelope is canonical JSON with a fixed key set"
    );
    assert_eq!(rendered.exit_code, 1);
}

// --- INV-002: no verb reaches the wire without its handle -------------------------------------

struct Deaf;

impl Transport for Deaf {
    fn exchange(&mut self, _frame: &[u8]) -> Result<Vec<u8>, LinkError> {
        Err(LinkError::NoAnswer)
    }
}

fn args(words: &[&str]) -> Vec<String> {
    let mut line: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
    line.extend(
        ["--actor", "agent:runner", "--capability", "cap_runner"]
            .iter()
            .map(|word| (*word).to_owned()),
    );
    line
}

#[test]
fn no_check_verb_reaches_the_wire_without_its_handle() {
    let lines: [&[&str]; 5] = [
        &["check", "start"],
        &["check", "result"],
        &["check", "await"],
        // A handle without the ceilings the protocol requires: `verification.start` is
        // `@task_starting`, so a budget travels, and this crate invents none.
        &["check", "start", "ws_abc123", "--target", "DieHard"],
        &["check", "await", "task_abc123"],
    ];
    for line in lines {
        let mut transport = Deaf;
        let error = cli::run(&args(line), &mut transport, false)
            .expect_err("an incomplete command never reaches the wire");
        assert_eq!(
            error.exit_code(),
            1,
            "{line:?} is a usage error, not a wire failure"
        );
    }
}

#[test]
fn an_unknown_vocabulary_member_is_a_usage_error_that_names_the_members_that_exist() {
    let mut transport = Deaf;
    let error = cli::run(
        &args(&[
            "check",
            "start",
            "ws_abc123",
            "--target",
            "DieHard",
            "--target-kind",
            "not-a-kind",
            "--portfolio",
            "interactive",
            "--states",
            "16",
        ]),
        &mut transport,
        false,
    )
    .expect_err("an unknown closed-enum member never reaches the wire");
    assert_eq!(error.exit_code(), 1);
    let continuum_cli::error::CliError::Usage(detail) = &error else {
        panic!("an unknown member is a usage error: {error:?}");
    };
    assert!(
        detail.contains(TargetKind::AllClaims.as_wire()),
        "the error names what is accepted: {detail}"
    );
}

#[test]
fn the_start_budget_is_the_one_this_crate_already_had_a_name_for() {
    // `check start`'s ceiling is `task::states_budget`, the same single-dimension budget
    // `task resume` spends — one reading of "the one budget dimension this daemon enforces",
    // not two.
    let budget = task::states_budget(CLOSING_BUDGET);
    assert_eq!(budget.states, Optional::Present(CLOSING_BUDGET));
    assert!(budget.wall_ms.is_absent());

    // `check await`'s is the wall-clock one, because a wait is bounded in time.
    let wait = check::wall_budget(25);
    assert!(wait.states.is_absent());
    assert_eq!(
        wait.wall_ms.value().map(|ms| ms.millis()),
        Some(25),
        "the caller's one --timeout-ms sets the body's bound and the envelope's ceiling"
    );
}
