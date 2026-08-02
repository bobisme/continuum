//! Dedicated evidence for `INV-002` — no hidden semantic state (`notes/plan/plan.md:311`,
//! the invariant bone `bn-1aqq`).
//!
//! > Every stateful workflow uses explicit handles. A dropped connection or restarted
//! > client does not change meaning.
//! >
//! > — `notes/plan/plan.md:311`
//!
//! RFC 0026 states the same property twice, once as a headline and once as an obligation:
//!
//! > Nothing is session-scoped. A dropped connection MUST NOT change the meaning of
//! > anything (INV-002). Every recoverable fact is reachable by re-reading a handle under a
//! > valid capability.
//! >
//! > — RFC 0026, "Summary"
//!
//! > No session state. The handshake establishes version, encoding, and authority only.
//! > Reconnecting with the same handles and a valid capability continues the work; a
//! > dropped connection changes nothing (INV-002). A daemon MUST NOT hold semantic state
//! > reachable only through a live connection.
//! >
//! > — RFC 0026, "Connection lifecycle"
//!
//! # Where the property lives in this crate, and where "the connection" lives
//!
//! [`Daemon`] separates the two things RFC 0026 separates: [`Daemon::state`]
//! ([`DaemonState`]) is every `ws_`/`in_`/`task_`/`cont_`-keyed fact this daemon holds —
//! workspaces, intents, the task table, the idempotency ledger, the capability tree — and
//! it is addressed by the handles a `RequestEnvelope` names, never by which caller is
//! asking. `Daemon`'s other half, [`Services`](continuumd::daemon::Services), carries
//! exactly the three things a handshake fixes (the negotiated version and encoding, and the
//! capability the connection itself presented) and nothing else — `transport::mod`'s own
//! module doc says outright that this split is *why* "a dropped connection costs nothing".
//! No operation family this file touches reaches into `Services` for anything but the
//! version check and the T4 capability-narrowing test; every semantic decision — whether a
//! continuation exists, what it pins, what a task's cost is — reads `DaemonState` alone.
//!
//! In this crate's data model, one `Daemon` value bundles one connection's `Services`
//! together with the whole of `DaemonState` (a single-connection scaffold —
//! `transport::Server` owns one `Daemon` by value, and `Daemon::dispatch` takes `&mut self`
//! precisely so two requests are never in flight at once). A real dropped-connection/
//! reconnect has no smaller unit to act on here than the whole `Daemon` object, so this file
//! simulates it the only way the type honestly allows: [`reconnect`] moves `DaemonState` —
//! and *only* `DaemonState` — out of the old connection's `Daemon` and into a freshly,
//! independently built one, exactly the value RFC 0026 calls "every recoverable fact". The
//! old `Daemon` is left holding an empty [`DaemonState::new`] and is never dispatched
//! against again; nothing about its `Services` — its negotiated version, its own presented
//! capability — is available to the new one, which builds its own from scratch. `store`
//! ([`continuum_workspace::publication::ReferenceStore`]) and `store_audit` are *not*
//! carried across, and that is not an oversight: `WorkspaceFamily::create`/`seal` are the
//! only handlers in this crate that touch `store` at all (`TaskFamily` and
//! `VerificationFamily` both take it as `_store`, an unused parameter, in
//! `crates/continuumd/src/daemon/task.rs` and `verification.rs`), and every step this file
//! runs after a simulated reconnect is a `task.*` or `verification.*` operation. The one
//! `workspace.create` this file issues happens entirely before the drop.
//!
//! # Clause → test map
//!
//! | Clause | Test |
//! |---|---|
//! | positive: a workflow parked on one connection, resumed to completion on a dropped-and-reconnected one, reaches a byte-identical result to the same workflow never interrupted | [`positive_a_reconnected_client_completes_the_parked_campaign_to_a_byte_identical_result`] |
//! | negative/mutant: a daemon that never received the transplanted state — i.e., one for which the continuation table behaved as connection-scoped, hidden state — denies the identical resume the honest reconnect admits | [`negative_a_lookalike_daemon_that_never_ran_the_campaign_denies_the_resume_the_honest_reconnect_admits`] |
//! | boundary: the protocol epoch *is* connection-scoped by RFC 0026, and a continuation structurally excludes it from what it pins | [`boundary_pinned_epochs_excludes_the_protocol_epoch_by_construction`] |
//! | boundary: a resume is never refused for a protocol-only disagreement, though it is refused for a real compatibility-epoch one (the anti-vacuity half) | [`boundary_resume_is_never_refused_for_a_protocol_only_disagreement`] |
//!
//! The Die Hard fixture (model, contract, workspace file) is the same one
//! `daemon_task_operations.rs` and `pr8_exit_evidence.rs` use, `include_str!`'d rather than
//! copied for the reason those files give: one Die Hard in this repository. The park point
//! (a `states: 4` budget) and the completion point (`states: 64`) are the same two numbers
//! `daemon_task_operations.rs`'s
//! `budget_exhaustion_parks_a_continuation_that_update_budget_and_resume_complete` already
//! proved reliable.

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::codec::operations::encode_payload;
use continuumd::codec::to_bytes;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{DaemonState, IntentRecord, RegistryStatus};
use continuumd::daemon::task::{PinnedEpochs, TaskFamily, admissible_epochs};
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope, Verdict};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::task::{
    TaskResumeRequest, TaskStatusRequest, TaskUpdateBudgetRequest,
};
use continuumd::protocol::operations::verification::{
    VerificationResultRequest, VerificationStartRequest,
};
use continuumd::protocol::operations::workspace::WorkspaceCreateRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, ContinuationHandle, EpochIdentity, IntentHandle, Opaque,
    OperationName, ProtocolVersion, RequestId, TaskHandle, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, Portfolio, ResultStatus, SemanticVerdict, TargetKind,
    TaskStatus,
};

