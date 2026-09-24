//! C018 on the system path: `evidence.verify` binds a verified finite-closure claim to the
//! model the daemon holds (bn-3hk4v).
//!
//! bn-35y4f gave `continuum-kernel-core` wire epoch 2: the certificate carries the model's
//! canonical encoding, and the kernel re-derives the successor relation from it. That makes
//! a verified claim true *of the carried model*. RFC 0005 correction 1 leaves one step to
//! the caller: compare the carried encoding with the caller's own model, byte for byte. Until
//! bn-3hk4v the daemon did not take that step. It promoted a wire-epoch-1 claim and a
//! wire-epoch-2 claim alike, and it never compared the carried model with the model it
//! holds, so a true certificate about a different model reached `validated`.
//!
//! | Case | Expected | Test |
//! |---|---|---|
//! | epoch-2 claim, carried model = the held model, through the transport | `validated`, `checked-certificate` | [`a_bound_claim_reaches_validated_through_the_wire`] |
//! | a true certificate about a different model | `CertificateRejected`, no data, not promoted | [`a_true_certificate_about_another_model_is_rejected`] |
//! | epoch-1 finite closure and state type (trusted correspondence) | `InsufficientEvidence` | [`a_claim_that_trusts_its_model_correspondence_does_not_reach_validated`] |
//! | no snapshot, unknown snapshot, two snapshots, unregistered model, unsealed snapshot | `InsufficientEvidence` | [`every_missing_held_model_fails_closed`] |
//! | an undecodable carried model | the kernel's own `CertificateRejected` | [`an_undecodable_carried_model_is_the_kernels_rejection`] |
//! | the same model, built twice and filed under two snapshots | `validated` under both | [`the_same_model_built_again_binds`] |
//! | two snapshots with different models | each certificate binds to its own snapshot only | [`two_snapshots_with_different_models_bind_apart`] |
//! | a grant that does not hold the snapshot the node derives from | `CapabilityDenied` whatever the binding says | [`the_derived_snapshot_is_decided_against_the_grant`] |
//! | a receipt node derived by `evidence.link` over a certificate subject | `InsufficientEvidence`: it inherits no binding | [`a_derived_receipt_does_not_inherit_a_binding`] |
//! | a same-actor replay of a `validated` answer under a grant without the snapshot | `CapabilityDenied`, nothing of the answer | [`a_replay_under_a_grant_without_the_snapshot_is_denied`] |
//! | the same replay under the full grant | the recorded answer | [`a_replay_under_the_full_grant_returns_the_recorded_answer`] |
//! | LRAT and SMT refutations (trusted formula / skeleton correspondence) | `InsufficientEvidence` | [`a_refutation_whose_encoding_is_trusted_does_not_reach_validated`] |
//!
//! No wire operation appends a certificate-class node in this protocol version, so nodes are
//! filed through state, as `daemon_evidence.rs` files them. The snapshots are real: created
//! and sealed by `workspace.create`.

#![allow(clippy::too_many_lines)]

#[path = "support/model_binding.rs"]
mod model_binding;

use continuum_certificate::{KernelVerdict, Outcome, continuum_kernel_core};
use continuum_engine_reference::bfs::{self, Bounds};
use continuum_engine_reference::certificate::{self, ClaimEnvelope, ClosedSet, PRODUCER};
use continuum_engine_reference::diehard;
use continuum_engine_reference::expr::{BoolExpr, CmpOp, IntExpr};
use continuum_engine_reference::model::{ActionDecl, Model, ModelBuilder};
use continuum_evidence::claim_status::ClaimStatus;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::evidence::{EvidenceFamily, node_identity};
use continuumd::daemon::family::{Arguments, ErrorData, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::state::{Denial, EvidenceNode, StatusWrite};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest};
use continuumd::protocol::envelope::{Budget, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::evidence::{EvidenceLinkRequest, EvidenceVerifyRequest};
use continuumd::protocol::operations::workspace::WorkspaceCreateRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, EpochIdentity, EvidenceHandle, Opaque, OperationName,
    ProtocolVersion, RequestId, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{SnapshotComponents, SnapshotEpochs};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, DataGrant, Encoding, ErrorCode, EvidenceKind, EvidenceNodeKind, EvidenceStatus,
    ResultStatus,
};
use continuumd::transport::{Server, decode_result, encode_request};

/// The committed C018 corpus: `id | recorded verdict | hex bytes`.
const CORPUS: &str = include_str!("../../continuum-certificate/tests/c018-corpus/cases.txt");

/// The profile every certificate node here is filed under, unless a test needs two nodes
/// over the same bytes.
const PROFILE: &str = "continuum-engine-reference/finite-closure";

// --- fixtures ----------------------------------------------------------------------------

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 1)
}

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("a well-formed capability handle")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("a well-formed actor identity")
}

fn now() -> Timestamp {
    Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp")
}

