//! Evidence for the minimal typed client (PR 10, bn-134i).
//!
//! # What is asserted here, and what is asserted elsewhere
//!
//! This file is about the **client**: that its operations are typed end to end over a real
//! byte boundary, that its refusals are typed, that its grammar is a sound and pure function
//! of the caller's state, that it holds no semantic state, and that its bytes are
//! deterministic. The *comparison* against a shell baseline — the G0-DX-10 measurement — is
//! `continuum-benchmark`'s, because the harness is where both arms live.
//!
//! The fixture is duplicated locally rather than imported. A `tests/*.rs` file is its own
//! crate, and the crate that would supply a rig is the benchmark harness, which links this
//! one: importing it back would put a dev-dependency cycle in the workspace to save a
//! hundred lines.
//!
//! # Clause → test
//!
//! - **"exposing only typed operations"** →
//!   [`a_named_operation_answers_a_typed_payload_over_a_real_byte_boundary`]: the whole Die
//!   Hard lane through four named methods, with every fact read off a decoded frame.
//! - **"every refusal is typed"** → [`a_daemon_refusal_is_a_typed_code_and_not_a_message`].
//! - **"omission manifests surface (INV-007)"** →
//!   [`the_omission_manifest_arrives_inline_on_the_answer_that_declared_the_budget`].
//! - **research/25's semantic action grammar** →
//!   [`the_register_refuses_an_operation_whose_precondition_does_not_hold_without_a_frame`]
//!   and [`the_register_admits_every_operation_whose_handles_the_caller_holds`].
//! - **the grammar is *sound*: it never refuses something the daemon would admit** →
//!   [`the_register_never_refuses_an_operation_the_daemon_then_admits`], which is the
//!   research/25 kill criterion "task grammar constrains legitimate strategies" stated as a
//!   test.
//! - **"adapters do not own semantic state" (plan §20)** →
//!   [`the_client_holds_no_handle_and_the_grammar_is_a_function_of_the_callers_state`].
//! - **INV-005 — deterministic bytes** →
//!   [`two_fresh_daemons_answer_the_identical_script_with_identical_bytes`].
//! - **INV-015 — the client may narrow authority, never widen it** →
//!   [`speaking_as_a_reader_cannot_buy_an_execute_operation`].
//! - **the declared `Error.data` shape survives the client surface (RFC 0026 F19,
//!   bn-ulhg3)** →
//!   [`a_kernel_certificate_rejection_arrives_typed_on_the_clients_refusal`].
//! - **`data` this version declares no shape for is carried verbatim, never guessed at
//!   (RFC 0026, 3.4 revision)** →
//!   [`data_under_a_code_this_client_declares_no_shape_for_is_preserved_verbatim`].

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_mcp::client::{Call, states_budget};
use continuum_mcp::register::{self, CampaignState, Requirement, SnapshotState};
use continuum_mcp::{
    AgentClient, AgentContext, LinkError, LocalLink, Outcome, RefusalData, Transport,
};
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::publication::ContentIdentifier;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::codec::{from_bytes, to_bytes};
use continuumd::daemon::evidence::{self, EvidenceFamily, lattice_status};
use continuumd::daemon::family::{Arguments, ErrorData, Payload};
use continuumd::daemon::identity::{Blake3Identity, intent_to_wire};
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{EvidenceNode, IntentRecord, RegistryStatus, StatusWrite};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationRequest};
use continuumd::protocol::envelope::{
    Budget, Cost, EpochSet, Error, RequestEnvelope, ResultEnvelope, Verdict,
};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, ServerLimits, ServerWelcome,
    VersionRange, negotiate,
};
use continuumd::protocol::operations::evidence::EvidenceVerifyRequest;
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, ByteCount, CapabilityHandle, Commitment, DurationMs, EpochIdentity, EvidenceHandle,
    IntentHandle, Opaque, OperationName, ProtocolVersion, RequestId, TaskHandle, Timestamp,
    WorkspaceHandle,
};
use continuumd::protocol::shared::{FileComponent, SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, EvidenceKind, EvidenceNodeKind, EvidenceStatus,
    ResultStatus, SemanticVerdict, TargetKind, TaskStatus,
};
use continuumd::transport::{LocalPair, Server};

