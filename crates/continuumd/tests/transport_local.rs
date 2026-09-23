//! The local transport, end to end: bytes in, dispatch, bytes out.
//!
//! # What these tests are evidence for
//!
//! Every exchange below crosses a real byte boundary. The client builds a frame, the
//! server reads a `Vec<u8>` it did not construct, dispatches it, and writes a `Vec<u8>`
//! the client then decodes. Nothing is handed across as a typed value, which is what
//! separates this file from `daemon_operations.rs` and `daemon_task_operations.rs`: those
//! exercise `Daemon::dispatch` over typed arguments, and this one exercises the whole
//! path the START_HERE PR-5 bullets ask for — "request/response types **and local
//! transport**".
//!
//! **The PR-5 exit, at the transport.** "Replaying an idempotent request returns the same
//! task/artifact identity" is asserted here in its strongest available form: the two
//! result *frames* are byte-identical, so the identity is the same and so is everything
//! else the caller can observe. It is asserted for both identity kinds — a `ws_` artifact
//! from `workspace.create` and a `task_` handle from `verification.start` — because the
//! exit names both.
//!
//! The Die Hard fixture is the same one the operation-layer suites use, and for the same
//! reason they give: there is one Die Hard model and one Die Hard contract in this
//! repository, and both arrive here through `include_str!` rather than as a copy.

use continuum_engine_reference::diehard;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::codec::{from_bytes, to_bytes};
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationRequest};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope, ResultEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, ServerReject, ServerWelcome,
    VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::verification::VerificationStartRequest;
use continuumd::protocol::operations::workspace::WorkspaceCreateRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, EpochIdentity, IntentHandle, Opaque, OperationName,
    ProtocolVersion, RequestId, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{FileComponent, SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, Encoding, ErrorCode, Portfolio, ResultStatus, TargetKind,
};
use continuumd::transport::{
    FrameError, LocalPair, MAX_FRAME_BYTES, Server, client_receive, client_send, encode_hello,
};

const DIE_HARD_MODEL: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");
const DIE_HARD_CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");
const MODULE_PATH: &str = "DieHard.ctm";

// --- fixtures -----------------------------------------------------------------------

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
        client: "continuumd-transport-test".to_owned(),
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

fn negotiated() -> Negotiated {
    negotiate(
        &implemented(),
        ProtocolWindow::new(3),
        ENCODINGS,
        &hello_at(version(), version()),
    )
    .expect("3.2 is served")
}

fn grant(handle: &str, actor: &str, level: AuthorityLevel, depth: u32) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(handle),
        actor: who(actor),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        // A delegated capability's depth is strictly less than its parent's (RFC 0027 D6),
        // so the root is 4 and every child is 3. A child at the parent's depth is denied,
        // and the denial is byte-identical with every other — which is exactly why it is
        // worth stating here rather than discovering it as a mystery.
        delegation_depth: depth,
        profile: Optional::Absent,
        instances: Optional::Absent,
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

fn daemon() -> Daemon {
    let root = Some(cap("cap_root"));
    Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .epochs(epochs())
        .now(Timestamp::new("2026-08-01T00:00:00.000Z").expect("a timestamp"))
        .capability(root_grant(), None)
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
        // Filled in by the transport from the typed arguments beside it
        // (`rule encoding.opaque_payloads`).
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

struct Wire {
    server: Server,
    pair: LocalPair,
    intent: IntentHandle,
    files: Vec<Commitment>,
    paths: Vec<FileComponent>,
    configuration: Commitment,
}

impl Wire {
    /// Send a request as bytes and read the answer back as bytes.
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
        // The raw frame, kept so a replay can be compared byte for byte.
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

fn wire() -> Wire {
    let mut daemon = daemon();
    let contract =
        IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes()).expect("the fixture");
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

    // Accepting the intent is a precondition of `workspace.create`, and it goes through
    // `dispatch` rather than the wire because it is not what these tests are about.
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

    Wire {
        server: Server::new(daemon, negotiated()),
        pair: LocalPair::new(),
        intent,
        files,
        paths,
        configuration,
    }
}

fn acceptance_bytes() -> Opaque {
    // The acceptance record the intent family validates; its shape is that family's and is
    // exercised in `daemon_operations.rs`. Here it is a precondition, not a subject.
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
        // The 3.2 addition, exercised across the wire: the request declares where each
        // file goes rather than leaving it to be recovered from daemon-held state.
        file_components: Optional::Present(wire.paths.clone()),
    }
}