fn grant(
    handle: &str,
    actor: &str,
    level: AuthorityLevel,
    classes: &[&str],
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(handle),
        actor: who(actor),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: classes.iter().map(|class| (*class).to_owned()).collect(),
        expires_at: Nullable::Null,
        delegation_depth: 3,
        profile: Optional::Present(CapabilityProfile {
            privileged_operations: Vec::new(),
            denied_operations: Vec::new(),
            data_grants: vec![DataGrant::ProductionTrace],
            cross_principal_sharing: false,
        }),
        instances: Optional::Absent,
    }
}

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-c018-model-binding".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.1 is served")
}

fn daemon() -> Daemon {
    let root = Some(cap("cap_root"));
    Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .now(now())
        .capability(
            {
                let mut descriptor = grant(
                    "cap_root",
                    "service:continuumd",
                    AuthorityLevel::Promote,
                    &[],
                );
                descriptor.delegation_depth = 4;
                descriptor
            },
            None,
        )
        // The verifying caller: `read`, every class.
        .capability(
            grant("cap_reader", "agent:reader", AuthorityLevel::Read, &[]),
            root.clone(),
        )
        // `read` over evidence and nothing else: it may name the node, and it does not
        // hold the snapshot class the node derives from.
        .capability(
            grant(
                "cap_evidence_only",
                "agent:narrow",
                AuthorityLevel::Read,
                &["ev"],
            ),
            root.clone(),
        )
        // The verifying caller's own actor under a narrower grant: every class except the
        // snapshot class `ws`. It admits every handle-shaped string the recorded answer
        // carries, and not the snapshot the binding read. A replay is keyed by actor and
        // request, not by capability, so this is the grant a same-actor replay presents
        // (cr-3lrkq3).
        .capability(
            {
                let mut narrow = grant(
                    "cap_reader_narrow",
                    "agent:reader",
                    AuthorityLevel::Read,
                    &[],
                );
                narrow.artifact_classes = ArtifactClass::ALL
                    .into_iter()
                    .filter(|class| *class != ArtifactClass::WorkspaceSnapshot)
                    .map(|class| class.token().to_owned())
                    .collect();
                narrow
            },
            root.clone(),
        )
        // The same actor, a sibling grant that does not cover `cap_reader` (it holds no
        // data grant) and does admit every class, the snapshot class included. A replay
        // under it is decided by the handles, not by `covers`, and it must succeed.
        .capability(
            {
                let mut sibling = grant(
                    "cap_reader_sibling",
                    "agent:reader",
                    AuthorityLevel::Read,
                    &[],
                );
                sibling.profile = Optional::Present(CapabilityProfile {
                    privileged_operations: Vec::new(),
                    denied_operations: Vec::new(),
                    data_grants: Vec::new(),
                    cross_principal_sharing: false,
                });
                sibling
            },
            root.clone(),
        )
        // A checker, for `evidence.link`.
        .capability(
            grant(
                "cap_checker",
                "service:kernel-core",
                AuthorityLevel::Execute,
                &[],
            ),
            root,
        )
        .family(EvidenceFamily::new())
        .family(WorkspaceFamily)
        .build()
}

fn envelope(operation: &str, actor: &str, capability: &str, request: &str) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request).expect("a well-formed request id"),
        idempotency_key: Optional::Absent,
        actor: who(actor),
        capability: cap(capability),
        operation: OperationName::new(operation).expect("a well-formed operation name"),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: Optional::Absent,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

fn create_envelope(key: &str) -> RequestEnvelope {
    let mut create = envelope(
        "workspace.create",
        "service:continuumd",
        "cap_root",
        &format!("req_{key}"),
    );
    create.idempotency_key = Optional::Present(format!("idem-{key}"));
    create
}

/// A `@mutation @task_starting` envelope for `evidence.verify`.
fn verify_envelope(actor: &str, capability: &str, request: &str) -> RequestEnvelope {
    let mut verify = envelope("evidence.verify", actor, capability, request);
    verify.idempotency_key = Optional::Present(format!("idem-{request}"));
    verify.budget = Optional::Present(Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Absent,
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    });
    verify
}

fn verify_as(
    daemon: &mut Daemon,
    actor: &str,
    capability: &str,
    handle: &EvidenceHandle,
    request: &str,
) -> OperationOutcome {
    daemon.dispatch(&OperationRequest {
        envelope: verify_envelope(actor, capability, request),
        arguments: Arguments::EvidenceVerify(EvidenceVerifyRequest {
            evidence: handle.clone(),
            expected_status: Optional::Absent,
        }),
    })
}

fn verify(daemon: &mut Daemon, handle: &EvidenceHandle, request: &str) -> OperationOutcome {
    verify_as(daemon, "agent:reader", "cap_reader", handle, request)
}

