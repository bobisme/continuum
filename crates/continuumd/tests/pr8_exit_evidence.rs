//! Dedicated exit evidence for `PR-8-EXIT` (`notes/plan/notes/PLAN_REQUIREMENTS.json`, id
//! `PR-8-EXIT`; `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 8's Exit line).
//!
//! > **Exit:** Die Hard returns 16 states and depth-6 solution through the daemon API.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 8
//!
//! This file asserts that sentence end to end, as directly as it reads, the way
//! `pr2_exit_evidence.rs` and `pr3_exit_evidence.rs` did for PR 2's and PR 3's exits
//! (`continuum-workspace`) and `transport_local.rs`'s own PR-5-exit tests did one layer
//! down in this crate. PR 8 landed across five engine bones —
//! `crates/continuum-engine-reference/src/{ident,domain,expr,model,diehard}.rs` (bn-2d0e,
//! the model), `src/bfs.rs` (bn-3p8u, deterministic exploration), `src/checking.rs`
//! (bn-3e0m, invariant/deadlock verdicts), `src/witness.rs` (bn-2pmc, the shortest path),
//! and `src/certificate.rs` (bn-3e4l, the CONTCERT emission) — plus the daemon wiring that
//! put a campaign behind the wire at all (`crates/continuumd/src/daemon/{task,
//! verification}.rs`, bn-18z, and the protocol-3.2 transport, bn-i4aem/bn-3bhkp). Every one
//! of those already carries its own slice of the exit sentence as a unit- or
//! operation-layer fact, and `daemon_task_operations.rs`'s
//! [`die_hard_returns_sixteen_states_and_a_depth_six_solution_through_the_daemon_api`] is
//! already a named, standalone witness of the *whole* sentence — through
//! [`Daemon::dispatch`], typed arguments in, typed payload out, no codec and no byte
//! boundary anywhere in the call.
//!
//! What none of them is is a witness of the sentence at the grain "through the daemon
//! **API**" can also honestly be read to mean: the actual bytes a real client sends and
//! receives, the way `transport_local.rs` proved the PR-5 exit at the transport rather than
//! at `dispatch`. This file is that witness for PR 8 — handshake, workspace staging,
//! `workspace.create`, `workspace.seal`, `verification.start` and `task.status`, each as a
//! real frame across a real [`LocalPair`], decoded by the client side of the boundary
//! rather than read off a Rust value the daemon handed back in-process. Everywhere else it
//! needs machinery, it reruns `transport_local.rs`'s idioms — the `Wire` harness, the
//! `exchange` method, the `include_str!`'d Die Hard fixtures — duplicated locally rather
//! than imported, because a `tests/*.rs` file is its own crate and nothing here can `use` a
//! sibling one.
//!
//! # The honesty this file exists to be precise about
//!
//! "16 states and depth-6 solution" is two numbers, and only one of them ever crosses a
//! frame in this workspace today. `crates/continuumd/src/daemon/task.rs`'s own module
//! documentation ("Where the frozen Die Hard facts are, and which of the two gaps is the
//! wire's") states this plainly, and this file takes it at its word rather than re-deciding
//! it:
//!
//! - **16** rides `TaskRecord.cost.states` — RFC 0026's `Cost` declares exactly this
//!   dimension, and it is a real field of a real decoded [`task.status`][TaskStatusRequest]
//!   response frame in [`positive_die_hard_returns_sixteen_states_through_the_wire_and_the_depth_six_solution_one_layer_down`]
//!   below.
//! - **96** (the labelled transitions) has no wire field **and cannot get one at a minor**:
//!   `Cost` and `Budget` share one nine-dimension list SD-12 holds identical across this
//!   protocol, `schemas/verification-task.schema.json`, and the plan §8.6 cost ledger, so a
//!   tenth dimension moves four artifacts at once. Recorded as RFC 0026 F16. It is read, in
//!   this file, from [`Daemon::state`] — a plain Rust reference into the same process, never
//!   a decoded frame — and the test says so at the point it reads it.
//! - **depth-6** (the witness) *does* have a wire home — `VerificationResult.crashpack`,
//!   whose class is `schemas/crashpack.schema.json` — and it is absent on every response
//!   this daemon produces, because nothing in this workspace builds a crashpack. `task.rs`
//!   calls this "an implementation gap wearing a wire gap's clothes": the field exists, the
//!   producer does not. This file confirms the field is genuinely absent on the actual wire
//!   response (rather than assuming it), and then reads the number the same way as the 96 —
//!   from `Daemon::state`, off the record.
//!
//! Neither gap is papered over by inventing a place to put the missing number. The exit
//! sentence's "through the daemon API" is therefore honoured exactly as far as it is true:
//! the reachable-state count crosses the wire; the transition count and the witness are
//! corroborated one layer down, in the same daemon, through the same campaign, and that
//! boundary is the whole reason this file exists rather than a shorter one that just
//! reruns `daemon_task_operations.rs` through frames.
//!
//! # Evidence map
//!
//! - **"Die Hard returns 16 states … through the daemon API"**, and the honesty boundary
//!   above —
//!   [`positive_die_hard_returns_sixteen_states_through_the_wire_and_the_depth_six_solution_one_layer_down`].
//!   A handshake (two frames — [`encode_hello`] then [`Server::open`]'s welcome), Die Hard's
//!   corpus staged and its model registered out of band (deliberately not frames — see
//!   "What does not cross a frame, and why" below), `workspace.create` unsealed (one
//!   frame), a separate `workspace.seal` (a second frame — the two-step path, not
//!   `create`'s `seal: true` shortcut `transport_local.rs` uses), `verification.start` (a
//!   frame that starts the campaign), `task.status` (the frame the frozen 16 is read off),
//!   and `verification.result` (a frame confirming `crashpack` is genuinely absent, not
//!   assumed). `Cost` is destructured exhaustively with no `..`, so a wire revision that
//!   ever added an eleventh, transitions-shaped field would fail this file's own
//!   compilation — F16 held as a fact about this file's shape, not only a runtime check of
//!   today's schema. The 96 and the depth-6 are then read from `Daemon::state` with an
//!   explicit note that no frame crossed between that line and the one before it.
//!   Corroborated by `daemon_task_operations.rs`'s own
//!   [`die_hard_returns_sixteen_states_and_a_depth_six_solution_through_the_daemon_api`] —
//!   the same claim, same daemon shape, at the `dispatch` grain rather than the byte grain.
//! - **determinism, the canonical-form discipline end to end** —
//!   [`positive_the_whole_frame_sequence_replayed_from_a_fresh_daemon_is_byte_identical`].
//!   Two independent daemons, built from scratch, driven through the identical scripted
//!   exchange — handshake, create, seal, start, status, result — produce byte-identical
//!   transcripts, frame for frame. This is a stronger claim than
//!   `transport_local.rs`'s `replaying_an_idempotent_request_through_the_transport_returns_a_byte_identical_frame`,
//!   which replays one request against *one* already-built daemon: here, nothing is shared
//!   between the two runs but the script itself, so no daemon-instance-specific state (an
//!   iteration order, a map's internal layout) can be leaking into the wire either.
//! - **negative: a tampered frame is rejected, never silently dispatched** —
//!   [`negative_a_single_byte_mutation_of_the_verification_start_frame_is_rejected_not_dispatched`].
//!   The exact bytes of a genuine, dispatchable `verification.start` request are captured
//!   (and proved genuine — sent unmutated first, and it starts the campaign); two single-byte
//!   mutations of that same message are then answered: one flips the frame's leading `{`,
//!   which fails to decode at all, so the connection owes no correlated answer (`transport`
//!   module's own reasoning) and the request never reaches a family handler; the other
//!   flips the last byte of the embedded `"target"` field name, which still decodes as an
//!   envelope but is not `VerificationStartRequest`'s shape, and is answered with a typed
//!   `MalformedRequest` result, request ID echoed, payload null — the same path
//!   `transport_local.rs`'s `a_request_body_that_does_not_decode_is_answered_rather_than_dropped`
//!   exercises with a hand-built bad body, exercised here against one real byte of one real
//!   captured frame. Neither mutation grows the task table beyond the one genuine task the
//!   unmutated frame started: tampering it never mints a phantom Die Hard result.
//!   Corroborated, at the byte-injectivity grain rather than the dispatch grain, by
//!   `codec_canonical_form.rs`'s
//!   `no_single_byte_mutation_of_a_golden_vector_decodes_to_the_same_document`, whose golden
//!   vectors include this exact operation's request shape.
//! - **negative: an unregistered model is `UnsupportedSemanticFeature`, through the wire** —
//!   [`negative_a_request_against_an_unregistered_model_is_unsupported_semantic_feature_through_the_wire`].
//!   The Die Hard corpus file itself is forked to different bytes, through a
//!   `workspace.fork` frame, and sealed, through a `workspace.seal` frame: a changed model
//!   is a different model (`verification.rs`'s own module doc), and nothing is registered
//!   for it. `verification.start` against that snapshot answers `UnsupportedSemanticFeature`
//!   with a null payload and no task minted. Corroborated by
//!   `daemon_task_operations.rs`'s `a_snapshot_whose_modules_are_not_a_registered_model_is_unsupported`
//!   — the identical shape, one layer down, without a byte boundary.
//!
//! # What does not cross a frame, and why
//!
//! Content staging and model registration are never frames in this protocol version.
//! `Server::daemon_mut`'s own documentation names them directly: "out-of-band
//! administration — capability registration, content staging, model registration. Those
//! are deliberately not operations (IDL §7), so they are deliberately not frames." Every
//! fixture in this file performs them before a connection exists, exactly as
//! `transport_local.rs`'s own `wire()` and `daemon_task_operations.rs`'s `fixture()` both
//! already do — a deployment provisions a daemon before any client connects to it, and RFC
//! 0026 gives a connection no session-scoped semantic state anyway (INV-002), so where
//! administration sits relative to the handshake carries no meaning a test could assert.
//! `intent.accept` is the one genuine wire *operation* this file still calls directly
//! through [`Daemon::dispatch`] rather than as a frame — the same choice
//! `transport_local.rs`'s own `wire()` makes, for the same stated reason: accepting the
//! governing intent is a precondition of `workspace.create`, not a fact the PR-8 exit
//! sentence is about.
//!
//! # House rules, inherited from `transport_local.rs` and `pr3_exit_evidence.rs`
//!
//! - **`src/` is not touched**, and no existing test in this crate — or any other — is
//!   edited, weakened, or moved.
//! - **A real byte boundary, every time.** Every positive assertion in this file is read
//!   off a `Vec<u8>` the client actually decoded, not off a Rust value `dispatch` handed
//!   back in the same process — that is the entire reason this file exists next to
//!   `daemon_task_operations.rs` rather than instead of it.
//! - **Assertions are on bytes and typed values, never on timing.** This file makes no
//!   concurrency claim; every daemon here is driven by one thread, one request at a time,
//!   which is `continuumd::transport`'s own documented grain (`Daemon::dispatch` takes
//!   `&mut self`).
//!
//! [`Daemon::dispatch`]: continuumd::daemon::Daemon::dispatch
//! [`Daemon::state`]: continuumd::daemon::Daemon::state
//! [`LocalPair`]: continuumd::transport::LocalPair
//! [`encode_hello`]: continuumd::transport::encode_hello
//! [`Server::open`]: continuumd::transport::Server::open
//! [`TaskStatusRequest`]: continuumd::protocol::operations::task::TaskStatusRequest
//! [`die_hard_returns_sixteen_states_and_a_depth_six_solution_through_the_daemon_api`]: ../daemon_task_operations.rs