/// The TV-009 port's model, verbatim — the bytes that go into the snapshot.
const DIE_HARD_MODEL: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");

/// The TV-009 port's default model configuration.
const DIE_HARD_CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");

/// The Die Hard Intent Contract, as `continuum-intent`'s own suites use it.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// Where the model lives inside the workspace.
const MODULE_PATH: &str = "DieHard.ctm";

// ---------------------------------------------------------------------------------------
// shared fixture plumbing (the same idioms `daemon_task_operations.rs` and
// `inv006_replay_stability_evidence.rs` use)
// ---------------------------------------------------------------------------------------

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 1)
}

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("a well-formed capability handle")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("a well-formed actor identity")
}

fn name(operation: &str) -> OperationName {
    OperationName::new(operation).expect("a well-formed operation name")
}

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
}

fn grant(
    handle: &str,
    actor: &str,
    level: AuthorityLevel,
    depth: u32,
    profile: Optional<CapabilityProfile>,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(handle),
        actor: who(actor),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: depth,
        profile,
    }
}

fn profile(privileged: &[&str]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged.iter().map(|entry| name(entry)).collect(),
        denied_operations: Vec::new(),
        data_grants: Vec::new(),
        cross_principal_sharing: true,
    }
}

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-inv002-evidence-test".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.1 is served")
}

/// The epochs this daemon serves. Pinned rather than unpinned, so a continuation has
/// something to pin and the resume predicate has something to disagree with.
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

fn now() -> Timestamp {
    Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp")
}

/// A daemon with all four families registered and every capability the workflow below
/// needs, presenting `connection` as the connection's own capability — the one thing that
/// varies between "connection A" and its reconnect.
fn shell(connection: &str) -> Daemon {
    let root = Some(cap("cap_root"));
    Daemon::builder(Blake3Identity, negotiated(), cap(connection))
        .epochs(epochs())
        .now(now())
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Present(profile(&["intent.accept", "intent.reject", "intent.lock"])),
            ),
            None,
        )
        .capability(
            grant(
                "cap_builder",
                "agent:builder",
                AuthorityLevel::Propose,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_runner",
                "agent:runner",
                AuthorityLevel::Execute,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_reader",
                "agent:reader",
                AuthorityLevel::Read,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_steward",
                "human:steward",
                AuthorityLevel::ReviseIntent,
                3,
                Optional::Present(profile(&["intent.accept", "intent.reject", "intent.lock"])),
            ),
            root,
        )
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(TaskFamily)
        .family(VerificationFamily)
        .build()
}