fn code(outcome: &OperationOutcome) -> ErrorCode {
    outcome.error_code().expect("an error result")
}

fn status(daemon: &Daemon, handle: &EvidenceHandle) -> ClaimStatus {
    daemon
        .state()
        .evidence(handle)
        .expect("the node is held")
        .status()
}

fn verified_status(outcome: &OperationOutcome) -> EvidenceStatus {
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        outcome.envelope.error
    );
    match &outcome.payload {
        Payload::EvidenceVerify(response) => response.status,
        other => panic!("expected an evidence.verify payload, got {other:?}"),
    }
}

/// Stage `bytes` and file a certificate-class node over them, under the identity it
/// derives, with `inputs` as its derivation.
fn certificate_node(
    daemon: &mut Daemon,
    path: &str,
    profile: &str,
    bytes: Vec<u8>,
    inputs: Vec<String>,
) -> EvidenceHandle {
    let artifact = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new(path).expect("a workspace path"),
            bytes,
        )
        .expect("staging names its content");
    let handle =
        node_identity(daemon.services(), &artifact, profile).expect("the identity seam names it");
    daemon.state_mut().append_evidence(
        handle.clone(),
        EvidenceNode {
            kind: EvidenceNodeKind::Certificate,
            evidence_kind: Some(EvidenceKind::Certificate),
            labels: Vec::new(),
            claim_id: format!("claim:{path}"),
            artifact,
            producer: who("agent:producer"),
            tool: profile.to_owned(),
            created_at: now(),
            inputs,
            idempotency_key: format!("idem-append-{path}"),
            history: vec![StatusWrite {
                status: ClaimStatus::BOTTOM,
                service_identity: None,
                validation_basis: None,
                inconclusive_reason: None,
            }],
            redaction: None,
            publication: None,
        },
    );
    handle
}

/// A node that derives from exactly `snapshot`.
fn bound_node(
    daemon: &mut Daemon,
    path: &str,
    profile: &str,
    bytes: Vec<u8>,
    snapshot: &WorkspaceHandle,
) -> EvidenceHandle {
    certificate_node(
        daemon,
        path,
        profile,
        bytes,
        vec![snapshot.as_str().to_owned()],
    )
}

/// A wire-epoch-2 finite-closure certificate for `model`, from the reference engine.
fn certificate_for(model: &Model) -> Vec<u8> {
    let exploration = bfs::explore(model, Bounds::CERTIFIABLE).expect("the model evaluates");
    let closed = ClosedSet::of(&exploration).expect("the exploration completes");
    certificate::emit_finite_closure(
        model,
        closed,
        &ClaimEnvelope {
            model_digest: "blake3:model-binding",
            semantic_epoch: "continuum-semantics-1",
            property_digest: "blake3:typeok",
            scope_digest: "blake3:scope",
            assumptions_digest: "blake3:empty-assumptions",
            producer: PRODUCER,
            domain_pack_digests: &[],
        },
    )
    .expect("a closed exploration of a declared model emits")
}

/// A model that is not Die Hard: one counter `x ∈ 0..=2`, `inc` while below 2, `reset`.
fn counter_model() -> Model {
    let x = || IntExpr::var("x");
    ModelBuilder::new()
        .variable("x", 0, 2)
        .initial_state(&[("x", 0)])
        .action(ActionDecl::deterministic(
            "inc",
            BoolExpr::compare(CmpOp::Lt, x(), IntExpr::constant(2)),
            vec![("x", IntExpr::plus(x(), IntExpr::constant(1)))],
        ))
        .action(ActionDecl::deterministic(
            "reset",
            BoolExpr::constant(true),
            vec![("x", IntExpr::constant(0))],
        ))
        .build()
        .expect("the counter is a valid model")
}

/// The counter's CML source, as a snapshot carries it. The daemon has no CML front end, so
/// the bytes only name the model source; the registered model is [`counter_model`].
const COUNTER_MODULE: &str = "module Counter\n// x in 0..2, inc while below 2, reset\n";

fn die_hard(daemon: &mut Daemon) -> WorkspaceHandle {
    model_binding::die_hard_snapshot(daemon, create_envelope("seal-die-hard"))
}

fn counter(daemon: &mut Daemon) -> WorkspaceHandle {
    model_binding::sealed_snapshot(
        daemon,
        create_envelope("seal-counter"),
        "Counter.ctm",
        COUNTER_MODULE.as_bytes(),
        Some(counter_model()),
    )
}

/// One corpus certificate, by id.
fn corpus(id: &str) -> Vec<u8> {
    let line = CORPUS
        .lines()
        .find(|line| line.split(" | ").next() == Some(id))
        .unwrap_or_else(|| panic!("the corpus holds {id}"));
    let hex = line.split(" | ").nth(2).expect("a hex column").trim();
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("hex"))
        .collect()
}