use continuum_engine_reference::diehard;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::codec::from_bytes;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationRequest};
use continuumd::protocol::envelope::{Budget, Cost, EpochSet, RequestEnvelope, ResultEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, ServerLimits, ServerWelcome,
    VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::task::TaskStatusRequest;
use continuumd::protocol::operations::verification::{
    VerificationResultRequest, VerificationStartRequest,
};
use continuumd::protocol::operations::workspace::{
    WorkspaceCreateRequest, WorkspaceForkRequest, WorkspaceSealRequest,
};
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, ByteCount, CapabilityHandle, Commitment, DurationMs, EpochIdentity, IntentHandle,
    Opaque, OperationName, ProtocolVersion, RequestId, TaskHandle, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{
    FileComponent, FileOverlay, SnapshotComponents, SnapshotEpochs, Target,
};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::task::TaskRecord;
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, Portfolio, PriorityClass, ResultStatus, TargetKind,
    TaskStatus,
};
use continuumd::transport::{
    LocalPair, Server, client_receive, client_send, encode_hello, encode_request,
};

const DIE_HARD_MODEL: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");
const DIE_HARD_CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");
const MODULE_PATH: &str = "DieHard.ctm";

/// The frozen TV-009 facts, named once (`SPIKE_REPORT.md:9-12`,
/// `docs/27_SPIKE_FINDINGS_REV2.md:24-27`, `continuum-kernel-core/src/fixture.rs:11-14`).
const FROZEN_STATES: u64 = 16;
const FROZEN_TRANSITIONS: u64 = 96;
const FROZEN_WITNESS_DEPTH: usize = 6;