const MODULE: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");
const CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");
const CONTRACT: &str = include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");
const MODULE_PATH: &str = "DieHard.ctm";
const NOW: &str = "2026-08-01T00:00:00.000Z";
/// The frozen TV-009 reachable-state count (`notes/plan/spikes/SPIKE_REPORT.md:9`).
const FROZEN_STATES: u64 = 16;

// --- fixture -----------------------------------------------------------------------------

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
        client: "continuum-mcp-evidence".to_owned(),
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

fn welcome_for(negotiated: Negotiated) -> ServerWelcome {
    ServerWelcome {
        protocol_version: negotiated.protocol_version(),
        encoding: negotiated.encoding(),
        majors_served: vec![3, 2],
        server: "continuumd/0".to_owned(),
        grant: root_grant(),
        limits: ServerLimits {
            idempotency_retention_ms: DurationMs::new(86_400_000),
            max_page_size: 100,
            max_result_bytes: ByteCount::new(1_048_576),
            max_concurrent_tasks: 4,
        },
        features: Vec::new(),
        epochs: epochs(),
        pending_advances: Vec::new(),
    }
}

/// One provisioned deployment behind a byte boundary.
struct Fixture {
    server: Server,
    pair: LocalPair,
    welcome: Vec<u8>,
    components: SnapshotComponents,
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
                root.clone(),
            )
            .capability(
                grant("cap_reader", "agent:reader", AuthorityLevel::Read),
                root,
            )
            .family(WorkspaceFamily)
            .family(IntentFamily)
            .family(TaskFamily)
            .family(VerificationFamily)
            .family(EvidenceFamily::new())
            .build();

        let intent = accept_intent(&mut daemon);
        let components = stage(&mut daemon, intent);

        let welcome = welcome_for(negotiated);
        let welcome_frame = Server::open(&welcome, None, Ok(negotiated))
            .expect("the welcome encodes")
            .expect("a welcome frame is sent");
        let decoded: ServerWelcome = from_bytes(&welcome_frame).expect("the welcome decodes");
        assert_eq!(decoded, welcome);

        Self {
            server: Server::new(daemon, negotiated),
            pair: LocalPair::new(),
            welcome: welcome_frame,
            components,
        }
    }

    fn link(&mut self) -> LocalLink<'_> {
        LocalLink::new(&mut self.server, &mut self.pair, &self.welcome)
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
        envelope: RequestEnvelope {
            protocol_version: version(),
            request_id: RequestId::new("req_accept").expect("a request id"),
            idempotency_key: Optional::Present("idem-accept".to_owned()),
            actor: actor("service:continuumd"),
            capability: capability("cap_root"),
            operation: OperationName::new("intent.accept").expect("a name"),
            snapshot: Nullable::Null,
            intent: Nullable::Null,
            arguments: Opaque::from_bytes(Vec::new()),
            budget: Optional::Absent,
            output_policy: Optional::Absent,
            trace: Optional::Absent,
            page: Optional::Absent,
        },
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

fn client(who: &str, handle: &str) -> AgentClient {
    AgentClient::new(version(), actor(who), capability(handle))
}

/// A budget declaring one ceiling this daemon meters and two it does not.
fn declared() -> Budget {
    Budget {
        wall_ms: Optional::Present(DurationMs::new(30_000)),
        bytes: Optional::Present(ByteCount::new(1_048_576)),
        ..states_budget(64)
    }
}

fn target() -> Target {
    Target {
        kind: TargetKind::AllClaims,
        id: "DieHard".to_owned(),
    }
}

/// Create a sealed Die Hard snapshot and return its handle and the context after it.
fn sealed(fixture: &mut Fixture, client: &mut AgentClient) -> (WorkspaceHandle, AgentContext) {
    let components = fixture.components.clone();
    let mut context = AgentContext::EMPTY;
    {
        let hello = hello();
        let mut link = fixture.link();
        let welcome = client
            .open(&mut link, &hello)
            .expect("the connection opens");
        assert_eq!(welcome.protocol_version, version());
    }
    let answer = {
        let mut link = fixture.link();
        client
            .workspace_create(&mut link, &context, components, true)
            .expect("the call is made")
    };
    let admitted = answer.success().expect("the create is admitted");
    context.observe("workspace.create", admitted);
    let Payload::WorkspaceCreate(response) = &admitted.payload else {
        panic!("a create answers a create payload");
    };
    (response.snapshot.clone(), context)
}