/// Move `DaemonState` — the whole of what RFC 0026 calls "every recoverable fact" — out of
/// `old` and into a freshly, independently negotiated daemon presenting `connection` as its
/// own capability. `old` is left holding an empty [`DaemonState::new`] and MUST NOT be
/// dispatched against again: it plays the part of the closed socket. Nothing about `old`'s
/// own `Services` — its `Negotiated`, its own connection capability — reaches the returned
/// daemon; that daemon's `Services` is built from scratch by [`shell`], which is the whole
/// point.
fn reconnect(old: &mut Daemon, connection: &str) -> Daemon {
    let mut fresh = shell(connection);
    *fresh.state_mut() = std::mem::replace(old.state_mut(), DaemonState::new());
    fresh
}

fn die_hard_contract() -> IntentContract {
    IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes()).expect("the fixture decodes")
}

fn intent_handle(contract: &IntentContract) -> IntentHandle {
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        continuum_workspace::artifact_path::ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    continuumd::daemon::identity::intent_to_wire(&stored).expect("an `in_` handle")
}

/// The content identity the daemon derives for a snapshot carrying only `DieHard.ctm`.
fn die_hard_source() -> Commitment {
    model_source(&Blake3Identity, [(MODULE_PATH, DIE_HARD_MODEL.as_bytes())])
        .expect("blake3 names the module set")
}

fn acceptance_bytes() -> Opaque {
    let mut fields: std::collections::BTreeMap<String, Json> = std::collections::BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "human:steward"),
        ("capability", "revise-intent"),
        ("signature", "sig-die-hard-v1"),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", "2026-08-01T00:00:00.000Z"),
    ] {
        fields.insert(key.to_owned(), Json::String(value.to_owned()));
    }
    Opaque::from_bytes(Json::Object(fields).to_canonical_bytes())
}

fn envelope(operation: &str, actor: &str, capability: &str, request: &str) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request).expect("a well-formed request id"),
        idempotency_key: Optional::Absent,
        actor: who(actor),
        capability: cap(capability),
        operation: name(operation),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: Optional::Absent,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

fn keyed(mut envelope: RequestEnvelope, key: &str) -> RequestEnvelope {
    envelope.idempotency_key = Optional::Present(key.to_owned());
    envelope
}

fn budget(states: Option<u64>) -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: match states {
            Some(states) => Optional::Present(states),
            None => Optional::Absent,
        },
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

fn budgeted(mut envelope: RequestEnvelope, states: Option<u64>) -> RequestEnvelope {
    envelope.budget = Optional::Present(budget(states));
    envelope
}

fn on(mut envelope: RequestEnvelope, snapshot: &WorkspaceHandle) -> RequestEnvelope {
    envelope.snapshot = Nullable::Value(snapshot.clone());
    envelope
}

/// A daemon holding the Die Hard workspace sealed and its contract accepted, connected as
/// `cap_root` — "connection A".
fn bootstrap() -> (Daemon, WorkspaceHandle) {
    let mut daemon = shell("cap_root");
    let contract = die_hard_contract();
    let intent = intent_handle(&contract);
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

    let mut files = Vec::new();
    for (path, content) in [(MODULE_PATH, DIE_HARD_MODEL), ("README.md", "# TV-009\n")] {
        files.push(
            daemon
                .state_mut()
                .stage(
                    &Blake3Identity,
                    WorkspacePath::new(path).expect("a workspace path"),
                    content.as_bytes().to_vec(),
                )
                .expect("staging names its content"),
        );
    }
    let configuration = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("default.model.toml").expect("a workspace path"),
            DIE_HARD_CONFIG.as_bytes().to_vec(),
        )
        .expect("staging names its content");

    daemon.state_mut().models_mut().register(
        die_hard_source(),
        diehard::model().expect("the port builds"),
    );

    let accepted = daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "intent.accept",
                "human:steward",
                "cap_steward",
                "req_accept",
            ),
            "idem-accept",
        ),
        arguments: Arguments::IntentAccept(IntentAcceptRequest {
            proposal: intent.clone(),
            acceptance: acceptance_bytes(),
            bundle: Optional::Absent,
        }),
    });
    assert_eq!(
        accepted.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        accepted.envelope.error
    );

    let created = daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "workspace.create",
                "agent:builder",
                "cap_builder",
                "req_create",
            ),
            "idem-create",
        ),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: SnapshotComponents {
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
                intent: intent.clone(),
                correspondence: Vec::new(),
                proof_environment: Vec::new(),
                configuration: vec![configuration],
                file_components: Optional::Absent,
            },
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    });
    assert_eq!(
        created.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        created.envelope.error
    );
    let snapshot = match &created.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    };

    (daemon, snapshot)
}