// --- fixtures --------------------------------------------------------------------------

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 2)
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

fn hello_at(low: ProtocolVersion, high: ProtocolVersion) -> ClientHello {
    ClientHello {
        protocol_versions: VersionRange { low, high },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-pr8-exit-evidence".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    }
}

fn implemented() -> Vec<ProtocolVersion> {
    vec![
        ProtocolVersion::new(2, 0),
        ProtocolVersion::new(3, 0),
        ProtocolVersion::new(3, 1),
        version(),
    ]
}

fn grant(handle: &str, actor: &str, level: AuthorityLevel, depth: u32) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(handle),
        actor: who(actor),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: continuumd::protocol::spec::Nullable::Null,
        delegation_depth: depth,
        profile: Optional::Absent,
        instances: Optional::Absent,
    }
}

fn root_grant() -> CapabilityDescriptor {
    CapabilityDescriptor {
        profile: Optional::Present(CapabilityProfile {
            privileged_operations: vec![name("intent.accept")],
            denied_operations: Vec::new(),
            data_grants: Vec::new(),
            cross_principal_sharing: true,
        }),
        ..grant("cap_root", "service:continuumd", AuthorityLevel::Promote, 4)
    }
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

/// A daemon with all four families registered, built against a `negotiated` value the
/// caller already has — from a real handshake, in every fixture this file builds.
fn daemon(negotiated: Negotiated) -> Daemon {
    let root = Some(cap("cap_root"));
    Daemon::builder(Blake3Identity, negotiated, cap("cap_root"))
        .epochs(epochs())
        .now(Timestamp::new("2026-08-01T00:00:00.000Z").expect("a timestamp"))
        .capability(root_grant(), None)
        .capability(
            grant("cap_builder", "agent:builder", AuthorityLevel::Propose, 3),
            root.clone(),
        )
        .capability(
            grant("cap_runner", "agent:runner", AuthorityLevel::Execute, 3),
            root.clone(),
        )
        .capability(
            grant("cap_reader", "agent:reader", AuthorityLevel::Read, 3),
            root,
        )
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(TaskFamily)
        .family(VerificationFamily)
        .build()
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

fn budgeted(mut envelope: RequestEnvelope, states: u64) -> RequestEnvelope {
    envelope.budget = Optional::Present(Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(states),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    });
    envelope
}

fn acceptance_bytes() -> Opaque {
    use continuum_intent::canonical_json::Json;
    let mut fields: std::collections::BTreeMap<String, Json> = std::collections::BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "service:continuumd"),
        ("capability", "revise-intent"),
        ("signature", "sig-die-hard-v1"),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", "2026-08-01T00:00:00.000Z"),
    ] {
        fields.insert(key.to_owned(), Json::String(value.to_owned()));
    }
    Opaque::from_bytes(Json::Object(fields).to_canonical_bytes())
}