fn create(wire: &mut Wire, request: &str, key: &str) -> (ResultEnvelope, Payload, Vec<u8>) {
    let arguments = Arguments::WorkspaceCreate(WorkspaceCreateRequest {
        components: components(wire),
        overlay: Optional::Absent,
        seal: Optional::Present(true),
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

fn snapshot_of(payload: &Payload) -> WorkspaceHandle {
    match payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("workspace.create answers with its own body: {other:?}"),
    }
}

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

// --- the handshake, as frames --------------------------------------------------------

#[test]
fn the_handshake_is_two_frames_and_the_second_is_a_welcome_or_a_reject() {
    // "The daemon's second frame is exactly one of `ServerWelcome` or `ServerReject`,
    // never both and never neither" (RFC 0026, "Connection lifecycle").
    let hello = hello_at(ProtocolVersion::new(3, 0), version());
    let frame = encode_hello(&hello).expect("the hello encodes");
    let decoded: ClientHello = from_bytes(&frame).expect("the hello decodes");
    assert_eq!(decoded, hello, "the hello crosses the boundary unchanged");

    let outcome = negotiate(&implemented(), ProtocolWindow::new(3), ENCODINGS, &hello);
    let settled = outcome.expect("3.2 is common");
    assert_eq!(settled.protocol_version(), version());

    let welcome = ServerWelcome {
        protocol_version: settled.protocol_version(),
        encoding: settled.encoding(),
        majors_served: vec![3, 2],
        server: "continuumd/0".to_owned(),
        grant: root_grant(),
        limits: continuumd::protocol::handshake::ServerLimits {
            idempotency_retention_ms: continuumd::protocol::scalar::DurationMs::new(86_400_000),
            max_page_size: 100,
            max_result_bytes: continuumd::protocol::scalar::ByteCount::new(1_048_576),
            max_concurrent_tasks: 4,
        },
        features: Vec::new(),
        epochs: epochs(),
        pending_advances: Vec::new(),
    };
    let frame = Server::open(&welcome, None, Ok(settled))
        .expect("the welcome encodes")
        .expect("a welcome frame is sent");
    let read: ServerWelcome = from_bytes(&frame).expect("the welcome decodes");
    assert_eq!(read, welcome);
    // The two frames never validate as each other: `ServerWelcome` requires
    // `protocol_version`, `ServerReject` requires `code`.
    assert!(from_bytes::<ServerReject>(&frame).is_err());
}

#[test]
fn the_bootstrap_frames_are_canonical_json_however_the_connection_negotiates() {
    // `rule handshake.bootstrap_encoding` (IDL 1.5, bn-1h158): `ClientHello`,
    // `ServerWelcome`, and `ServerReject` are `canonical_json` unconditionally,
    // independent of `ClientHello.encodings` and of what negotiation selects. This is
    // the pin bn-1mhcr's canonical_cbor delivery left owed — the codec's own golden set
    // (`tests/codec_canonical_cbor.rs`) shows `ClientHello` and `ServerReject` *can* be
    // spelled in canonical_cbor; this test is evidence for the narrower and different
    // claim that the transport never does, even when the offer prefers CBOR and even
    // once CBOR is what gets negotiated.
    use continuumd::codec::cbor::Cbor;
    use continuumd::codec::json::Json;

    // The hello offers `canonical_cbor` first — the strongest case for the rule, since a
    // transport that sniffed the offer's preference would pick CBOR here.
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: ProtocolVersion::new(3, 0),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalCbor, Encoding::CanonicalJson],
        client: "continuumd-transport-test".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };

    let hello_frame = encode_hello(&hello).expect("the hello encodes");
    assert_eq!(
        hello_frame,
        br#"{"actor":"service:continuumd","capability":"cap_root","client":"continuumd-transport-test","encodings":["canonical_cbor","canonical_json"],"protocol_versions":{"high":"3.2","low":"3.0"}}"#,
        "the pinned bootstrap bytes: canonical_json, field order ascending, despite the CBOR-first offer"
    );
    assert!(
        Json::parse(&hello_frame).is_ok(),
        "the hello frame is canonical JSON"
    );
    assert!(
        Cbor::parse(&hello_frame).is_err(),
        "the hello frame is not canonical CBOR, though `ClientHello` offers it first"
    );

    // Negotiate CBOR — the encoding the hello prefers — so the welcome and reject below
    // are built against a connection whose *negotiated* encoding is CBOR.
    let outcome = negotiate(&implemented(), ProtocolWindow::new(3), ENCODINGS, &hello);
    let settled = outcome.expect("3.2 is common");
    assert_eq!(
        settled.encoding(),
        Encoding::CanonicalCbor,
        "the hello's first preference is what negotiation selects"
    );

    let welcome = ServerWelcome {
        protocol_version: settled.protocol_version(),
        encoding: settled.encoding(),
        majors_served: vec![3, 2],
        server: "continuumd/0".to_owned(),
        grant: root_grant(),
        limits: continuumd::protocol::handshake::ServerLimits {
            idempotency_retention_ms: continuumd::protocol::scalar::DurationMs::new(86_400_000),
            max_page_size: 100,
            max_result_bytes: continuumd::protocol::scalar::ByteCount::new(1_048_576),
            max_concurrent_tasks: 4,
        },
        features: Vec::new(),
        epochs: epochs(),
        pending_advances: Vec::new(),
    };
    let welcome_frame = Server::open(&welcome, None, Ok(settled))
        .expect("the welcome encodes")
        .expect("a welcome frame is sent");
    assert!(
        Json::parse(&welcome_frame).is_ok(),
        "`ServerWelcome` is canonical JSON even though the connection just negotiated CBOR"
    );
    assert!(
        Cbor::parse(&welcome_frame).is_err(),
        "`ServerWelcome` is not spelled in the encoding it announces"
    );
    let read_welcome: ServerWelcome = from_bytes(&welcome_frame).expect("the welcome decodes");
    assert_eq!(read_welcome, welcome);
    assert_eq!(
        read_welcome.encoding,
        Encoding::CanonicalCbor,
        "the frame's own bytes are JSON; the value it carries names CBOR as what comes next"
    );

    // A reject frame answers the same way: the connection has nothing negotiated yet, so
    // there is no encoding to have inherited, but a version-outside-the-window rejection
    // still cannot ride the CBOR the client offered.
    let narrow_hello = ClientHello {
        protocol_versions: VersionRange {
            low: ProtocolVersion::new(1, 0),
            high: version(),
        },
        ..hello.clone()
    };
    let mut narrow_implemented = implemented();
    narrow_implemented.retain(|entry| entry.major() == 9);
    let error = negotiate(
        &narrow_implemented,
        ProtocolWindow::new(3),
        ENCODINGS,
        &narrow_hello,
    )
    .expect_err("nothing is common");
    let reject = ServerReject::for_negotiation(error, &narrow_hello, &[3, 2], &error.to_string())
        .expect("the offer reaches 3.1, so the frame is sent");
    let reject_frame = Server::open(&welcome, Some(&reject), Err(error))
        .expect("the reject encodes")
        .expect("a reject frame is sent");
    assert!(
        Json::parse(&reject_frame).is_ok(),
        "`ServerReject` is canonical JSON too, sent before anything is negotiated"
    );
    assert!(
        Cbor::parse(&reject_frame).is_err(),
        "`ServerReject` is not spelled in canonical CBOR, though the offer preferred it"
    );

    // The other half of the rule — the negotiated encoding governs from the first
    // post-negotiation frame onward — is pinned at the transport already, in CBOR
    // specifically: `a_cbor_connection_serves_a_whole_exchange_in_cbor`
    // (`tests/codec_canonical_cbor.rs`) drives a real `evidence.link` exchange over a
    // CBOR-negotiated connection and asserts both the request and the result frame are
    // CBOR maps that do not parse as canonical JSON. That test and this one are the same
    // claim from opposite ends of one connection.
}