/// The kernel's verified claim over `bytes`: its wire epoch and whether it trusts the
/// model correspondence.
fn core_claim(bytes: &[u8]) -> (u16, bool) {
    match continuum_certificate::check_certificate(bytes) {
        Outcome::Checked(KernelVerdict::Core(continuum_kernel_core::Verdict::Verified(claim))) => (
            claim.wire_epoch(),
            claim
                .trusted_components()
                .contains(&"certificate-model-correspondence"),
        ),
        other => panic!("expected a verified core claim, got {other:?}"),
    }
}

// --- the tests ---------------------------------------------------------------------------

#[test]
fn a_bound_claim_reaches_validated_through_the_wire() {
    let mut daemon = daemon();
    let snapshot = die_hard(&mut daemon);
    let bytes = certificate_for(&diehard::model().expect("the port builds"));
    assert_eq!(
        core_claim(&bytes),
        (2, false),
        "a model-bound epoch-2 claim"
    );
    let handle = bound_node(
        &mut daemon,
        "certs/die-hard.cert",
        PROFILE,
        bytes,
        &snapshot,
    );

    // The daemon behind the transport a client talks to.
    let mut server = Server::new(daemon, negotiated());
    let request = encode_request(
        &verify_envelope("agent:reader", "cap_reader", "req_wire"),
        &Arguments::EvidenceVerify(EvidenceVerifyRequest {
            evidence: handle.clone(),
            expected_status: Optional::Absent,
        }),
    )
    .expect("the request encodes");
    let answer = server.answer(&request).expect("the daemon answers");
    let (result, payload) =
        decode_result("evidence.verify", &answer).expect("the client decodes the frame");
    assert_eq!(result.status, ResultStatus::Ok, "{:?}", result.error);
    match payload {
        Payload::EvidenceVerify(response) => {
            assert_eq!(response.status, EvidenceStatus::Validated);
            assert_eq!(response.validation_basis, "checked-certificate");
            assert_eq!(response.evidence_kind, EvidenceKind::Certificate);
        }
        other => panic!("expected an evidence.verify payload, got {other:?}"),
    }
    assert_eq!(status(server.daemon(), &handle), ClaimStatus::Validated);
}

#[test]
fn a_true_certificate_about_another_model_is_rejected() {
    // The attack bn-3hk4v closes. The certificate is true: the kernel re-derives every row
    // from the model it carries and verifies it. It is about the counter, and the node says
    // it derives from the Die Hard snapshot. Before bn-3hk4v this reached `validated`.
    let mut daemon = daemon();
    let snapshot = die_hard(&mut daemon);
    let bytes = certificate_for(&counter_model());
    assert_eq!(core_claim(&bytes), (2, false), "the kernel verifies it");
    let handle = bound_node(&mut daemon, "certs/counter.cert", PROFILE, bytes, &snapshot);

    let refused = verify(&mut daemon, &handle, "req_wrong_model");
    assert_eq!(code(&refused), ErrorCode::CertificateRejected);
    // No kernel rejected anything, so there is no kernel token to relay.
    assert!(
        matches!(refused.data, ErrorData::None),
        "a binding refusal is not the kernel's rejection: {:?}",
        refused.data
    );
    assert_eq!(refused.envelope.verdict, Nullable::Null);
    assert_eq!(status(&daemon, &handle), ClaimStatus::Proposed);
}

#[test]
fn a_claim_that_trusts_its_model_correspondence_does_not_reach_validated() {
    // Wire epoch 1 and the state-type family: the kernel verifies both, and both claims say
    // that nothing checked their relation against a model.
    for id in ["green:core/finite-closure", "green:core/state-type"] {
        let mut daemon = daemon();
        let snapshot = die_hard(&mut daemon);
        let bytes = corpus(id);
        assert_eq!(
            core_claim(&bytes),
            (1, true),
            "{id}: epoch 1, trusted correspondence"
        );
        let handle = bound_node(&mut daemon, "certs/epoch-1.cert", PROFILE, bytes, &snapshot);
        let refused = verify(&mut daemon, &handle, "req_epoch_1");
        assert_eq!(code(&refused), ErrorCode::InsufficientEvidence, "{id}");
        assert_eq!(status(&daemon, &handle), ClaimStatus::Proposed, "{id}");
    }

    // The same refusal without a snapshot: the binding is refused on the claim, before any
    // snapshot is read.
    let mut daemon = daemon();
    let handle = certificate_node(
        &mut daemon,
        "certs/epoch-1.cert",
        PROFILE,
        corpus("green:core/finite-closure"),
        Vec::new(),
    );
    assert_eq!(
        code(&verify(&mut daemon, &handle, "req_epoch_1_unbound")),
        ErrorCode::InsufficientEvidence
    );
}