// --- the wire harness --------------------------------------------------------------------

/// A connected client/server pair, plus everything a request needs to name the Die Hard
/// corpus: the two frames the handshake actually produced, and the staged content
/// identities `SnapshotComponents` carries.
struct Wire {
    server: Server,
    pair: LocalPair,
    intent: IntentHandle,
    files: Vec<Commitment>,
    paths: Vec<FileComponent>,
    configuration: Commitment,
    /// The first frame of the connection, captured so the determinism test can compare it
    /// too.
    hello_frame: Vec<u8>,
    /// The second frame of the connection: the welcome this exact negotiation produced.
    welcome_frame: Vec<u8>,
}

impl Wire {
    /// Send a request as bytes and read the answer back as bytes — `transport_local.rs`'s
    /// own `Wire::exchange`, duplicated locally.
    fn exchange(
        &mut self,
        envelope: &RequestEnvelope,
        arguments: &Arguments,
    ) -> (ResultEnvelope, Payload, Vec<u8>) {
        client_send(&mut self.pair, envelope, arguments).expect("the request frames");
        let served = self
            .server
            .serve(&mut self.pair)
            .expect("the server answers");
        assert_eq!(served, 1, "one request, one result");
        let mut peek = LocalPair::new();
        let frame = {
            let taken = self
                .pair
                .to_client
                .take_frame()
                .expect("a whole frame")
                .expect("a frame is waiting");
            peek.to_client.put_frame(&taken).expect("re-framing");
            taken
        };
        let (result, payload) = client_receive(&mut peek, envelope.operation.as_str())
            .expect("the result decodes")
            .expect("a frame is waiting");
        (result, payload, frame)
    }
}

/// Build a fresh daemon: stage the Die Hard corpus and register its model out of band,
/// accept the governing intent (a precondition, not a frame — see the module doc), perform
/// the handshake as two real frames, and wrap the result in a [`Server`].
fn wire() -> Wire {
    // The handshake happens first, deliberately: everything that follows is built *from*
    // the negotiated value it produced, not from a second, separately computed one.
    let hello = hello_at(version(), version());
    let hello_frame = encode_hello(&hello).expect("the hello frames");
    let decoded_hello: ClientHello = from_bytes(&hello_frame).expect("the hello decodes");
    assert_eq!(
        decoded_hello, hello,
        "the hello crosses the boundary unchanged"
    );
    let negotiated = negotiate(&implemented(), ProtocolWindow::new(3), ENCODINGS, &hello)
        .expect("3.2 is served");

    let mut daemon = daemon(negotiated);

    // Out-of-band administration: staging and model registration are deliberately not
    // operations (IDL §7, `Server::daemon_mut`'s own doc), so they are deliberately not
    // frames — see "What does not cross a frame, and why" above.
    let contract = IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes())
        .expect("the fixture decodes");
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        continuum_workspace::artifact_path::ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    let intent = continuumd::daemon::identity::intent_to_wire(&stored).expect("an `in_` handle");
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
    let mut paths = Vec::new();
    for (path, content) in [(MODULE_PATH, DIE_HARD_MODEL), ("README.md", "# TV-009\n")] {
        let commitment = daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new(path).expect("a workspace path"),
                content.as_bytes().to_vec(),
            )
            .expect("staging names its content");
        paths.push(FileComponent {
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
            DIE_HARD_CONFIG.as_bytes().to_vec(),
        )
        .expect("staging names its content");
    daemon.state_mut().models_mut().register(
        model_source(&Blake3Identity, [(MODULE_PATH, DIE_HARD_MODEL.as_bytes())])
            .expect("blake3 names the module set"),
        diehard::model().expect("the port builds"),
    );

    // The one genuine wire *operation* still called directly rather than as a frame: a
    // precondition of `workspace.create`, not a fact the exit sentence is about — the same
    // choice `transport_local.rs`'s own `wire()` makes.
    let accepted = daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "intent.accept",
                "service:continuumd",
                "cap_root",
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

    let welcome = welcome_for(negotiated);
    let welcome_frame = Server::open(&welcome, None, Ok(negotiated))
        .expect("the welcome encodes")
        .expect("a welcome frame is sent");
    let read_welcome: ServerWelcome = from_bytes(&welcome_frame).expect("the welcome decodes");
    assert_eq!(
        read_welcome, welcome,
        "the welcome crosses the boundary unchanged"
    );

    Wire {
        server: Server::new(daemon, negotiated),
        pair: LocalPair::new(),
        intent,
        files,
        paths,
        configuration,
        hello_frame,
        welcome_frame,
    }
}