// --- the evidence -------------------------------------------------------------------------

#[test]
fn a_named_operation_answers_a_typed_payload_over_a_real_byte_boundary() {
    let mut fixture = Fixture::fresh();
    let mut agent = client("agent:builder", "cap_builder");
    let (snapshot, mut context) = sealed(&mut fixture, &mut agent);
    assert_eq!(context.snapshot, SnapshotState::Sealed);
    assert!(snapshot.as_str().starts_with("ws_"));

    agent.speak_as(actor("agent:runner"), capability("cap_runner"));
    let started = {
        let mut link = fixture.link();
        agent
            .verification_start(&mut link, &context, &snapshot, target(), declared())
            .expect("the call is made")
    };
    let admitted = started.success().expect("the campaign starts");
    assert_eq!(admitted.status, ResultStatus::TaskStarted);
    let task = admitted
        .task
        .clone()
        .expect("a task-starting result names its task");
    context.observe("verification.start", admitted);
    assert_eq!(context.campaign, CampaignState::Live);

    agent.speak_as(actor("agent:reader"), capability("cap_reader"));
    let status = {
        let mut link = fixture.link();
        agent
            .task_status(&mut link, &context, &task)
            .expect("the call is made")
    };
    let admitted = status.success().expect("the record is answered");
    let Payload::TaskStatus(record) = &admitted.payload else {
        panic!("a status answers a task record");
    };
    assert_eq!(record.status, TaskStatus::Completed);
    assert_eq!(
        record.cost.states,
        Optional::Present(FROZEN_STATES),
        "the frozen sixteen, read off a decoded frame rather than off the engine"
    );
    context.observe("task.status", admitted);
    assert_eq!(context.campaign, CampaignState::Settled);

    agent.speak_as(actor("agent:runner"), capability("cap_runner"));
    let result = {
        let mut link = fixture.link();
        agent
            .verification_result(&mut link, &context, &task)
            .expect("the call is made")
    };
    let admitted = result.success().expect("the verdict is answered");
    let Some(Verdict::Semantic(verdict)) = &admitted.verdict else {
        panic!("a verification result carries a semantic verdict");
    };
    assert_eq!(
        verdict.verdict,
        SemanticVerdict::Refuted,
        "`NotSolved` is refuted by the film's six-step solution"
    );
    let Payload::VerificationResult(body) = &admitted.payload else {
        panic!("a result answers a VerificationResult payload");
    };
    assert_eq!(body.task, task);

    let ledger = agent.ledger();
    assert_eq!(ledger.calls, 4, "four operations reached the wire");
    assert_eq!(ledger.unmet, 0, "and the register refused none of them");
    assert!(ledger.operations.sent > 0 && ledger.operations.received > 0);
    assert!(
        ledger.total() > ledger.operations.total(),
        "the handshake counts too"
    );
}

#[test]
fn a_daemon_refusal_is_a_typed_code_and_not_a_message() {
    let mut fixture = Fixture::fresh();
    let mut agent = client("agent:builder", "cap_builder");
    let (snapshot, context) = sealed(&mut fixture, &mut agent);

    // `verification.start` requires `execute`; this principal has `read`.
    agent.speak_as(actor("agent:reader"), capability("cap_reader"));
    let answer = {
        let mut link = fixture.link();
        agent
            .verification_start(&mut link, &context, &snapshot, target(), declared())
            .expect("the call is made")
    };
    let refusal = answer.refusal().expect("a read capability cannot execute");
    assert_eq!(refusal.code, ErrorCode::CapabilityDenied);
    assert!(!refusal.retryable, "an identical retry cannot succeed");
    assert_eq!(
        answer.error_code(),
        Some(ErrorCode::CapabilityDenied),
        "the code is the branch, and it is an enum"
    );
    assert!(answer.attempted() && !answer.admitted());
    assert!(
        answer.bytes.total() > 0,
        "a refusal is answered over the wire and is paid for"
    );

    // RFC 0026 declares `Error.recovery` as the protocol's only recovery channel. This daemon
    // populates none, so the client carries an empty list rather than inventing a next step.
    assert!(
        refusal.recovery.is_empty(),
        "no prefilled recovery operations exist in this daemon yet"
    );
    assert!(refusal.continuation.is_none());
}