#[test]
fn a_refused_connection_is_a_typed_frame_before_anything_is_negotiated() {
    // RFC 0026 correction 38 / `rule handshake.rejection`. The client offers only a major
    // this daemon no longer serves, so there is no negotiated version — and the refusal is
    // still a frame the client can decode rather than a closed socket.
    let hello = hello_at(ProtocolVersion::new(1, 0), ProtocolVersion::new(1, 9));
    let error = negotiate(&implemented(), ProtocolWindow::new(3), ENCODINGS, &hello)
        .expect_err("major 1 is outside the window");
    let reject = ServerReject::for_negotiation(error, &hello, &[3, 2], &error.to_string());
    assert!(
        reject.is_none(),
        "a client whose offer does not reach 3.1 cannot parse the frame, so none is sent"
    );

    // A client that *can* parse it gets it, as bytes.
    let hello = hello_at(ProtocolVersion::new(1, 0), version());
    let mut narrow = implemented();
    narrow.retain(|entry| entry.major() == 9);
    let error = negotiate(&narrow, ProtocolWindow::new(3), ENCODINGS, &hello)
        .expect_err("nothing is common");
    let reject = ServerReject::for_negotiation(error, &hello, &[3, 2], &error.to_string())
        .expect("the offer reaches 3.1, so the frame is sent");
    let frame = Server::open(&placeholder_welcome(), Some(&reject), Err(error))
        .expect("the reject encodes")
        .expect("a reject frame is sent");
    let read: ServerReject = from_bytes(&frame).expect("the reject decodes");
    assert_eq!(read.code, ErrorCode::ProtocolVersionUnsupported);
    assert!(!read.retryable, "`rule handshake.rejection` fixes it false");
    assert_eq!(read.majors_served, vec![3, 2]);
}