/// `verification.start` over the fixture's sealed snapshot, budgeted to park deterministically
/// (the same `states: 4` `daemon_task_operations.rs` proved reliable for this exact model).
fn start_campaign(daemon: &mut Daemon, snapshot: &WorkspaceHandle) -> TaskHandle {
    let started = daemon.dispatch(&OperationRequest {
        envelope: on(
            budgeted(
                keyed(
                    envelope(
                        "verification.start",
                        "agent:runner",
                        "cap_runner",
                        "req_start",
                    ),
                    "idem-start",
                ),
                Some(4),
            ),
            snapshot,
        ),
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target: Target {
                kind: TargetKind::AllClaims,
                id: "DieHard".to_owned(),
            },
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    });
    assert_eq!(
        started.envelope.status,
        ResultStatus::TaskSuspended,
        "a `states: 4` budget parks Die Hard within the one synchronous dispatch that starts \
         it, so `@task_starting`'s own status lane is `task_suspended`, not `task_started` \
         (`verification.rs`'s own `started` doc comment): {:?}",
        started.envelope.error
    );
    match &started.payload {
        Payload::VerificationStart(response) => response
            .task
            .value()
            .cloned()
            .expect("a fresh start names a task"),
        other => panic!("expected a verification.start payload, got {other:?}"),
    }
}

/// `task.status`, read as `agent:reader`/`cap_reader` — a different actor and capability
/// than the one that started or will resume the campaign, so nothing about the read-back
/// depends on which identity is asking.
fn read_continuation(daemon: &mut Daemon, task: &TaskHandle) -> ContinuationHandle {
    let status = daemon.dispatch(&OperationRequest {
        envelope: envelope("task.status", "agent:reader", "cap_reader", "req_status"),
        arguments: Arguments::TaskStatus(TaskStatusRequest { task: task.clone() }),
    });
    let record = match &status.payload {
        Payload::TaskStatus(record) => record.clone(),
        other => panic!("expected a task.status payload, got {other:?}"),
    };
    assert_eq!(
        record.status,
        TaskStatus::Suspended,
        "a states:4 budget parks Die Hard"
    );
    record
        .continuation
        .value()
        .cloned()
        .expect("a suspended task is resumable by definition (plan §11.4)")
}

/// One parked campaign: the daemon that ran it (still connected), and the two handles a
/// caller would hold onto across a reconnect.
struct Parked {
    daemon: Daemon,
    task: TaskHandle,
    continuation: ContinuationHandle,
}

fn drive_to_park() -> Parked {
    let (mut daemon, snapshot) = bootstrap();
    let task = start_campaign(&mut daemon, &snapshot);
    let continuation = read_continuation(&mut daemon, &task);
    Parked {
        daemon,
        task,
        continuation,
    }
}

fn update_budget_call(daemon: &mut Daemon, task: &TaskHandle) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "task.update_budget",
                "agent:runner",
                "cap_runner",
                "req_budget",
            ),
            "idem-budget",
        ),
        arguments: Arguments::TaskUpdateBudget(TaskUpdateBudgetRequest {
            task: task.clone(),
            budget: budget(Some(64)),
        }),
    })
}