fn components(wire: &Wire) -> SnapshotComponents {
    SnapshotComponents {
        files: wire.files.clone(),
        cml_modules: Vec::new(),
        rust_extraction: Vec::new(),
        domain_packs: Vec::new(),
        dependencies: Vec::new(),
        epochs: SnapshotEpochs {
            semantic: epoch("semantic-1"),
            proof: epoch("proof-1"),
            toolchain: Optional::Absent,
        },
        intent: wire.intent.clone(),
        correspondence: Vec::new(),
        proof_environment: Vec::new(),
        configuration: vec![wire.configuration.clone()],
        file_components: Optional::Present(wire.paths.clone()),
    }
}

/// `workspace.create`, as a frame. `seal` chooses whether the snapshot is sealed on
/// creation, so a caller can exercise the two-frame `create`-then-`seal` path instead of
/// the one-frame shortcut.
fn create(
    wire: &mut Wire,
    seal: bool,
    request: &str,
    key: &str,
) -> (ResultEnvelope, Payload, Vec<u8>) {
    let arguments = Arguments::WorkspaceCreate(WorkspaceCreateRequest {
        components: components(wire),
        overlay: Optional::Absent,
        seal: Optional::Present(seal),
    });
    let envelope = keyed(
        envelope(
            "workspace.create",
            "service:continuumd",
            "cap_root",
            request,
        ),
        key,
    );
    wire.exchange(&envelope, &arguments)
}

/// `workspace.seal`, as a frame.
fn seal_workspace(
    wire: &mut Wire,
    snapshot: &WorkspaceHandle,
    request: &str,
    key: &str,
) -> (ResultEnvelope, Payload, Vec<u8>) {
    let envelope = keyed(
        envelope("workspace.seal", "agent:builder", "cap_builder", request),
        key,
    );
    let arguments = Arguments::WorkspaceSeal(WorkspaceSealRequest {
        snapshot: snapshot.clone(),
    });
    wire.exchange(&envelope, &arguments)
}

/// `workspace.fork`, as a frame — used only by the unregistered-model negative test, to
/// derive a snapshot whose `.ctm` bytes changed.
fn fork_workspace(
    wire: &mut Wire,
    base: &WorkspaceHandle,
    overlay_path: &str,
    overlay_content: &[u8],
    request: &str,
    key: &str,
) -> (ResultEnvelope, Payload, Vec<u8>) {
    let envelope = keyed(
        envelope("workspace.fork", "agent:builder", "cap_builder", request),
        key,
    );
    let arguments = Arguments::WorkspaceFork(WorkspaceForkRequest {
        base: base.clone(),
        overlay: Optional::Present(vec![FileOverlay {
            path: overlay_path.to_owned(),
            content: overlay_content.to_vec(),
        }]),
        patches: Optional::Absent,
    });
    wire.exchange(&envelope, &arguments)
}

/// `verification.start` over Die Hard, `AllClaims`, as a frame.
fn start(
    wire: &mut Wire,
    snapshot: &WorkspaceHandle,
    request: &str,
    key: &str,
) -> (ResultEnvelope, Payload, Vec<u8>) {
    let mut envelope = budgeted(
        keyed(
            envelope("verification.start", "agent:runner", "cap_runner", request),
            key,
        ),
        64,
    );
    envelope.snapshot = Nullable::Value(snapshot.clone());
    let arguments = Arguments::VerificationStart(VerificationStartRequest {
        target: Target {
            kind: TargetKind::AllClaims,
            id: "DieHard".to_owned(),
        },
        portfolio: Portfolio::Interactive,
        context_policy: Optional::Absent,
        priority_class: Optional::Absent,
    });
    wire.exchange(&envelope, &arguments)
}

/// `task.status`, as a frame.
fn status(wire: &mut Wire, task: &TaskHandle, request: &str) -> (ResultEnvelope, Payload, Vec<u8>) {
    let envelope = envelope("task.status", "agent:reader", "cap_reader", request);
    let arguments = Arguments::TaskStatus(TaskStatusRequest { task: task.clone() });
    wire.exchange(&envelope, &arguments)
}

/// `verification.result`, as a frame.
fn result_of(
    wire: &mut Wire,
    task: &TaskHandle,
    request: &str,
) -> (ResultEnvelope, Payload, Vec<u8>) {
    let envelope = envelope("verification.result", "agent:runner", "cap_runner", request);
    let arguments = Arguments::VerificationResult(VerificationResultRequest { task: task.clone() });
    wire.exchange(&envelope, &arguments)
}