fn placeholder_welcome() -> ServerWelcome {
    ServerWelcome {
        protocol_version: version(),
        encoding: Encoding::CanonicalJson,
        majors_served: vec![3, 2],
        server: "continuumd/0".to_owned(),
        grant: root_grant(),
        limits: continuumd::protocol::handshake::ServerLimits {
            idempotency_retention_ms: continuumd::protocol::scalar::DurationMs::new(86_400_000),
            max_page_size: 100,
            max_result_bytes: continuumd::protocol::scalar::ByteCount::new(1_048_576),
            max_concurrent_tasks: 4,
        },
        features: Vec::new(),
        epochs: epochs(),
        pending_advances: Vec::new(),
    }
}

// --- framing --------------------------------------------------------------------------

#[test]
fn a_frame_is_read_only_once_all_of_it_has_arrived() {
    // The reader does not assume a frame arrived whole, which is what a stream transport
    // needs. Delivering one byte at a time is the strongest form of that test.
    let mut pair = LocalPair::new();
    let payload = to_bytes(&hello_at(version(), version())).expect("encodes");
    let mut framed = Vec::new();
    framed.extend((payload.len() as u32).to_be_bytes());
    framed.extend_from_slice(&payload);

    for (index, byte) in framed.iter().enumerate() {
        pair.to_server.put_bytes(&[*byte]);
        let taken = pair.to_server.take_frame().expect("no framing error");
        if index + 1 < framed.len() {
            assert!(taken.is_none(), "a partial frame is not a frame");
        } else {
            assert_eq!(taken.expect("the last byte completes it"), payload);
        }
    }
    assert!(pair.to_server.is_empty(), "nothing is left over");
}

// T01 (parser/decoder memory exhaustion): the module doc states the reason for
// `MAX_FRAME_BYTES` in one sentence — "a length prefix cannot ask an endpoint for an
// allocation before any of the payload has arrived". The three tests below are that
// sentence exercised from outside the crate: a four-byte prefix is the entire hostile
// input, never a multi-gigabyte fixture, because the bound must reject the *declared*
// count on its own, before a single payload byte is awaited or buffered.

#[test]
fn a_declared_frame_length_past_the_transport_bound_is_rejected_before_any_wait() {
    // One byte past MAX_FRAME_BYTES, and nothing else: if the reader ever buffered
    // waiting for a declared count instead of checking it first, this is the frame
    // that would ask it to hold slightly more than the transport's own ceiling.
    let mut pair = LocalPair::new();
    let declared = u32::try_from(MAX_FRAME_BYTES).expect("the bound fits a u32") + 1;
    pair.to_server.put_bytes(&declared.to_be_bytes());
    assert_eq!(pair.to_server.take_frame(), Err(FrameError::TooLarge));
    // The rejection does not consume the bytes: an endpoint that cannot read a frame
    // closes the connection rather than resynchronizing (the method's own doc).
    assert!(!pair.to_server.is_empty());
}

#[test]
fn a_maximally_hostile_declared_length_is_rejected_from_four_bytes_alone() {
    // The worst case a four-byte big-endian prefix can spell: a ~4 GiB declared
    // payload with zero bytes behind it. Four bytes in, a typed rejection out — this
    // is the exact shape a memory-exhaustion bug in this transport would take, so the
    // fixture is the attack, not a stand-in for it.
    let mut pair = LocalPair::new();
    pair.to_server.put_bytes(&u32::MAX.to_be_bytes());
    assert_eq!(pair.to_server.take_frame(), Err(FrameError::TooLarge));
}