#[test]
fn the_omission_manifest_arrives_inline_on_the_answer_that_declared_the_budget() {
    let mut fixture = Fixture::fresh();
    let mut agent = client("agent:builder", "cap_builder");
    let (snapshot, context) = sealed(&mut fixture, &mut agent);
    agent.speak_as(actor("agent:runner"), capability("cap_runner"));

    let answer = {
        let mut link = fixture.link();
        agent
            .verification_start(&mut link, &context, &snapshot, target(), declared())
            .expect("the call is made")
    };
    let admitted = answer.success().expect("the campaign starts");
    let subjects: Vec<&str> = admitted
        .omissions
        .iter()
        .map(|omission| omission.subject.as_str())
        .collect();
    assert!(
        subjects.contains(&"budget.wall_ms") && subjects.contains(&"budget.bytes"),
        "the two ceilings this daemon cannot meter are named: {subjects:?}"
    );
    assert!(
        !subjects.contains(&"budget.states"),
        "and the one it can meter is not, which is how a caller learns the ceiling applied"
    );
    // INV-007: the manifest is on the answer the caller already has. No second call, no
    // expansion handle, no extra byte.
    assert_eq!(
        agent.ledger().calls,
        2,
        "the create and the start; nothing else"
    );
}

#[test]
fn the_register_refuses_an_operation_whose_precondition_does_not_hold_without_a_frame() {
    let mut fixture = Fixture::fresh();
    let mut agent = client("agent:builder", "cap_builder");

    // Create *unsealed*, then try to verify it. The grammar says `verification.start`
    // requires a sealed snapshot.
    let components = fixture.components.clone();
    let mut context = AgentContext::EMPTY;
    let created = {
        let mut link = fixture.link();
        agent
            .workspace_create(&mut link, &context, components, false)
            .expect("the call is made")
    };
    let admitted = created.success().expect("the create is admitted");
    context.observe("workspace.create", admitted);
    assert_eq!(context.snapshot, SnapshotState::Draft);
    let Payload::WorkspaceCreate(response) = &admitted.payload else {
        panic!("a create answers a create payload");
    };
    let snapshot = response.snapshot.clone();

    let before = agent.ledger();
    agent.speak_as(actor("agent:runner"), capability("cap_runner"));
    let answer = {
        let mut link = fixture.link();
        agent
            .verification_start(&mut link, &context, &snapshot, target(), declared())
            .expect("the call is made")
    };
    let unmet = answer.unmet().expect("the register refuses it");
    assert_eq!(unmet.operation, "verification.start");
    assert_eq!(unmet.requires, Requirement::SealedSnapshot);
    assert_eq!(unmet.context.snapshot, SnapshotState::Draft);

    assert!(answer.attempted(), "it is still an attempt");
    assert!(!answer.admitted());
    assert_eq!(answer.error_code(), None, "no wire code is minted for it");
    assert_eq!(answer.bytes.total(), 0, "and no frame was written");
    let after = agent.ledger();
    assert_eq!(after.calls, before.calls, "the wire saw nothing");
    assert_eq!(after.unmet, before.unmet + 1);
    assert_eq!(after.operations, before.operations);
}

#[test]
fn the_register_admits_every_operation_whose_handles_the_caller_holds() {
    assert_eq!(
        register::allowed(&AgentContext::EMPTY),
        vec!["workspace.create"],
        "with nothing in hand, only a create is legal"
    );
    let drafted = AgentContext {
        snapshot: SnapshotState::Draft,
        campaign: CampaignState::Absent,
    };
    assert_eq!(
        register::allowed(&drafted),
        vec!["workspace.create", "workspace.fork", "workspace.seal"],
        "a draft snapshot buys the two operations that take one"
    );
    let running = AgentContext {
        snapshot: SnapshotState::Sealed,
        campaign: CampaignState::Suspended,
    };
    assert_eq!(
        register::allowed(&running),
        vec![
            "workspace.create",
            "workspace.fork",
            "workspace.seal",
            "verification.start",
            "verification.result",
            "task.status",
            "task.cancel",
            "task.resume",
        ],
        "and a parked continuation is the only thing that buys a resume"
    );
    assert_eq!(
        register::OPERATIONS.len(),
        8,
        "the grammar names exactly the acts the client exposes — protocol 3.6's \
         `workspace.create_by_reference` is the second spelling of the first row's \
         argument, not a ninth act"
    );
    // `workspace.create` is unconditioned on purpose: creating a second snapshot while
    // holding a first is a legitimate strategy, and research/25's kill list includes a
    // grammar that constrains those.
    assert_eq!(
        register::rule("workspace.create").map(|rule| rule.requires),
        Some(Requirement::Nothing)
    );
}