fn snapshot_of(payload: &Payload) -> WorkspaceHandle {
    match payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    }
}

fn task_of(result: &ResultEnvelope) -> TaskHandle {
    result
        .task
        .value()
        .cloned()
        .expect("a task-starting result names its task")
}

fn record_of(payload: Payload) -> TaskRecord {
    match payload {
        Payload::TaskStatus(record) => record,
        other => panic!("expected a task.status payload, got {other:?}"),
    }
}

// --- PR-8-EXIT: the sentence, through the wire, honestly ---------------------------------

/// The headline. Every fact up to and including **16** is read off a frame the client
/// actually decoded; **96** and the depth-**6** witness are then read from [`Daemon::state`]
/// with an explicit note that no frame crosses between that point and the assertions —
/// which is the wire-vs-operation-layer boundary this file's module doc names.
///
/// [`Daemon::state`]: continuumd::daemon::Daemon::state
#[test]
fn positive_die_hard_returns_sixteen_states_through_the_wire_and_the_depth_six_solution_one_layer_down()
 {
    let mut wire = wire();
    assert!(
        !wire.hello_frame.is_empty() && !wire.welcome_frame.is_empty(),
        "the handshake happened as two real frames before this line"
    );

    // link: `workspace.create`, unsealed — a frame.
    let (created, created_payload, _) = create(&mut wire, false, "req_create", "idem-create");
    assert_eq!(created.status, ResultStatus::Ok, "{:?}", created.error);
    match &created_payload {
        Payload::WorkspaceCreate(response) => {
            assert!(!response.sealed, "seal was requested false")
        }
        other => panic!("expected a workspace.create payload, got {other:?}"),
    }
    let snapshot = snapshot_of(&created_payload);

    // link: `workspace.seal` — a second frame, the wire's own two-step path to a sealed
    // lineage.
    let (sealed, sealed_payload, _) = seal_workspace(&mut wire, &snapshot, "req_seal", "idem-seal");
    assert_eq!(sealed.status, ResultStatus::Ok, "{:?}", sealed.error);
    match &sealed_payload {
        Payload::WorkspaceSeal(response) => assert_eq!(response.snapshot, snapshot),
        other => panic!("expected a workspace.seal payload, got {other:?}"),
    }

    // link: `verification.start` — a frame that starts the Die Hard campaign.
    let (started, started_payload, _) = start(&mut wire, &snapshot, "req_start", "idem-start");
    assert_eq!(
        started.status,
        ResultStatus::TaskStarted,
        "{:?}",
        started.error
    );
    let task = task_of(&started);
    match &started_payload {
        Payload::VerificationStart(response) => assert_eq!(response.task.value(), Some(&task)),
        other => panic!("expected a verification.start payload, got {other:?}"),
    }

    // link: `task.status` — the frame the exit sentence's "returns … through the daemon
    // API" cashes out as. Everything up to the honesty boundary below is read from *this*
    // decoded frame alone.
    let (status_result, status_payload, _) = status(&mut wire, &task, "req_status");
    assert_eq!(
        status_result.status,
        ResultStatus::Ok,
        "{:?}",
        status_result.error
    );
    let record = record_of(status_payload);
    assert_eq!(record.task, task);
    assert_eq!(record.status, TaskStatus::Completed);
    assert_eq!(record.operation.as_str(), "verification.start");
    assert_eq!(record.priority_class, PriorityClass::Interactive);
    assert!(
        record.continuation.is_absent(),
        "a closed campaign parks nothing"
    );
    assert!(record.failed_reason.is_absent());
    assert_eq!(record.epochs, epochs());

    // --- the wire's own honest half: 16, and 16 alone --------------------------------
    //
    // `Cost` is destructured exhaustively, with no `..`: if a future wire revision ever
    // added an eleventh, `transitions`-shaped field, this line would fail to *compile*,
    // not merely fail an assertion — F16 held as a fact about this file's own shape.
    let Cost {
        wall_ms: _,
        cpu_ms: _,
        memory_bytes: _,
        states,
        solver_ms: _,
        proof_ms: _,
        tokens: _,
        candidates: _,
        bytes: _,
        tokenizer_id: _,
    } = record.cost;
    assert_eq!(
        states,
        Optional::Present(FROZEN_STATES),
        "the frozen reachable-state count, decoded from the actual frame the client received"
    );

    // `verification.result`, one more frame: the wire's own declared home for the witness
    // — `VerificationResult.crashpack` — really is absent on this response, not merely
    // assumed absent.
    let (result_envelope, result_payload, _) = result_of(&mut wire, &task, "req_result");
    assert_eq!(
        result_envelope.status,
        ResultStatus::Ok,
        "{:?}",
        result_envelope.error
    );
    match result_payload {
        Payload::VerificationResult(response) => {
            assert_eq!(response.task, task);
            assert!(
                response.crashpack.is_absent(),
                "no crashpack producer exists in this workspace (task.rs's own module doc)"
            );
        }
        other => panic!("expected a verification.result payload, got {other:?}"),
    }

    // --- one layer down: 96 transitions and the depth-6 witness, corroborated ---------
    //
    // No frame crosses between this comment and the assertions below: `Daemon::state` is
    // a plain Rust reference into the same process this `Server` already ran in, never a
    // decoded response. That is the honest boundary "through the daemon API" draws today
    // — RFC 0026 F16 is why 96 can never become a wire field, and the missing crashpack
    // producer is why depth-6 is not one *yet*. Both numbers are corroborated already, at
    // the operation layer, by `daemon_task_operations.rs`'s
    // `die_hard_returns_sixteen_states_and_a_depth_six_solution_through_the_daemon_api` —
    // the same daemon shape, run through `dispatch` rather than through frames, landing on
    // the same two numbers.
    let campaign = wire
        .server
        .daemon()
        .state()
        .tasks()
        .get(&task)
        .expect("the daemon holds the task")
        .campaign
        .as_ref()
        .expect("the campaign ran");
    assert!(campaign.is_closed(), "64 states admits Die Hard's 16");
    assert_eq!(campaign.states() as u64, FROZEN_STATES);
    assert_eq!(
        campaign.transitions, FROZEN_TRANSITIONS,
        "the 96 no wire field carries — RFC 0026 F16"
    );
    assert_eq!(
        campaign.violation_depth(),
        Some(FROZEN_WITNESS_DEPTH),
        "the depth-6 solution, read off daemon state because no crashpack producer exists \
         to put it on the wire"
    );
}

