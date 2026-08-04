//! Evidence for `continuum task status|resume|cancel` (bn-3tz60, PR-13 third command
//! group) — the PR-6 lifecycle, driven through `continuum_cli::task`'s command functions
//! over a real `LocalPair` boundary against a real, provisioned `Daemon`.
//!
//! # Clause → test
//!
//! - **"status renders the TaskRecord including cost/budget/omissions faithfully"** →
//!   [`status_renders_a_suspended_records_cost_budget_and_continuation_faithfully`].
//! - **"resume refuses a continuation whose epoch or inputs no longer validate, with the
//!   typed reason surfaced"** →
//!   [`resume_refuses_a_continuation_whose_pinned_snapshot_the_lineage_superseded`], the
//!   `StaleSnapshot` recipe `continuumd/tests/dx03_falsification.rs`'s
//!   `attack_staleness_every_snapshot_consuming_operation_refuses_a_superseded_handle`
//!   exercises against `Daemon::dispatch` directly — reproduced here against the CLI's own
//!   command function instead.
//! - **a valid resume, for contrast** →
//!   [`resume_spends_a_valid_continuation_and_closes_the_frozen_campaign`].
//! - **"cancel at any phase leaves a valid continuation or a clean absence, with the handle
//!   printed and artifacts committed-or-absent stated"** →
//!   [`cancel_of_a_suspended_task_reports_a_valid_continuation_and_prints_its_handle`],
//!   [`cancel_of_a_completed_task_reports_a_clean_absence`].
//!
//! # The fixture
//!
//! `Fixture::fresh` is `continuum-mcp/tests/typed_surface.rs`'s `Fixture` and
//! `continuum-benchmark::rig::Rig::fresh`'s daemon-provisioning half, trimmed to the one
//! corpus port (Die Hard, TV-009) both already use, and duplicated locally rather than
//! imported — the reason is `typed_surface.rs`'s own comment: a `tests/*.rs` file is its
//! own crate, and the crate that would supply a shared rig is the benchmark harness, which
//! would put a dev-dependency cycle in the workspace to save a hundred lines. Setup calls
//! (`workspace.create`, `workspace.fork`, `verification.start`) are not part of this
//! bone's command surface, so they are dispatched directly against the fixture's `Daemon`,
//! exactly as the same two fixtures dispatch their own setup.

use continuum_cli::format::Format;
use continuum_cli::task::{self, CancelOutcome};
use continuum_cli::wire::{Connection, LocalLink, Outcome};
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
use continuumd::protocol::operations::verification::VerificationStartRequest;
use continuumd::protocol::operations::workspace::{WorkspaceCreateRequest, WorkspaceForkRequest};
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, EpochIdentity, IntentHandle, Opaque, OperationName,
    ProtocolVersion, RequestId, TaskHandle, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{
    FileComponent, FileOverlay, SnapshotComponents, SnapshotEpochs, Target,
};
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, Portfolio, ResultStatus, TargetKind, TaskStatus,
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
/// Below the frozen count: forces a park with a continuation.
const PARK_BUDGET: u64 = 4;
/// What a `PARK_BUDGET` campaign actually costs before parking: the ceiling check runs
/// before a state is explored, so a bound of `N` commits `N - 1` — "a Die Hard campaign
/// that parks at 3 states and closes at 16 cost 16" (`continuumd::daemon::task`'s own
/// module doc, bn-23j7s).
const PARKED_COST_STATES: u64 = 3;
/// Above the frozen count: closes the campaign outright.
const CLOSING_BUDGET: u64 = 64;

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

/// One provisioned deployment behind a byte boundary — a real `Server`/`LocalPair`, exactly
/// as `continuum-mcp/tests/typed_surface.rs::Fixture` is.
struct Fixture {
    server: Server,
    pair: LocalPair,
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
            .build();

        let intent = accept_intent(&mut daemon);
        let components = stage(&mut daemon, intent);