/// Create a snapshot through `workspace.create`, sealed or not, carrying one module and no
/// registered model.
fn unregistered(daemon: &mut Daemon, key: &str, seal: bool) -> WorkspaceHandle {
    if seal {
        return model_binding::sealed_snapshot(
            daemon,
            create_envelope(key),
            "Unknown.ctm",
            b"module Unknown\n",
            None,
        );
    }
    // An unsealed workspace over the Die Hard module, whose model *is* registered: the
    // refusal is about the seal, not about the model.
    let sealed = die_hard(daemon);
    let intent = daemon
        .state()
        .workspace(&sealed)
        .expect("the sealed snapshot is held")
        .intent
        .clone();
    let file = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new(model_binding::DIE_HARD_PATH).expect("a workspace path"),
            model_binding::DIE_HARD_MODULE.as_bytes().to_vec(),
        )
        .expect("staging names its content");
    // A second file, so the unsealed workspace is not the sealed one: a workspace handle is
    // the content identity of its descriptor. The `.ctm` module set, and so the model, is
    // the same.
    let readme = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("README.md").expect("a workspace path"),
            b"# unsealed\n".to_vec(),
        )
        .expect("staging names its content");
    let epoch = |token: &str| EpochIdentity::new(token).expect("a well-formed epoch identity");
    let created = daemon.dispatch(&OperationRequest {
        envelope: create_envelope(key),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: SnapshotComponents {
                files: vec![file, readme],
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
                configuration: Vec::new(),
                file_components: Optional::Absent,
            },
            overlay: Optional::Absent,
            seal: Optional::Present(false),
        }),
    });
    let snapshot = match &created.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    };
    assert!(
        !daemon
            .state()
            .workspace(&snapshot)
            .expect("the workspace is held")
            .sealed(),
        "the fixture is unsealed"
    );
    snapshot
}

#[test]
fn every_missing_held_model_fails_closed() {
    let die_hard_certificate = || certificate_for(&diehard::model().expect("the port builds"));

    // (case, the node's derivation inputs)
    let mut daemon = daemon();
    let die_hard_snapshot = die_hard(&mut daemon);
    let counter_snapshot = counter(&mut daemon);
    let unregistered_snapshot = unregistered(&mut daemon, "seal-unknown", true);
    let unsealed_snapshot = unregistered(&mut daemon, "unsealed", false);
    let cases: Vec<(&str, Vec<String>)> = vec![
        ("no snapshot", Vec::new()),
        (
            "an input that is not a snapshot",
            vec!["trace_not-a-snapshot".to_owned()],
        ),
        (
            "an unknown snapshot",
            vec!["ws_nothing-is-held-here".to_owned()],
        ),
        (
            "two different snapshots",
            vec![
                die_hard_snapshot.as_str().to_owned(),
                counter_snapshot.as_str().to_owned(),
            ],
        ),
        (
            "a sealed snapshot with no registered model",
            vec![unregistered_snapshot.as_str().to_owned()],
        ),
        (
            "an unsealed snapshot",
            vec![unsealed_snapshot.as_str().to_owned()],
        ),
    ];
    for (index, (case, inputs)) in cases.into_iter().enumerate() {
        let handle = certificate_node(
            &mut daemon,
            &format!("certs/missing-{index}.cert"),
            &format!("{PROFILE}/{index}"),
            die_hard_certificate(),
            inputs,
        );
        let refused = verify(&mut daemon, &handle, &format!("req_missing_{index}"));
        assert_eq!(code(&refused), ErrorCode::InsufficientEvidence, "{case}");
        assert_eq!(status(&daemon, &handle), ClaimStatus::Proposed, "{case}");
    }

    // A repeated spelling of the one snapshot is one snapshot, not two.
    let handle = certificate_node(
        &mut daemon,
        "certs/repeated.cert",
        PROFILE,
        die_hard_certificate(),
        vec![
            die_hard_snapshot.as_str().to_owned(),
            die_hard_snapshot.as_str().to_owned(),
        ],
    );
    assert_eq!(
        verified_status(&verify(&mut daemon, &handle, "req_repeated")),
        EvidenceStatus::Validated
    );
}

#[test]
fn an_undecodable_carried_model_is_the_kernels_rejection() {
    // The daemon does not decode the carried model; the kernel does. Corrupt the model's
    // version tag and the kernel rejects the certificate, before any binding.
    let mut daemon = daemon();
    let snapshot = die_hard(&mut daemon);
    let mut bytes = certificate_for(&diehard::model().expect("the port builds"));
    let tag = bytes
        .windows(b"continuum-model/1".len())
        .position(|window| window == b"continuum-model/1")
        .expect("the certificate carries the model tag");
    bytes[tag] = b'X';
    let handle = bound_node(
        &mut daemon,
        "certs/undecodable.cert",
        PROFILE,
        bytes,
        &snapshot,
    );
    let refused = verify(&mut daemon, &handle, "req_undecodable");
    assert_eq!(code(&refused), ErrorCode::CertificateRejected);
    match &refused.data {
        ErrorData::CertificateRejection(data) => {
            assert_eq!(data.checker, "continuum-kernel-core");
        }
        ErrorData::None => panic!("the kernel's rejection carries its data"),
    }
    assert_eq!(status(&daemon, &handle), ClaimStatus::Proposed);
}