/// Soundness: the grammar must never refuse an operation the daemon would have admitted.
#[test]
fn the_register_never_refuses_an_operation_the_daemon_then_admits() {
    // The one refusal this grammar makes on the Die Hard lane is `verification.start` over an
    // unsealed snapshot. Send it anyway, with the register bypassed by presenting a context
    // that satisfies it, and confirm the daemon refuses it too — so the local refusal was a
    // prediction rather than a restriction.
    let mut fixture = Fixture::fresh();
    let mut agent = client("agent:builder", "cap_builder");
    let components = fixture.components.clone();
    let mut context = AgentContext::EMPTY;
    let created = {
        let mut link = fixture.link();
        agent
            .workspace_create(&mut link, &context, components, false)
            .expect("the call is made")
    };
    let admitted = created.success().expect("the create is admitted");
    context.observe("workspace.create", admitted);
    let Payload::WorkspaceCreate(response) = &admitted.payload else {
        panic!("a create answers a create payload");
    };
    let snapshot = response.snapshot.clone();

    let bypass = AgentContext {
        snapshot: SnapshotState::Sealed,
        campaign: CampaignState::Absent,
    };
    agent.speak_as(actor("agent:runner"), capability("cap_runner"));
    let answer = {
        let mut link = fixture.link();
        agent
            .verification_start(&mut link, &bypass, &snapshot, target(), declared())
            .expect("the call is made")
    };
    assert!(
        answer.unmet().is_none(),
        "the register was told the snapshot is sealed, so it admitted the call"
    );
    assert_eq!(
        answer.error_code(),
        Some(ErrorCode::StaleSnapshot),
        "and the daemon refused it, which is what the register predicted"
    );
}

#[test]
fn the_client_holds_no_handle_and_the_grammar_is_a_function_of_the_callers_state() {
    let mut fixture = Fixture::fresh();
    let mut agent = client("agent:builder", "cap_builder");
    let (_, context) = sealed(&mut fixture, &mut agent);

    // The client just created and sealed a snapshot. A *second* caller's context, which
    // knows nothing about it, is refused the operations that snapshot would have unlocked —
    // because the state is the caller's and the client is holding none of it.
    let stranger = AgentContext::EMPTY;
    assert_eq!(register::allowed(&stranger), vec!["workspace.create"]);
    assert!(register::admits(&stranger, "verification.start").is_err());
    assert!(register::admits(&context, "verification.start").is_ok());

    // Two subagents, one workspace, no session coupling (plan §10.5): the same client answers
    // both contexts without either learning about the other.
    assert_ne!(stranger, context);
}

#[test]
fn two_fresh_daemons_answer_the_identical_script_with_identical_bytes() {
    let script = |()| -> (Vec<u64>, u64) {
        let mut fixture = Fixture::fresh();
        let mut agent = client("agent:builder", "cap_builder");
        let (snapshot, context) = sealed(&mut fixture, &mut agent);
        agent.speak_as(actor("agent:runner"), capability("cap_runner"));
        let mut sizes = Vec::new();
        {
            let mut link = fixture.link();
            let answer = agent
                .verification_start(&mut link, &context, &snapshot, target(), declared())
                .expect("the call is made");
            sizes.push(answer.bytes.sent);
            sizes.push(answer.bytes.received);
        }
        (sizes, agent.ledger().total())
    };
    assert_eq!(
        script(()),
        script(()),
        "two independently built daemons, one script, one byte count"
    );
}