// --- PR-8-EXIT: determinism, end to end ---------------------------------------------------

/// One scripted exchange, run to completion, returning every frame the server produced.
fn run_end_to_end() -> Vec<Vec<u8>> {
    let mut wire = wire();
    let mut transcript = vec![wire.hello_frame.clone(), wire.welcome_frame.clone()];

    let (_, created_payload, create_frame) = create(&mut wire, false, "req_create", "idem-create");
    transcript.push(create_frame);
    let snapshot = snapshot_of(&created_payload);

    let (_, _, seal_frame) = seal_workspace(&mut wire, &snapshot, "req_seal", "idem-seal");
    transcript.push(seal_frame);

    let (started, _, start_frame) = start(&mut wire, &snapshot, "req_start", "idem-start");
    transcript.push(start_frame);
    let task = task_of(&started);

    let (_, _, status_frame) = status(&mut wire, &task, "req_status");
    transcript.push(status_frame);

    let (_, _, result_frame) = result_of(&mut wire, &task, "req_result");
    transcript.push(result_frame);

    transcript
}

/// **Determinism.** Two independently built daemons, driven through the identical
/// scripted exchange, produce byte-identical transcripts — every frame, in order. Nothing
/// is shared between the two runs but the script itself, so this is a stronger claim than
/// replaying one request against one already-built daemon
/// (`transport_local.rs`'s `replaying_an_idempotent_request_through_the_transport_returns_a_byte_identical_frame`):
/// no daemon-instance-specific state can be leaking into the wire either.
#[test]
fn positive_the_whole_frame_sequence_replayed_from_a_fresh_daemon_is_byte_identical() {
    let first = run_end_to_end();
    let second = run_end_to_end();
    assert!(
        first.len() >= 6,
        "the transcript is not vacuous: {}",
        first.len()
    );
    assert_eq!(first.len(), second.len());
    for (index, (one, two)) in first.iter().zip(&second).enumerate() {
        assert_eq!(
            one, two,
            "frame {index} of two independent runs of the same script differed"
        );
    }
}

// --- PR-8-EXIT: negative, a tampered frame ------------------------------------------------