fn resume_call(daemon: &mut Daemon, continuation: &ContinuationHandle) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: budgeted(
            keyed(
                envelope("task.resume", "agent:runner", "cap_runner", "req_resume"),
                "idem-resume",
            ),
            Some(64),
        ),
        arguments: Arguments::TaskResume(TaskResumeRequest {
            continuation: continuation.clone(),
            // Already raised via `task.update_budget` above; RFC 0026 names both as valid
            // ways to re-admit a parked task and `daemon_task_operations.rs`'s own
            // `budget_exhaustion_parks_a_continuation_…` composes them the same way.
            budget: Optional::Absent,
        }),
    })
}

fn result_call(daemon: &mut Daemon, task: &TaskHandle) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: envelope(
            "verification.result",
            "agent:runner",
            "cap_runner",
            "req_result",
        ),
        arguments: Arguments::VerificationResult(VerificationResultRequest { task: task.clone() }),
    })
}

/// The wire bytes of one result: the envelope (whose own `payload` field the operation
/// layer always leaves `null`, by design — see `transport::mod`'s module doc) followed by
/// the typed payload's own encoding, exactly as `transport::Server::answer` splices them
/// into one frame. Two calls compare byte-identical here if and only if they would have
/// produced the identical frame on the real wire.
fn wire_bytes(outcome: &OperationOutcome) -> Vec<u8> {
    let mut bytes = to_bytes(&outcome.envelope).expect("a result envelope encodes");
    if let Some(payload) = encode_payload(&outcome.payload).expect("a payload encodes") {
        bytes.extend_from_slice(payload.as_bytes());
    }
    bytes
}

/// Raise the budget, resume to completion, and read the final verdict — the three requests
/// a caller issues after a park, wherever they land. Returns the wire bytes of all three, in
/// order, plus the last (typed, for readable assertions).
fn finish(
    daemon: &mut Daemon,
    task: &TaskHandle,
    continuation: &ContinuationHandle,
) -> (Vec<Vec<u8>>, OperationOutcome) {
    let raised = update_budget_call(daemon, task);
    let resumed = resume_call(daemon, continuation);
    let result = result_call(daemon, task);
    (
        vec![
            wire_bytes(&raised),
            wire_bytes(&resumed),
            wire_bytes(&result),
        ],
        result,
    )
}

// ---------------------------------------------------------------------------------------
// positive: meaning survives the reconnect
// ---------------------------------------------------------------------------------------