#[test]
fn speaking_as_a_reader_cannot_buy_an_execute_operation() {
    let mut fixture = Fixture::fresh();
    let mut agent = client("agent:builder", "cap_builder");
    let (snapshot, context) = sealed(&mut fixture, &mut agent);

    // The client changes which principal it speaks as. It cannot change what that principal
    // may do: the capability table is the daemon's (INV-015).
    for (who, handle, expected) in [
        (
            "agent:reader",
            "cap_reader",
            Some(ErrorCode::CapabilityDenied),
        ),
        (
            "agent:builder",
            "cap_builder",
            Some(ErrorCode::CapabilityDenied),
        ),
        ("agent:runner", "cap_runner", None),
    ] {
        let mut fresh = client(who, handle);
        let answer = {
            let mut link = fixture.link();
            fresh
                .verification_start(&mut link, &context, &snapshot, target(), declared())
                .expect("the call is made")
        };
        assert_eq!(answer.error_code(), expected, "{who} under {handle}");
    }
    let _ = agent.ledger();
}

// --- the `Error.data` carry (RFC 0026 F19, bn-ulhg3) -------------------------------------

/// The instrumentation profile a certificate node is filed under.
const CERTIFICATE_PROFILE: &str = "continuum-engine-reference/finite-closure";

/// A real `CONTCERT` artifact: Die Hard explored, closed, and emitted as wire bytes — the
/// same construction `continuumd`'s own F19 evidence emits (`tests/daemon_evidence.rs`),
/// built from the engine crate this file already provisions the daemon's models from.
fn certificate_bytes() -> Vec<u8> {
    use continuum_engine_reference::bfs::{self, Bounds};
    use continuum_engine_reference::certificate::{self, ClaimEnvelope, ClosedSet, PRODUCER};

    let model = diehard::model().expect("the Die Hard transcription is a valid model");
    let exploration = bfs::explore(&model, Bounds::CERTIFIABLE).expect("Die Hard evaluates");
    let closed = ClosedSet::of(&exploration).expect("Die Hard's exploration completes");
    certificate::emit_finite_closure(
        &model,
        closed,
        &ClaimEnvelope {
            model_digest: "blake3:diehard-model",
            semantic_epoch: "continuum-semantics-1",
            property_digest: "blake3:diehard-typeok",
            scope_digest: "blake3:diehard-scope",
            assumptions_digest: "blake3:empty-assumptions",
            producer: PRODUCER,
            domain_pack_digests: &[],
        },
    )
    .expect("a closed exploration of a declared model emits")
}

/// Stage `bytes` and file a certificate-class node over them, under the identity the seam
/// derives — out of band, through [`Server::daemon_mut`], because provisioning is
/// administration rather than an operation (IDL §7).
fn certificate_node(fixture: &mut Fixture, bytes: Vec<u8>) -> EvidenceHandle {
    let artifact = fixture
        .server
        .daemon_mut()
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("certs/mutated.cert").expect("a workspace path"),
            bytes,
        )
        .expect("staging names its content");
    let handle = evidence::node_identity(
        fixture.server.daemon().services(),
        &artifact,
        CERTIFICATE_PROFILE,
    )
    .expect("the identity seam names the node");
    let record = EvidenceNode {
        kind: EvidenceNodeKind::Certificate,
        evidence_kind: Some(EvidenceKind::Certificate),
        labels: Vec::new(),
        claim_id: "claim:die-hard-closure".to_owned(),
        artifact,
        // Not the verification service: INV-004 refuses a producer that verifies its own
        // claim before any byte is read, and that refusal is not this test's subject.
        producer: actor("agent:builder"),
        tool: CERTIFICATE_PROFILE.to_owned(),
        created_at: Timestamp::new(NOW).expect("a timestamp"),
        inputs: Vec::new(),
        idempotency_key: "idem-certificate-append".to_owned(),
        history: vec![StatusWrite {
            status: lattice_status(EvidenceStatus::Proposed),
            service_identity: None,
            validation_basis: None,
            inconclusive_reason: None,
        }],
        redaction: None,
        // Appended out of band: no store publication, so no receipt-tied name (bn-283p6).
        publication: None,
    };
    fixture
        .server
        .daemon_mut()
        .state_mut()
        .append_evidence(handle.clone(), record);
    handle
}