/// **Negative.** The exact bytes of a genuine, dispatchable `verification.start` request
/// are captured — and proved genuine, sent unmutated first — then mutated one byte at a
/// time. Neither mutation is silently dispatched: one fails to decode at all, the other
/// decodes to a shape this operation does not declare and is answered with a typed
/// refusal. Neither grows the task table beyond the one real task the unmutated frame
/// started.
#[test]
fn negative_a_single_byte_mutation_of_the_verification_start_frame_is_rejected_not_dispatched() {
    let mut wire = wire();
    let (created, created_payload, _) = create(&mut wire, true, "req_create", "idem-create");
    assert_eq!(created.status, ResultStatus::Ok, "{:?}", created.error);
    let snapshot = snapshot_of(&created_payload);

    let mut start_envelope = budgeted(
        keyed(
            envelope(
                "verification.start",
                "agent:runner",
                "cap_runner",
                "req_tamper",
            ),
            "idem-tamper",
        ),
        64,
    );
    start_envelope.snapshot = Nullable::Value(snapshot);
    let arguments = Arguments::VerificationStart(VerificationStartRequest {
        target: Target {
            kind: TargetKind::AllClaims,
            id: "DieHard".to_owned(),
        },
        portfolio: Portfolio::Interactive,
        context_policy: Optional::Absent,
        priority_class: Optional::Absent,
    });
    let good = encode_request(&start_envelope, &arguments).expect("the request frames");

    // Prove the frame is genuine before tampering with it: sent as-is, it starts the
    // campaign, exactly as `start()` does elsewhere in this file.
    let genuine = wire
        .server
        .answer(&good)
        .expect("the untampered frame is answered");
    let genuine_result: ResultEnvelope = from_bytes(&genuine).expect("the answer decodes");
    assert_eq!(
        genuine_result.status,
        ResultStatus::TaskStarted,
        "{:?}",
        genuine_result.error
    );
    let baseline_tasks = wire.server.daemon().state().tasks().handles().len();
    assert_eq!(
        baseline_tasks, 1,
        "the genuine frame minted exactly one task"
    );

    // --- mutation A: the frame's very first byte, a structural `{` ---------------------
    assert_eq!(
        good[0], b'{',
        "the message is a JSON object, by construction"
    );
    let mut mutant_a = good.clone();
    mutant_a[0] = b'[';
    assert!(
        wire.server.answer(&mutant_a).is_err(),
        "a frame whose outer shape does not parse has no request identity to answer with \
         (the transport module's own reasoning) and is rejected outright"
    );
    assert_eq!(
        wire.server.daemon().state().tasks().handles().len(),
        baseline_tasks,
        "no phantom task from a frame that was never dispatched"
    );

    // --- mutation B: one byte inside the embedded arguments, the field name `"target"` --
    // `rule encoding.opaque_payloads` splices the request body directly into the envelope
    // rather than escaping it into a string, so its field names are literal text in this
    // frame (`codec_canonical_form.rs`'s
    // `an_opaque_payload_is_the_operations_own_struct_and_is_carried_verbatim`).
    let text = String::from_utf8(good.clone()).expect("UTF-8");
    let marker = "\"target\"";
    assert_eq!(
        text.matches(marker).count(),
        1,
        "exactly one field is named `target` in this request"
    );
    let offset = text
        .find(marker)
        .expect("the field name is literal text in the frame");
    // The last `t` of `"target"`: quote(0) t(1) a(2) r(3) g(4) e(5) t(6) quote(7).
    let flip_at = offset + 6;
    assert_eq!(good[flip_at], b't', "the byte this test means to flip");
    let mut mutant_b = good.clone();
    mutant_b[flip_at] = b'X';

    let answer = wire
        .server
        .answer(&mutant_b)
        .expect("the outer envelope still decodes; only the field name changed");
    let result: ResultEnvelope = from_bytes(&answer).expect("the answer decodes");
    assert_eq!(result.status, ResultStatus::Error);
    assert_eq!(
        result.error.value().map(|error| error.code),
        Some(ErrorCode::MalformedRequest),
        "a required field silently renamed by one byte is a shape this operation no longer \
         declares"
    );
    assert!(result.payload.is_null());
    assert_eq!(
        result.request_id.as_str(),
        "req_tamper",
        "a caller whose message this daemon cannot read is still owed an answer \
         (the transport module's own comment)"
    );
    assert_eq!(
        wire.server.daemon().state().tasks().handles().len(),
        baseline_tasks,
        "the tampered request never reached the family handler that mints a task"
    );
}

// --- PR-8-EXIT: negative, an unregistered model -------------------------------------------

/// **Negative.** A snapshot whose Die Hard corpus file was forked to different bytes — and
/// sealed — through the wire; nothing is registered for the model that content identifies,
/// so `verification.start` answers `UnsupportedSemanticFeature` rather than a degraded or
/// guessed campaign, and mints no task.
#[test]
fn negative_a_request_against_an_unregistered_model_is_unsupported_semantic_feature_through_the_wire()
 {
    let mut wire = wire();
    let (created, created_payload, _) = create(&mut wire, true, "req_create", "idem-create");
    assert_eq!(created.status, ResultStatus::Ok, "{:?}", created.error);
    let base = snapshot_of(&created_payload);

    // Fork the model file itself, through a frame: a changed model is a different model
    // (`verification.rs`'s own module doc), and nothing is registered for it.
    let (forked, forked_payload, _) = fork_workspace(
        &mut wire,
        &base,
        MODULE_PATH,
        b"// not the corpus port\n",
        "req_fork",
        "idem-fork",
    );
    assert_eq!(forked.status, ResultStatus::Ok, "{:?}", forked.error);
    let derived = match forked_payload {
        Payload::WorkspaceFork(response) => response.snapshot,
        other => panic!("expected a workspace.fork payload, got {other:?}"),
    };

    let (sealed, _, _) = seal_workspace(&mut wire, &derived, "req_seal", "idem-seal");
    assert_eq!(sealed.status, ResultStatus::Ok, "{:?}", sealed.error);

    let (refused, refused_payload, _) = start(&mut wire, &derived, "req_start", "idem-start");
    assert_eq!(refused.status, ResultStatus::Error);
    assert_eq!(
        refused.error.value().map(|error| error.code),
        Some(ErrorCode::UnsupportedSemanticFeature)
    );
    assert_eq!(
        refused_payload,
        Payload::None,
        "no partial effect: no task is minted for a model this daemon cannot construct"
    );
    assert!(
        wire.server.daemon().state().tasks().handles().is_empty(),
        "the refusal never reached the family handler that would mint a task"
    );
}