#[test]
fn the_same_model_built_again_binds() {
    // Identity is the canonical encoding, compared exactly (ADR-0013). A snapshot whose
    // module bytes differ, registered with Die Hard built a second time, holds the same
    // model: the certificate from the first construction binds to it.
    let mut daemon = daemon();
    let first = die_hard(&mut daemon);
    let variant = format!("{}\n// re-filed\n", model_binding::DIE_HARD_MODULE);
    let second = model_binding::sealed_snapshot(
        &mut daemon,
        create_envelope("seal-die-hard-again"),
        model_binding::DIE_HARD_PATH,
        variant.as_bytes(),
        Some(diehard::model().expect("the port builds again")),
    );
    assert_ne!(first, second, "two snapshots");
    assert_eq!(
        diehard::model().expect("builds").identity(),
        diehard::model().expect("builds").identity(),
        "one canonical encoding"
    );

    let bytes = certificate_for(&diehard::model().expect("the port builds"));
    for (index, snapshot) in [first, second].iter().enumerate() {
        let handle = bound_node(
            &mut daemon,
            "certs/die-hard.cert",
            &format!("{PROFILE}/{index}"),
            bytes.clone(),
            snapshot,
        );
        assert_eq!(
            verified_status(&verify(&mut daemon, &handle, &format!("req_again_{index}"))),
            EvidenceStatus::Validated
        );
    }
}

#[test]
fn two_snapshots_with_different_models_bind_apart() {
    let mut daemon = daemon();
    let die_hard_snapshot = die_hard(&mut daemon);
    let counter_snapshot = counter(&mut daemon);
    let die_hard_bytes = certificate_for(&diehard::model().expect("the port builds"));
    let counter_bytes = certificate_for(&counter_model());

    // (certificate, snapshot, expected)
    let cases = [
        (&counter_bytes, &counter_snapshot, None),
        (&die_hard_bytes, &die_hard_snapshot, None),
        (
            &counter_bytes,
            &die_hard_snapshot,
            Some(ErrorCode::CertificateRejected),
        ),
        (
            &die_hard_bytes,
            &counter_snapshot,
            Some(ErrorCode::CertificateRejected),
        ),
    ];
    for (index, (bytes, snapshot, expected)) in cases.into_iter().enumerate() {
        let handle = bound_node(
            &mut daemon,
            &format!("certs/apart-{index}.cert"),
            &format!("{PROFILE}/{index}"),
            bytes.clone(),
            snapshot,
        );
        let outcome = verify(&mut daemon, &handle, &format!("req_apart_{index}"));
        match expected {
            None => {
                assert_eq!(
                    verified_status(&outcome),
                    EvidenceStatus::Validated,
                    "case {index}"
                );
            }
            Some(expected) => {
                assert_eq!(code(&outcome), expected, "case {index}");
                assert_eq!(
                    status(&daemon, &handle),
                    ClaimStatus::Proposed,
                    "case {index}"
                );
            }
        }
    }
}

#[test]
fn the_derived_snapshot_is_decided_against_the_grant() {
    // `cap_evidence_only` holds the `ev` class and not `ws`. It may name the node, and the
    // snapshot the node derives from is decided against its grant before anything about the
    // snapshot is read. The refusal is the same whether the binding would hold or fail, so
    // the answer says nothing about the snapshot's model.
    let mut daemon = daemon();
    let snapshot = die_hard(&mut daemon);
    let matching = bound_node(
        &mut daemon,
        "certs/matching.cert",
        PROFILE,
        certificate_for(&diehard::model().expect("the port builds")),
        &snapshot,
    );
    let mismatched = bound_node(
        &mut daemon,
        "certs/mismatched.cert",
        PROFILE,
        certificate_for(&counter_model()),
        &snapshot,
    );
    for (name, handle) in [("matching", &matching), ("mismatched", &mismatched)] {
        let refused = verify_as(
            &mut daemon,
            "agent:narrow",
            "cap_evidence_only",
            handle,
            &format!("req_narrow_{name}"),
        );
        assert_eq!(code(&refused), ErrorCode::CapabilityDenied, "{name}");
        assert_eq!(status(&daemon, handle), ClaimStatus::Proposed, "{name}");
    }
    // The control: the full grant reaches the binding.
    assert_eq!(
        verified_status(&verify(&mut daemon, &matching, "req_full_matching")),
        EvidenceStatus::Validated
    );
}