/// F19 through the client surface: a certificate the kernel rejects, driven through the
/// daemon over this client's own wire, arrives as a client-visible typed value with the
/// kernel's `checker`/`reason`/`field` intact (bn-ulhg3).
///
/// Flagged at bn-3jrtz's 3.4 bundle: the CLI rendered `error.data` from the day the shape
/// was declared, while this client's [`Refusal`](continuum_mcp::Refusal) dropped it — so a
/// typed-client caller could not see the one thing `Error.data` was declared to carry.
/// This test is the anti-vacuity for the carry: before bn-ulhg3 the `data` read below was
/// impossible, and if the block is ever dropped again the field reads `None` and the test
/// fails.
#[test]
fn a_kernel_certificate_rejection_arrives_typed_on_the_clients_refusal() {
    let mut fixture = Fixture::fresh();
    // One trailing byte. The magic is untouched, so routing succeeds and a kernel really
    // answers `Rejected` — a rejection, not a routing failure (the same mutation
    // `continuumd`'s own F19 evidence drives).
    let mut mutated = certificate_bytes();
    mutated.push(0x00);
    let handle = certificate_node(&mut fixture, mutated);

    // The daemon's own typed outcome over the same node, captured in-process before any
    // frame exists. The wire-carried value below is held against THIS — the relay whose
    // tokens `continuumd`'s F19 evidence pins byte-equal to the kernel's own answer —
    // rather than against this test's guess at a vocabulary the kernels own.
    let expected = {
        let outcome = fixture.server.daemon_mut().dispatch(&OperationRequest {
            envelope: RequestEnvelope {
                protocol_version: version(),
                request_id: RequestId::new("req_seed").expect("a request id"),
                idempotency_key: Optional::Present("idem-seed".to_owned()),
                actor: actor("agent:reader"),
                capability: capability("cap_reader"),
                operation: OperationName::new("evidence.verify").expect("a name"),
                snapshot: Nullable::Null,
                intent: Nullable::Null,
                arguments: Opaque::from_bytes(Vec::new()),
                budget: Optional::Present(declared()),
                output_policy: Optional::Absent,
                trace: Optional::Absent,
                page: Optional::Absent,
            },
            arguments: Arguments::EvidenceVerify(EvidenceVerifyRequest {
                evidence: handle.clone(),
                expected_status: Optional::Absent,
            }),
        });
        assert_eq!(outcome.envelope.status, ResultStatus::Error);
        match outcome.data {
            ErrorData::CertificateRejection(body) => *body,
            ErrorData::None => panic!("a CertificateRejected outcome carries its declared data"),
        }
    };

    // The same rejection, over the client's own wire. A rejection writes no status, so the
    // node is still at the lattice's bottom and the kernel answers the second caller too.
    let mut agent = client("agent:reader", "cap_reader");
    {
        let hello = hello();
        let mut link = fixture.link();
        agent.open(&mut link, &hello).expect("the connection opens");
    }
    let arguments = Arguments::EvidenceVerify(EvidenceVerifyRequest {
        evidence: handle,
        expected_status: Optional::Absent,
    });
    let answer = {
        let mut link = fixture.link();
        agent
            .invoke(
                &mut link,
                &AgentContext::EMPTY,
                &Call::new(&arguments).within(declared()),
            )
            .expect("the call is made")
    };
    assert!(
        answer.bytes.total() > 0,
        "the rejection was answered over a real byte boundary"
    );
    let refusal = answer.refusal().expect("the kernel rejected the bytes");
    assert_eq!(refusal.code, ErrorCode::CertificateRejected);

    let data = refusal
        .data
        .as_ref()
        .expect("the declared `Error.data` block survives the client surface (RFC 0026 F19)");
    let RefusalData::CertificateRejection(body) = data else {
        panic!("`CertificateRejected` resolves to its declared shape, got {data:?}");
    };
    assert_eq!(
        body, &expected,
        "checker/reason/field intact across the client's wire"
    );
    // The tokens are the first authorities' own, relayed verbatim (bn-dtg61): the checker
    // names the trusted checking-base crate, and a trailing-byte rejection names no wire
    // position — so none is invented for it.
    assert_eq!(body.checker, "continuum-kernel-core");
    assert!(
        !body.reason.is_empty(),
        "the kernel's own stable reason token travels"
    );
    assert!(body.field.is_absent());
}