#[test]
fn a_declared_length_at_the_bound_is_incomplete_not_rejected() {
    // MAX_FRAME_BYTES itself is admissible, so the check is strictly `>`. Declaring
    // exactly the bound with no payload yet must read as "not here yet" (`Ok(None)`),
    // never as `TooLarge` — otherwise the transport would refuse its own largest
    // legal frame.
    let mut pair = LocalPair::new();
    let declared = u32::try_from(MAX_FRAME_BYTES).expect("the bound fits a u32");
    pair.to_server.put_bytes(&declared.to_be_bytes());
    assert_eq!(pair.to_server.take_frame(), Ok(None));
}

#[test]
fn two_requests_in_one_buffer_are_both_served_in_order() {
    let mut wire = wire();
    let (_, first, _) = create(&mut wire, "req_create", "idem-create");
    let snapshot = snapshot_of(&first);

    // Two frames written before the server reads either.
    let mut envelope = budgeted(
        keyed(
            envelope("verification.start", "agent:runner", "cap_runner", "req_a"),
            "idem-a",
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
    client_send(&mut wire.pair, &envelope, &arguments).expect("frames");
    let mut second = envelope.clone();
    second.request_id = RequestId::new("req_b").expect("a request id");
    second.idempotency_key = Optional::Present("idem-b".to_owned());
    client_send(&mut wire.pair, &second, &arguments).expect("frames");

    assert_eq!(
        wire.server.serve(&mut wire.pair).expect("both are served"),
        2
    );
    for expected in ["req_a", "req_b"] {
        let (result, _) = client_receive(&mut wire.pair, "verification.start")
            .expect("decodes")
            .expect("a frame is waiting");
        assert_eq!(result.request_id.as_str(), expected, "answers are in order");
    }
}

// --- bytes in, dispatch, bytes out ------------------------------------------------------

#[test]
fn a_request_crosses_the_boundary_as_bytes_and_the_payload_comes_back_inside_the_envelope() {
    let mut wire = wire();
    let (result, payload, frame) = create(&mut wire, "req_create", "idem-create");
    assert_eq!(result.status, ResultStatus::Ok, "{:?}", result.error);

    // `ResultEnvelope.payload` is no longer `Opaque`-null at the boundary: it carries the
    // operation's response struct as real bytes, which is what bn-3bhkp's acceptance
    // criterion asks for.
    let carried = match &result.payload {
        Nullable::Value(opaque) => opaque.clone(),
        Nullable::Null => panic!("a successful result carries its response body"),
    };
    assert!(!carried.as_bytes().is_empty());
    let snapshot = snapshot_of(&payload);
    let text = String::from_utf8(frame).expect("UTF-8");
    assert!(
        text.contains(snapshot.as_str()),
        "the handle is in the frame the client actually received"
    );
}

#[test]
fn a_denial_is_a_frame_and_carries_a_null_payload() {
    let mut wire = wire();
    // `cap_reader` is `read`; `workspace.create` declares `propose`.
    let arguments = Arguments::WorkspaceCreate(WorkspaceCreateRequest {
        components: components(&wire),
        overlay: Optional::Absent,
        seal: Optional::Present(true),
    });
    let envelope = keyed(
        envelope(
            "workspace.create",
            "agent:reader",
            "cap_reader",
            "req_denied",
        ),
        "idem-denied",
    );
    let (result, payload, _) = wire.exchange(&envelope, &arguments);
    assert_eq!(result.status, ResultStatus::Error);
    assert_eq!(
        result.error.value().map(|error| error.code),
        Some(ErrorCode::CapabilityDenied)
    );
    assert!(
        result.payload.is_null(),
        "`payload` is null on `status = error`, which is what the IDL declares"
    );
    assert_eq!(payload, Payload::None);
    assert!(
        result.audit.value().is_some(),
        "`rule audit.correlation` requires it on every `CapabilityDenied`"
    );
}

#[test]
fn a_request_body_that_does_not_decode_is_answered_rather_than_dropped() {
    // The codec sits outside the operation layer, so this request never reaches
    // `dispatch` — and a caller that sent a message this daemon cannot read is still owed
    // an answer. `Daemon::refuse` builds it through the same epoch and audit seams.
    let mut wire = wire();
    let mut envelope = keyed(
        envelope(
            "workspace.create",
            "service:continuumd",
            "cap_root",
            "req_malformed",
        ),
        "idem-malformed",
    );
    envelope.arguments = Opaque::from_bytes(br#"{"components":7}"#.to_vec());
    let frame = to_bytes(&envelope).expect("the envelope itself is well formed");
    let answer = wire.server.answer(&frame).expect("an answer is produced");
    let result: ResultEnvelope = from_bytes(&answer).expect("the answer decodes");
    assert_eq!(result.status, ResultStatus::Error);
    assert_eq!(
        result.error.value().map(|error| error.code),
        Some(ErrorCode::MalformedRequest)
    );
    assert_eq!(
        result.request_id.as_str(),
        "req_malformed",
        "the echo holds"
    );
    assert!(result.payload.is_null());
}

// --- the PR-5 exit, at the transport -----------------------------------------------------

#[test]
fn replaying_an_idempotent_request_through_the_transport_returns_a_byte_identical_frame() {
    // The PR-5 exit: "replaying an idempotent request returns the same task/artifact
    // identity". Both identity kinds, and in the strongest form the wire admits — the two
    // result frames are equal byte for byte, so the identity is the same and so is
    // everything else a caller can observe.
    let mut wire = wire();

    // (1) an artifact identity, from `workspace.create`.
    let (_, first, first_frame) = create(&mut wire, "req_create", "idem-create");
    let (_, replayed, replay_frame) = create(&mut wire, "req_create", "idem-create");
    assert_eq!(
        snapshot_of(&first),
        snapshot_of(&replayed),
        "the same artifact identity"
    );
    assert_eq!(first_frame, replay_frame, "byte-identical result frames");

    // (2) a task identity, from `verification.start`.
    let snapshot = snapshot_of(&first);
    let (started, _, started_frame) = start(&mut wire, &snapshot, "req_start", "idem-start");
    assert_eq!(
        started.status,
        ResultStatus::TaskStarted,
        "{:?}",
        started.error
    );
    let task = started
        .task
        .value()
        .cloned()
        .expect("the envelope names the task it started");
    let (again, _, replay_frame) = start(&mut wire, &snapshot, "req_start", "idem-start");
    assert_eq!(
        again.task.value(),
        Some(&task),
        "the same task identity, through the transport"
    );
    assert_eq!(started_frame, replay_frame, "byte-identical result frames");
}

#[test]
fn reusing_a_key_for_a_different_request_is_refused_at_the_transport_too() {
    // The other half of `rule idempotency.replay`. Same key, different canonical request.
    let mut wire = wire();
    let (_, created, _) = create(&mut wire, "req_create", "idem-create");
    let snapshot = snapshot_of(&created);
    let (_, _, _) = start(&mut wire, &snapshot, "req_start", "idem-start");

    let mut envelope = budgeted(
        keyed(
            envelope(
                "verification.start",
                "agent:runner",
                "cap_runner",
                "req_start_2",
            ),
            "idem-start",
        ),
        64,
    );
    envelope.snapshot = Nullable::Value(snapshot);
    // Same key, different *arguments*: a different target is a different campaign.
    let arguments = Arguments::VerificationStart(VerificationStartRequest {
        target: Target {
            kind: TargetKind::Property,
            id: continuum_engine_reference::diehard::TYPE_OK.to_owned(),
        },
        portfolio: Portfolio::Interactive,
        context_policy: Optional::Absent,
        priority_class: Optional::Absent,
    });
    let (result, payload, _) = wire.exchange(&envelope, &arguments);
    assert_eq!(
        result.error.value().map(|error| error.code),
        Some(ErrorCode::IdempotencyKeyReused)
    );
    assert_eq!(payload, Payload::None, "no partial effect");
}

#[test]
fn the_declared_file_placement_travels_on_the_wire_and_is_checked_against_the_commitments() {
    // The 3.2 addition (bn-i4aem item 2), exercised through the transport rather than only
    // through `dispatch`: a request declares where each file goes, and a request whose two
    // statements disagree is refused rather than resolved in either list's favour.
    let mut wire = wire();
    let (result, _, _) = create(&mut wire, "req_create", "idem-create");
    assert_eq!(result.status, ResultStatus::Ok, "{:?}", result.error);

    let mut wrong = components(&wire);
    if let Optional::Present(declared) = &mut wrong.file_components {
        declared[0].path = "Elsewhere.ctm".to_owned();
    }
    let envelope = keyed(
        envelope(
            "workspace.create",
            "service:continuumd",
            "cap_root",
            "req_mismatch",
        ),
        "idem-mismatch",
    );
    let (refused, payload, _) = wire.exchange(
        &envelope,
        &Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: wrong,
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    );
    assert_eq!(
        refused.error.value().map(|error| error.code),
        Some(ErrorCode::MalformedRequest)
    );
    assert_eq!(payload, Payload::None, "no partial effect");
}