        Self {
            server: Server::new(daemon, negotiated),
            pair: LocalPair::new(),
            components,
        }
    }

    /// The byte boundary a CLI command's own calls travel over.
    fn link(&mut self) -> LocalLink<'_> {
        LocalLink::new(&mut self.server, &mut self.pair)
    }

    /// Dispatch one raw operation directly — for fixture setup only, never through the
    /// CLI's own command surface.
    fn setup(&mut self, envelope: RequestEnvelope, arguments: Arguments) -> Payload {
        let outcome = self.server.daemon_mut().dispatch(&OperationRequest {
            envelope,
            arguments,
        });
        assert_eq!(
            outcome.envelope.status,
            ResultStatus::Ok,
            "{:?}",
            outcome.envelope.error
        );
        outcome.payload
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

fn states_budget(states: u64) -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(states),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

fn target() -> Target {
    Target {
        kind: TargetKind::AllClaims,
        id: "DieHard".to_owned(),
    }
}

/// Create and seal a Die Hard snapshot.
fn sealed(fixture: &mut Fixture, request_id: &str) -> WorkspaceHandle {
    let components = fixture.components.clone();
    let payload = fixture.setup(
        setup_envelope(
            "agent:builder",
            "cap_builder",
            "workspace.create",
            request_id,
            Optional::Absent,
        ),
        Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components,
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    );
    let Payload::WorkspaceCreate(response) = payload else {
        panic!("a create answers a create payload");
    };
    response.snapshot
}

/// Start a campaign over `snapshot` under `states`, returning its task handle.
fn start(
    fixture: &mut Fixture,
    snapshot: &WorkspaceHandle,
    states: u64,
    request_id: &str,
) -> TaskHandle {
    let envelope = RequestEnvelope {
        snapshot: Nullable::Value(snapshot.clone()),
        ..setup_envelope(
            "agent:runner",
            "cap_runner",
            "verification.start",
            request_id,
            Optional::Present(states_budget(states)),
        )
    };
    let outcome = fixture.server.daemon_mut().dispatch(&OperationRequest {
        envelope,
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target: target(),
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    });
    outcome
        .envelope
        .task
        .value()
        .cloned()
        .expect("a task-starting result names its task")
}

/// Fork the base snapshot with unrelated content, advancing the lineage past it — the
/// `dx03_falsification.rs` recipe for making a still-valid continuation's pinned snapshot
/// stale.
fn fork_past(fixture: &mut Fixture, base: &WorkspaceHandle, request_id: &str) {
    fixture.setup(
        setup_envelope(
            "agent:builder",
            "cap_builder",
            "workspace.fork",
            request_id,
            Optional::Absent,
        ),
        Arguments::WorkspaceFork(WorkspaceForkRequest {
            base: base.clone(),
            overlay: Optional::Present(vec![FileOverlay {
                path: "elsewhere.txt".to_owned(),
                content: b"elsewhere".to_vec(),
            }]),
            patches: Optional::Absent,
        }),
    );
}

fn connection_as(who: &str, cap: &str) -> Connection {
    Connection::new(version(), actor(who), capability(cap))
}

// --- the evidence -------------------------------------------------------------------------

#[test]
fn status_renders_a_suspended_records_cost_budget_and_continuation_faithfully() {
    let mut fixture = Fixture::fresh();
    let snapshot = sealed(&mut fixture, "req_create");
    let task = start(&mut fixture, &snapshot, PARK_BUDGET, "req_start");

    // The typed answer, read directly: the source of truth every rendering below is held
    // to.
    let mut connection = connection_as("agent:runner", "cap_runner");
    let outcome = {
        let mut link = fixture.link();
        connection
            .task_status(&mut link, &task)
            .expect("the call is answered")
    };
    let Outcome::Admitted(admitted) = &outcome else {
        panic!("a status of a task this daemon holds is admitted: {outcome:?}");
    };
    let Payload::TaskStatus(record) = &admitted.payload else {
        panic!("task.status answers a task record");
    };
    assert_eq!(record.status, TaskStatus::Suspended);
    assert_eq!(record.cost.states, Optional::Present(PARKED_COST_STATES));
    assert_eq!(record.budget.states, Optional::Present(PARK_BUDGET));
    assert!(
        record.continuation.value().is_some(),
        "a suspended task carries a continuation"
    );

    // The rendered projection of that same answer, in every format — `task.status` is
    // `@readonly`, so calling it again per format is a re-read, not a second effect
    // (`rule subscription.hints_only`'s neighbor for a plain read).
    for format in [Format::Text, Format::Pretty, Format::Json] {
        let mut connection = connection_as("agent:runner", "cap_runner");
        let mut link = fixture.link();
        let rendered =
            task::status(&mut connection, &mut link, &task, format).expect("the call is answered");
        assert_eq!(rendered.exit_code, 0);
        assert!(
            rendered.text.contains(TaskStatus::Suspended.as_wire()),
            "{format:?}:\n{}",
            rendered.text
        );
        assert!(
            rendered.text.contains(&PARK_BUDGET.to_string()),
            "record.cost.states and record.budget.states render in {format:?}:\n{}",
            rendered.text
        );
    }

    // The record's `epochs` reached no format at all before bn-ybh1z — a field the daemon
    // sent and the adapter dropped, and the one a caller needs to read a
    // `ContinuationEpochMismatch` refusal. All seven are rendered now, by name.
    let mut connection = connection_as("agent:runner", "cap_runner");
    let mut link = fixture.link();
    let text = task::status(&mut connection, &mut link, &task, Format::Text)
        .expect("the call is answered");
    for key in [
        "record.epochs  7",
        "record.epochs.protocol  ",
        "record.epochs.semantic  ",
        "record.epochs.intent  ",
        "record.epochs.evidence  ",
        "record.epochs.proof  ",
        "record.epochs.corpus  ",
        "record.epochs.engine  ",
    ] {
        assert!(text.text.contains(key), "{key} renders:\n{}", text.text);
    }
    // The record's own `operation` — the operation the *task* runs — is one level down, so
    // it cannot be read as the operation this command drove (`continuum_cli::contract`,
    // "Reserved keys").
    assert!(
        text.text.contains("operation  task.status"),
        "{}",
        text.text
    );
    assert!(
        text.text.contains("record.operation  verification.start"),
        "{}",
        text.text
    );
}

#[test]
fn resume_spends_a_valid_continuation_and_closes_the_frozen_campaign() {
    let mut fixture = Fixture::fresh();
    let snapshot = sealed(&mut fixture, "req_create");
    let task = start(&mut fixture, &snapshot, PARK_BUDGET, "req_start");

    let status_before = {
        let mut connection = connection_as("agent:runner", "cap_runner");
        let mut link = fixture.link();
        connection
            .task_status(&mut link, &task)
            .expect("the call is answered")
    };
    let Outcome::Admitted(admitted) = &status_before else {
        panic!("expected an admitted status");
    };
    let Payload::TaskStatus(record) = &admitted.payload else {
        panic!("expected a task record");
    };
    let continuation = record
        .continuation
        .value()
        .cloned()
        .expect("the parked task carries a continuation");

    let mut connection = connection_as("agent:runner", "cap_runner");
    let rendered = {
        let mut link = fixture.link();
        task::resume(
            &mut connection,
            &mut link,
            &continuation,
            CLOSING_BUDGET,
            Format::Text,
        )
        .expect("the call is answered")
    };
    assert_eq!(rendered.exit_code, 0, "{}", rendered.text);
    // `depth  served`, not the `refused  false` this command carried before bn-ybh1z: a
    // CLI-invented boolean that restated what `depth` and `error` already say, typed and
    // finer (`continuum_cli::contract`, "No invented fields").
    assert!(rendered.text.contains("depth  served"), "{}", rendered.text);
    assert!(
        rendered.text.contains(TaskStatus::Completed.as_wire()),
        "the resumed campaign closes to Die Hard's frozen verdict:\n{}",
        rendered.text
    );

    let status_after = {
        let mut link = fixture.link();
        connection
            .task_status(&mut link, &task)
            .expect("the call is answered")
    };
    let Outcome::Admitted(admitted) = &status_after else {
        panic!("expected an admitted status");
    };
    let Payload::TaskStatus(record) = &admitted.payload else {
        panic!("expected a task record");
    };
    assert_eq!(record.status, TaskStatus::Completed);
    assert_eq!(record.cost.states, Optional::Present(FROZEN_STATES));
    assert!(
        record.continuation.value().is_none(),
        "a closed campaign parks nothing"
    );
}

#[test]
fn resume_refuses_a_continuation_whose_pinned_snapshot_the_lineage_superseded() {
    let mut fixture = Fixture::fresh();
    let snapshot = sealed(&mut fixture, "req_create");
    let task = start(&mut fixture, &snapshot, PARK_BUDGET, "req_start");

    let continuation = {
        let mut connection = connection_as("agent:runner", "cap_runner");
        let mut link = fixture.link();
        let outcome = connection
            .task_status(&mut link, &task)
            .expect("the call is answered");
        let Outcome::Admitted(admitted) = &outcome else {
            panic!("expected an admitted status");
        };
        let Payload::TaskStatus(record) = &admitted.payload else {
            panic!("expected a task record");
        };
        record
            .continuation
            .value()
            .cloned()
            .expect("the parked task carries a continuation")
    };

    // The lineage moves on: a fork from the continuation's own pinned snapshot supersedes
    // it, exactly as `dx03_falsification.rs`'s attack 6 does before asserting the same
    // `StaleSnapshot` on `task.resume` against `Daemon::dispatch` directly.
    fork_past(&mut fixture, &snapshot, "req_fork");

    let mut connection = connection_as("agent:runner", "cap_runner");
    let rendered = {
        let mut link = fixture.link();
        task::resume(
            &mut connection,
            &mut link,
            &continuation,
            CLOSING_BUDGET,
            Format::Text,
        )
        .expect("the call is answered")
    };
    assert_eq!(
        rendered.exit_code, 1,
        "a refusal exits non-zero:\n{}",
        rendered.text
    );
    // The typed depth in place of the former `refused  true`, and it says more: this is a
    // refusal about *this request*, not about a surface the deployment does not serve.
    assert!(
        rendered.text.contains("depth  refused"),
        "{}",
        rendered.text
    );
    assert!(
        rendered.text.contains(ErrorCode::StaleSnapshot.as_wire()),
        "the typed reason is surfaced, not a bare failure:\n{}",
        rendered.text
    );

    // The refusal advanced nothing: the task the CLI can still read is exactly as parked.
    let status_after = {
        let mut link = fixture.link();
        connection
            .task_status(&mut link, &task)
            .expect("the call is answered")
    };
    let Outcome::Admitted(admitted) = &status_after else {
        panic!("expected an admitted status");
    };
    let Payload::TaskStatus(record) = &admitted.payload else {
        panic!("expected a task record");
    };
    assert_eq!(
        record.status,
        TaskStatus::Suspended,
        "the refused resume advanced nothing"
    );

    // The JSON rendering carries the same typed reason, structurally.
    let json = {
        let mut link = fixture.link();
        task::resume(
            &mut connection,
            &mut link,
            &continuation,
            CLOSING_BUDGET,
            Format::Json,
        )
        .expect("the call is answered")
    };
    assert_eq!(json.exit_code, 1);
    assert!(json.text.contains("\"depth\":\"refused\""), "{}", json.text);
    // …and the machine envelope keeps the response key set on the refusal arm, so a parser
    // never branches on which keys exist (bn-ybh1z).
    assert!(json.text.contains("\"record\":null"), "{}", json.text);
    assert!(json.text.contains(&format!(
        "\"code\":\"{}\"",
        ErrorCode::StaleSnapshot.as_wire()
    )));
}

#[test]
fn cancel_of_a_suspended_task_reports_a_valid_continuation_and_prints_its_handle() {
    let mut fixture = Fixture::fresh();
    let snapshot = sealed(&mut fixture, "req_create");
    let task = start(&mut fixture, &snapshot, PARK_BUDGET, "req_start");

    let continuation = {
        let mut connection = connection_as("agent:runner", "cap_runner");
        let mut link = fixture.link();
        let outcome = connection
            .task_status(&mut link, &task)
            .expect("the call is answered");
        let Outcome::Admitted(admitted) = &outcome else {
            panic!("expected an admitted status");
        };
        let Payload::TaskStatus(record) = &admitted.payload else {
            panic!("expected a task record");
        };
        record
            .continuation
            .value()
            .cloned()
            .expect("the parked task carries a continuation")
    };

    let mut connection = connection_as("agent:runner", "cap_runner");
    let outcome = {
        let mut link = fixture.link();
        connection
            .task_cancel(&mut link, &task)
            .expect("the call is answered")
    };
    let Outcome::Admitted(admitted) = &outcome else {
        panic!("cancelling a held task is admitted: {outcome:?}");
    };
    let Payload::TaskCancel(response) = &admitted.payload else {
        panic!("task.cancel answers a cancel response");
    };
    assert_eq!(
        CancelOutcome::of(response),
        CancelOutcome::ValidContinuation
    );
    assert_eq!(
        response.continuation.value().cloned(),
        Some(continuation.clone()),
        "the same continuation the task was parked under"
    );

    for format in [Format::Text, Format::Pretty, Format::Json] {
        let mut connection = connection_as("agent:runner", "cap_runner");
        let mut link = fixture.link();
        let rendered = task::cancel(&mut connection, &mut link, &task, format)
            .expect("cancelling an already-cancelled task is still admitted (INV-002)");
        assert_eq!(rendered.exit_code, 0, "{format:?}:\n{}", rendered.text);
        assert!(
            rendered
                .text
                .contains(CancelOutcome::ValidContinuation.token()),
            "{format:?}:\n{}",
            rendered.text
        );
        assert!(
            rendered.text.contains(continuation.as_str()),
            "the handle is printed in full, not truncated, in {format:?}:\n{}",
            rendered.text
        );
    }
}

#[test]
fn cancel_of_a_completed_task_reports_a_clean_absence() {
    let mut fixture = Fixture::fresh();
    let snapshot = sealed(&mut fixture, "req_create");
    // Closes outright: no suspension, no continuation ever parked.
    let task = start(&mut fixture, &snapshot, CLOSING_BUDGET, "req_start");

    let mut connection = connection_as("agent:runner", "cap_runner");
    let outcome = {
        let mut link = fixture.link();
        connection
            .task_cancel(&mut link, &task)
            .expect("the call is answered")
    };
    let Outcome::Admitted(admitted) = &outcome else {
        panic!("cancelling a held task is admitted: {outcome:?}");
    };
    let Payload::TaskCancel(response) = &admitted.payload else {
        panic!("task.cancel answers a cancel response");
    };
    assert_eq!(
        response.status,
        TaskStatus::Completed,
        "cancelling a terminal task changes nothing"
    );
    assert!(
        response.continuation.value().is_none(),
        "a closed campaign parks nothing"
    );
    let side = CancelOutcome::of(response);
    assert!(
        matches!(
            side,
            CancelOutcome::NothingPublished | CancelOutcome::ClosedAndComplete
        ),
        "either shape of a clean absence is correct here: {side:?}",
    );

    let rendered = {
        let mut link = fixture.link();
        task::cancel(&mut connection, &mut link, &task, Format::Text).expect("the call is answered")
    };
    assert_eq!(rendered.exit_code, 0, "{}", rendered.text);
    assert!(
        rendered.text.contains("continuation  none"),
        "a clean absence names its own absence, not silence:\n{}",
        rendered.text
    );
    assert!(rendered.text.contains(side.token()), "{}", rendered.text);
}