#[test]
fn a_derived_receipt_does_not_inherit_a_binding() {
    // `evidence.link` appends a receipt node over a certificate subject, and the receipt
    // inherits the subject's evidence class, so `evidence.verify` routes its bytes to the
    // kernel. Make those bytes a true certificate about another model. The receipt's
    // derivation is the receipt itself, not the subject's snapshot, so it binds to nothing.
    let mut daemon = daemon();
    let snapshot = die_hard(&mut daemon);
    let subject = bound_node(
        &mut daemon,
        "certs/subject.cert",
        PROFILE,
        certificate_for(&diehard::model().expect("the port builds")),
        &snapshot,
    );
    let receipt: Commitment = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("receipts/counter.cert").expect("a workspace path"),
            certificate_for(&counter_model()),
        )
        .expect("staging names its content");
    let mut link = envelope(
        "evidence.link",
        "service:kernel-core",
        "cap_checker",
        "req_link",
    );
    link.idempotency_key = Optional::Present("idem-link".to_owned());
    let linked = daemon.dispatch(&OperationRequest {
        envelope: link,
        arguments: Arguments::EvidenceLink(EvidenceLinkRequest {
            subject,
            receipt,
            checker_profile: "kernel-core/1".to_owned(),
        }),
    });
    assert_eq!(
        linked.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        linked.envelope.error
    );
    let receipt_node = match &linked.payload {
        Payload::EvidenceLink(response) => response.receipt.clone(),
        other => panic!("expected an evidence.link payload, got {other:?}"),
    };
    assert_eq!(
        daemon
            .state()
            .evidence(&receipt_node)
            .expect("the receipt node is held")
            .evidence_kind,
        Some(EvidenceKind::Certificate),
        "the receipt routes to the certificate lane"
    );
    let refused = verify(&mut daemon, &receipt_node, "req_receipt");
    assert_eq!(code(&refused), ErrorCode::InsufficientEvidence);
    // The receipt's input is its own content commitment, spelled in the `ws_` class. It is
    // not read as a snapshot: the refusal is that the node names none.
    let detail = &refused
        .envelope
        .error
        .value()
        .expect("an error result carries one")
        .detail;
    assert!(detail.contains("names no snapshot"), "{detail}");
    assert_eq!(status(&daemon, &receipt_node), ClaimStatus::Proposed);
}

#[test]
fn a_refutation_whose_encoding_is_trusted_does_not_reach_validated() {
    // An LRAT or SMT refutation is checked against the formula or skeleton it carries, and
    // its claim trusts that this is the encoding of the model and property the envelope
    // names (`formula-model-correspondence`, `skeleton-model-correspondence`). A true
    // refutation of an unrelated formula is exactly that residual, so it does not reach
    // `validated` (adversarial review of bn-3hk4v; before it, both promoted).
    for id in ["green:sat/lrat", "green:smt/smt-proof"] {
        let bytes = corpus(id);
        assert!(
            matches!(
                continuum_certificate::check_certificate(&bytes),
                Outcome::Checked(
                    KernelVerdict::Sat(
                        continuum_certificate::continuum_kernel_sat::Verdict::Verified(_)
                    ) | KernelVerdict::Smt(
                        continuum_certificate::continuum_kernel_smt::Verdict::Verified(_)
                    )
                )
            ),
            "{id}: the kernel verifies it"
        );
        let mut daemon = daemon();
        let snapshot = die_hard(&mut daemon);
        let handle = bound_node(
            &mut daemon,
            "certs/refutation.cert",
            PROFILE,
            bytes,
            &snapshot,
        );
        let refused = verify(&mut daemon, &handle, "req_refutation");
        assert_eq!(code(&refused), ErrorCode::InsufficientEvidence, "{id}");
        assert_eq!(status(&daemon, &handle), ClaimStatus::Proposed, "{id}");
    }
}

#[test]
fn a_caller_refused_the_snapshot_learns_nothing_about_the_certificate() {
    // The inputs are decided against the grant before the kernel runs, so a caller without
    // the snapshot class gets the one `CapabilityDenied` for a valid certificate, a
    // corrupted one, and one that trusts its model correspondence alike.
    let mut daemon = daemon();
    let snapshot = die_hard(&mut daemon);
    let mut corrupted = certificate_for(&diehard::model().expect("the port builds"));
    corrupted.push(0);
    let cases = [
        (
            "valid",
            certificate_for(&diehard::model().expect("the port builds")),
        ),
        ("corrupted", corrupted),
        ("epoch-1", corpus("green:core/finite-closure")),
    ];
    for (name, bytes) in cases {
        let handle = bound_node(
            &mut daemon,
            &format!("certs/narrow-{name}.cert"),
            &format!("{PROFILE}/{name}"),
            bytes,
            &snapshot,
        );
        let refused = verify_as(
            &mut daemon,
            "agent:narrow",
            "cap_evidence_only",
            &handle,
            &format!("req_narrow_learns_{name}"),
        );
        assert_eq!(code(&refused), ErrorCode::CapabilityDenied, "{name}");
        assert!(
            matches!(refused.data, ErrorData::None),
            "{name}: no kernel data"
        );
    }
}