/// A byte boundary standing in for a daemon *newer than this client*: it answers every
/// exchange with one prepared frame. This daemon cannot be made to play the part — `rule
/// encoding.opaque_payloads` forbids it from emitting `data` under a code that declares no
/// shape — so the future one is a hand-built frame, the conformance-vector posture.
struct FutureDaemon {
    answer: Vec<u8>,
}

impl Transport for FutureDaemon {
    fn open(&mut self, _hello: &[u8]) -> Result<Vec<u8>, LinkError> {
        Err(LinkError::NoAnswer)
    }

    fn exchange(&mut self, _frame: &[u8]) -> Result<Vec<u8>, LinkError> {
        Ok(self.answer.clone())
    }
}

/// The forward-compatibility half of the carry: `data` under a code this client's version
/// declares no shape for MUST NOT break the client, and MUST NOT be guessed at — RFC
/// 0026's 3.4 revision obliges exactly one reading, "carry the canonical value verbatim".
///
/// The frame stages F11's standing candidate: a future minor declaring a retry-after shape
/// for `QuotaExhausted`. The codec refuses to invent a reading for it
/// (`CodecError::UndeclaredErrorData`), and this client's posture mirrors the refusal's
/// grounds without inheriting its severity: nothing is interpreted, and nothing is lost —
/// the bytes arrive as [`RefusalData::Undeclared`], preserved verbatim.
#[test]
fn data_under_a_code_this_client_declares_no_shape_for_is_preserved_verbatim() {
    // F11's proposed retry-after, as a future daemon might declare it: a canonical value
    // this client's protocol version has no shape for.
    let future_shape = Opaque::from_bytes(br#"{"retry_after_ms":30000}"#.to_vec());
    let envelope = ResultEnvelope {
        request_id: RequestId::new("req_c000000").expect("a request id"),
        status: ResultStatus::Error,
        verdict: Nullable::Null,
        error: Optional::Present(Error {
            code: ErrorCode::QuotaExhausted,
            detail: "concurrent task quota exhausted".to_owned(),
            data: Optional::Present(future_shape.clone()),
            recovery: Vec::new(),
            continuation: Optional::Absent,
            non_resumable_reason: Optional::Absent,
            retryable: true,
        }),
        assurance: Optional::Absent,
        artifacts: Vec::new(),
        task: Optional::Absent,
        continuation: Optional::Absent,
        omissions: Vec::new(),
        warnings: Vec::new(),
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
        epochs: epochs(),
        next_operations: Vec::new(),
        next_page_token: Optional::Absent,
        payload: Nullable::Null,
        audit: Optional::Absent,
    };
    let mut future = FutureDaemon {
        answer: to_bytes(&envelope).expect("the future frame encodes"),
    };

    let mut agent = client("agent:runner", "cap_runner");
    let held = AgentContext {
        snapshot: SnapshotState::Absent,
        campaign: CampaignState::Live,
    };
    let task = TaskHandle::new("task_future").expect("a task handle");
    let answer = agent
        .task_status(&mut future, &held, &task)
        .expect("an unrecognized code's `data` must not break the client");

    let refusal = answer.refusal().expect("the frame is a typed refusal");
    assert_eq!(refusal.code, ErrorCode::QuotaExhausted);
    assert!(refusal.retryable);
    assert_eq!(
        refusal.data,
        Some(RefusalData::Undeclared(future_shape)),
        "the canonical value is carried verbatim, never guessed at (RFC 0026, 3.4)"
    );
}

#[test]
fn a_client_error_is_never_a_refusal() {
    // The three arms of `Outcome` are distinct by construction, and `ClientError` is not one
    // of them: a transport or codec failure is a `Result::Err`, never an answer.
    let mut fixture = Fixture::fresh();
    let mut agent = client("agent:builder", "cap_builder");
    let components = fixture.components.clone();
    let answer = {
        let mut link = fixture.link();
        agent
            .workspace_create(&mut link, &AgentContext::EMPTY, components, true)
            .expect("a well-formed call does not fail below the protocol")
    };
    assert!(matches!(answer.outcome, Outcome::Admitted(_)));
    assert_eq!(answer.operation, "workspace.create");
}