/// **Positive.** The same Die Hard campaign, parked at `states: 4`, is driven to
/// completion two ways: entirely on the connection that started it, and — after that
/// connection is dropped and [`reconnect`] carries only `DaemonState` to a freshly,
/// independently negotiated one presenting a *different* capability (`cap_runner` rather
/// than the original `cap_root`) — resumed and finished on the reconnect. `task.
/// update_budget`, `task.resume`, and `verification.result` produce byte-identical wire
/// frames either way: the continuation minted on connection A is admitted, resumed, and
/// completed on connection B to the identical semantic verdict, because what answers the
/// request is `DaemonState` addressed by the `task_`/`cont_` handles the caller holds, not
/// anything about which connection is asking.
#[test]
fn positive_a_reconnected_client_completes_the_parked_campaign_to_a_byte_identical_result() {
    let mut continuous = drive_to_park();
    let mut split = drive_to_park();
    assert_eq!(
        continuous.task, split.task,
        "the task identity is a function of the campaign alone (PR 5 exit), so two \
         independent runs of the identical script mint the identical handle before any \
         reconnect is even simulated"
    );
    assert_eq!(continuous.continuation, split.continuation);

    // Connection A drops. Only `DaemonState` — the parked continuation included — survives
    // into a freshly negotiated connection B, presenting a capability connection A never
    // used (`cap_runner`, not `cap_root`).
    let mut reconnected = reconnect(&mut split.daemon, "cap_runner");

    let (continuous_frames, continuous_result) = finish(
        &mut continuous.daemon,
        &continuous.task,
        &continuous.continuation,
    );
    let (split_frames, split_result) = finish(&mut reconnected, &split.task, &split.continuation);

    assert_eq!(
        continuous_frames.len(),
        3,
        "update_budget, resume, and verification.result — nothing silently skipped"
    );
    assert_eq!(continuous_frames.len(), split_frames.len());
    for (index, (one, two)) in continuous_frames.iter().zip(&split_frames).enumerate() {
        assert_eq!(
            one, two,
            "frame {index} (update_budget=0, resume=1, verification.result=2) differed \
             between the never-interrupted connection and the dropped-and-reconnected one"
        );
    }

    // The comparison above is over real, successful results, not two errors that happen to
    // agree: the continuous run reaches the frozen Die Hard verdict, and the byte-identical
    // assertion says the reconnected run reaches the same one.
    assert_eq!(
        continuous_result.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        continuous_result.envelope.error
    );
    match &continuous_result.envelope.verdict {
        Nullable::Value(Verdict::Semantic(value)) => {
            assert_eq!(
                value.verdict,
                SemanticVerdict::Refuted,
                "NotSolved is refuted by the film's six-step solution, over the closed \
                 exploration `states: 64` admits"
            );
        }
        other => panic!("expected a semantic verdict, got {other:?}"),
    }
    match &split_result.payload {
        Payload::VerificationResult(response) => assert_eq!(response.task, split.task),
        other => panic!("expected a verification.result payload, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------
// negative / mutant: the guard is not vacuous
// ---------------------------------------------------------------------------------------

/// **Negative, anti-vacuity.** The positive test's claim rests on `DaemonState` being what
/// a resume is checked against — not the connection, not the daemon object's own identity.
/// This test proves that claim is not vacuously true by constructing the one hypothetical
/// the bone asks for: a "connection-scoped" task table. A second daemon is bootstrapped
/// with the *identical* deterministic setup — same intent, same sealed workspace, an
/// outside observer diffing the two would see no difference in either — but it never
/// itself runs `verification.start`, so its own task table simply has no entry for the
/// continuation connection A minted. If the daemon's task/continuation state were the
/// hidden, connection-scoped value INV-002 forbids — reachable only through the connection
/// that started the campaign, rather than through `DaemonState` under the handle — this is
/// exactly what "reconnecting" to it would look like. `task.update_budget` and
/// `task.resume` both deny it (`ErrorCode::CapabilityDenied`, RFC 0027 X1's "no
/// distinguishable not-found" — `crates/continuumd/src/daemon/task.rs:966`), the same two
/// requests the positive test's honest reconnect admits and completes. Swapping only which
/// daemon's state answers the identical request — nothing about the request itself changes
/// — flips the outcome from admitted-and-byte-identical to denied, which is what makes the
/// positive test's comparison a real guard rather than one that would pass no matter what.
#[test]
fn negative_a_lookalike_daemon_that_never_ran_the_campaign_denies_the_resume_the_honest_reconnect_admits()
 {
    let mut run = drive_to_park();

    // The honest reconnect (as the positive test does it): admitted.
    let mut reconnected = reconnect(&mut run.daemon, "cap_runner");
    let admitted = update_budget_call(&mut reconnected, &run.task);
    assert_eq!(
        admitted.envelope.status,
        ResultStatus::Ok,
        "a real reconnect, carrying `DaemonState`, is admitted: {:?}",
        admitted.envelope.error
    );

    // The look-alike: same intent, same sealed workspace, never ran the campaign. Its task
    // table holds nothing for `run.continuation` or `run.task` — the hypothetical world
    // where that table was the connection's, not the daemon's.
    let (mut lookalike, _snapshot) = bootstrap();
    let denied_budget = update_budget_call(&mut lookalike, &run.task);
    assert_eq!(
        denied_budget.error_code(),
        Some(ErrorCode::CapabilityDenied),
        "a daemon holding no entry for this task denies rather than silently ignoring it: {:?}",
        denied_budget.envelope.error
    );

    let denied_resume = resume_call(&mut lookalike, &run.continuation);
    assert_eq!(
        denied_resume.error_code(),
        Some(ErrorCode::CapabilityDenied),
        "same denial for `task.resume` against the identical continuation handle: {:?}",
        denied_resume.envelope.error
    );
}

// ---------------------------------------------------------------------------------------
// boundary: the one thing that IS connection-scoped, and why it isn't semantic state
// ---------------------------------------------------------------------------------------

/// **Boundary.** RFC 0026 is explicit that exactly one epoch is connection-scoped:
///
/// > The protocol epoch does not participate. A continuation MUST leave `protocol`
/// > `Unpinned` in its pinned set: the protocol epoch is connection-scoped and is consumed
/// > by no task. […] Protocol compatibility on resume is decided by the handshake's N and
/// > N−1 window and reported as `ProtocolVersionUnsupported`. A daemon MUST NOT reject a
/// > resume with `ContinuationEpochMismatch` for a protocol-minor difference.
/// >
/// > — RFC 0026, "The two-predicate obligation"
///
/// This is not a violation of INV-002 because `protocol` is not a *fact a workflow
/// depends on for meaning* — it is a negotiation artifact of the transport, re-derived
/// fresh at every handshake, and RFC 0026 says a client that wants a different one "opens a
/// new connection" rather than asking this one to change meaning under it. `PinnedEpochs`
/// (`crates/continuumd/src/daemon/task.rs`) is the type that makes this a property of the
/// code rather than a rule an implementer has to remember: it has no `protocol` field at
/// all, so two connections that negotiated different protocol versions pin identical
/// compatibility state.
#[test]
fn boundary_pinned_epochs_excludes_the_protocol_epoch_by_construction() {
    let mut connection_a = epochs();
    connection_a.protocol = ProtocolVersion::new(3, 1);
    let mut connection_b = epochs();
    connection_b.protocol = ProtocolVersion::new(19, 4);

    assert_ne!(
        connection_a.protocol, connection_b.protocol,
        "the two connections really did negotiate different protocol epochs"
    );
    assert_eq!(
        PinnedEpochs::of(&connection_a),
        PinnedEpochs::of(&connection_b),
        "`PinnedEpochs` has no `protocol` field to disagree in, so a continuation pins the \
         same compatibility state regardless of which protocol version its connection \
         negotiated"
    );
}

/// **Boundary, live.** The structural fact above holds at the actual predicate
/// [`admissible_epochs`] uses to decide `task.resume`: a continuation whose pinned epochs
/// otherwise agree with the daemon's current ones is admissible even when read against a
/// wildly different "current" protocol epoch — the disagreement never reaches the
/// function's protocol slot because there is no such slot. The second half is the
/// anti-vacuity check: the *same* function, given a real compatibility-epoch disagreement
/// (the semantic epoch, which — unlike `protocol` — is one of the five `PinnedEpochs`
/// actually carries) refuses with `ContinuationEpochMismatch`. So the silence on `protocol`
/// above is a decision about that one epoch specifically, not evidence that this predicate
/// never refuses anything.
#[test]
fn boundary_resume_is_never_refused_for_a_protocol_only_disagreement() {
    let pinned = PinnedEpochs::of(&epochs());

    let mut reconnected_under_a_new_protocol_version = epochs();
    reconnected_under_a_new_protocol_version.protocol = ProtocolVersion::new(3, 9);
    assert_ne!(
        epochs().protocol,
        reconnected_under_a_new_protocol_version.protocol
    );
    assert!(
        admissible_epochs(&pinned, &reconnected_under_a_new_protocol_version).is_ok(),
        "RFC 0026: a daemon MUST NOT reject a resume with `ContinuationEpochMismatch` for a \
         protocol-minor difference"
    );

    let mut incompatible = epochs();
    incompatible.semantic = Nullable::Value(epoch("semantic-2"));
    let fault = admissible_epochs(&pinned, &incompatible)
        .expect_err("a real semantic-epoch disagreement is refused, unlike a protocol one");
    assert_eq!(fault.code, ErrorCode::ContinuationEpochMismatch);
}