/// Verify `handle` as `agent:reader` under `capability`, with one fixed idempotency key, so
/// a second call is a replay of the first whatever capability it presents.
fn verify_keyed(
    daemon: &mut Daemon,
    capability: &str,
    handle: &EvidenceHandle,
    request: &str,
) -> OperationOutcome {
    let mut envelope = verify_envelope("agent:reader", capability, request);
    envelope.idempotency_key = Optional::Present("idem-replayed-verify".to_owned());
    daemon.dispatch(&OperationRequest {
        envelope,
        arguments: Arguments::EvidenceVerify(EvidenceVerifyRequest {
            evidence: handle.clone(),
            expected_status: Optional::Absent,
        }),
    })
}

#[test]
fn a_replay_under_a_grant_without_the_snapshot_is_denied() {
    // cr-3lrkq3. The first call runs under the full grant and binds the claim to the
    // snapshot. The replay presents the same actor's `ev`-only grant: it admits the node
    // the answer names, and not the snapshot the binding read. A fresh call under it is
    // denied, so the replay is denied too, and it returns nothing of the recorded answer.
    // Before cr-3lrkq3 the replay returned `validated`.
    let mut daemon = daemon();
    let snapshot = die_hard(&mut daemon);
    let handle = bound_node(
        &mut daemon,
        "certs/replayed.cert",
        PROFILE,
        certificate_for(&diehard::model().expect("the port builds")),
        &snapshot,
    );
    let first = verify_keyed(&mut daemon, "cap_reader", &handle, "req_replay_first");
    assert_eq!(verified_status(&first), EvidenceStatus::Validated);

    let replayed = verify_keyed(
        &mut daemon,
        "cap_reader_narrow",
        &handle,
        "req_replay_narrow",
    );
    assert_eq!(code(&replayed), ErrorCode::CapabilityDenied);
    assert_eq!(replayed.payload, Payload::None, "no payload");
    assert_eq!(replayed.envelope.verdict, Nullable::Null, "no verdict");
    assert!(matches!(replayed.data, ErrorData::None), "no data");

    // The narrow grant passed admission, and the replay check is what refused it: the
    // test is not a denial at T1-T4, which the pre-fix code would give as well.
    let record = daemon
        .state()
        .admissions()
        .last()
        .expect("the replay was admitted and recorded");
    assert_eq!(record.capability.as_str(), "cap_reader_narrow");
    assert!(record.admitted, "admission accepted the narrow grant");
    assert_eq!(record.denial, Some(Denial::ReplayAuthority));

    // The same denial a fresh call under that grant gets: learn nothing.
    let fresh = verify_as(
        &mut daemon,
        "agent:reader",
        "cap_reader_narrow",
        &handle,
        "req_fresh_narrow",
    );
    assert_eq!(code(&fresh), ErrorCode::CapabilityDenied);
    let detail = |outcome: &OperationOutcome| {
        outcome
            .envelope
            .error
            .value()
            .expect("an error result carries one")
            .detail
            .clone()
    };
    assert_eq!(detail(&replayed), detail(&fresh));
    let record = daemon.state().admissions().last().expect("recorded");
    assert!(record.admitted);
    assert_eq!(
        record.denial,
        Some(Denial::DerivedHandle),
        "the fresh call's binding refused it"
    );
}

#[test]
fn a_replay_under_the_full_grant_returns_the_recorded_answer() {
    let mut daemon = daemon();
    let snapshot = die_hard(&mut daemon);
    let handle = bound_node(
        &mut daemon,
        "certs/replayed.cert",
        PROFILE,
        certificate_for(&diehard::model().expect("the port builds")),
        &snapshot,
    );
    let first = verify_keyed(&mut daemon, "cap_reader", &handle, "req_replay_first");
    assert_eq!(verified_status(&first), EvidenceStatus::Validated);
    let replayed = verify_keyed(&mut daemon, "cap_reader", &handle, "req_replay_again");
    assert_eq!(verified_status(&replayed), EvidenceStatus::Validated);
    assert_eq!(replayed.payload, first.payload, "the recorded answer");

    // A sibling grant that does not cover the recording one, and admits the snapshot and
    // every name in the answer, replays it too: the check is not over-strict.
    let sibling = verify_keyed(
        &mut daemon,
        "cap_reader_sibling",
        &handle,
        "req_replay_sibling",
    );
    assert_eq!(verified_status(&sibling), EvidenceStatus::Validated);
    assert_eq!(sibling.payload, first.payload, "the recorded answer");
    assert_eq!(
        daemon.state().admissions().last().expect("recorded").denial,
        None
    );
}
