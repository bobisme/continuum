//! The production receipt-signing path (bn-1hape, plan §18.6, ADR-0054, TEST-9-07).
//!
//! `evidence.link` is the daemon's receipt producer: it publishes a checker's receipt and
//! appends the `receipt` node. With a `ReceiptSigner` supplied — here the solo-developer key
//! from `continuum_security::keystore::LocalKeystore`, minted on first use from the
//! operating-system entropy capability — the daemon signs the receipt's canonical bytes
//! through `SigningRegistry::sign` before publishing. These tests drive the real operation
//! through `Daemon::dispatch` and verify what it produced with `SignatureVerifier`.

use std::fs;
use std::path::PathBuf;

use continuum_evidence::actor::ActorId as SigningActor;
use continuum_evidence::signing::{
    AllowedSigners, RevocationReason, SignatureVerifier, SignedArtifactKind, UnverifiedReason,
};
use continuum_security::entropy::OsEntropy;
use continuum_security::keystore::LocalKeystore;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::evidence::{EvidenceFamily, signed_receipt_identity};
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest, ReceiptSigner};
use continuumd::protocol::envelope::{Budget, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::evidence::EvidenceLinkRequest;
use continuumd::protocol::operations::observe::ObserveIngestRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, EvidenceHandle, Opaque, OperationName, ProtocolVersion,
    RequestId, Timestamp,
};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{AuthorityLevel, DataGrant, Encoding, ErrorCode};

const PRODUCER: &str = "agent:producer";
const CHECKER: &str = "service:checker";
const TRACE: &str = "{\"events\":[{\"at\":0,\"op\":\"fill\"}]}\n";
const RECEIPT_BLOB: &str = "{\"receipt\":\"the checker re-checked this\"}\n";

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 3)
}

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("capability")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("actor")
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
        delegation_depth: depth,
        profile: Optional::Present(CapabilityProfile {
            privileged_operations: Vec::new(),
            denied_operations: Vec::new(),
            data_grants: vec![DataGrant::ProductionTrace],
            cross_principal_sharing: false,
        }),
    }
}

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-receipt-signing".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.3 is served")
}

struct World {
    daemon: Daemon,
    trace: Commitment,
    receipt: Commitment,
}

fn world(signer: Option<ReceiptSigner>) -> World {
    let root = Some(cap("cap_root"));
    let mut builder = Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .now(Timestamp::new("2026-09-23T00:00:00.000Z").expect("timestamp"))
        .capability(
            grant("cap_root", "service:continuumd", AuthorityLevel::Promote, 4),
            None,
        )
        .capability(
            grant("cap_producer", PRODUCER, AuthorityLevel::Execute, 3),
            root.clone(),
        )
        .capability(
            grant("cap_checker", CHECKER, AuthorityLevel::Promote, 3),
            root,
        )
        .family(EvidenceFamily::verifying_as(who(CHECKER)))
        .family(ObserveFamily);
    if let Some(signer) = signer {
        builder = builder.receipt_signer(signer);
    }
    let mut daemon = builder.build();
    let mut stage = |path: &str, content: &str| {
        daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new(path).expect("path"),
                content.as_bytes().to_vec(),
            )
            .expect("staged")
    };
    let trace = stage("traces/claim.jsonl", TRACE);
    let receipt = stage("receipts/checker.json", RECEIPT_BLOB);
    World {
        daemon,
        trace,
        receipt,
    }
}

fn envelope(operation: &str, actor: &str, capability: &str, key: &str) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(&format!("req_{key}")).expect("request id"),
        idempotency_key: Optional::Present(key.to_owned()),
        actor: who(actor),
        capability: cap(capability),
        operation: OperationName::new(operation).expect("operation"),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: Optional::Absent,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

fn subject(world: &mut World) -> EvidenceHandle {
    let mut ingest = envelope("observe.ingest", PRODUCER, "cap_producer", "ingest");
    ingest.budget = Optional::Present(Budget {
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
    let outcome = world.daemon.dispatch(&OperationRequest {
        envelope: ingest,
        arguments: Arguments::ObserveIngest(ObserveIngestRequest {
            trace: world.trace.clone(),
            instrumentation_profile: "otel-1.0/sampled".to_owned(),
        }),
    });
    match &outcome.payload {
        Payload::ObserveIngest(response) => response.evidence[0].clone(),
        other => panic!("expected an ingest payload, got {other:?}"),
    }
}

fn link(world: &mut World, subject: &EvidenceHandle) -> OperationOutcome {
    world.daemon.dispatch(&OperationRequest {
        envelope: envelope("evidence.link", CHECKER, "cap_checker", "link"),
        arguments: Arguments::EvidenceLink(EvidenceLinkRequest {
            subject: subject.clone(),
            receipt: world.receipt.clone(),
            checker_profile: "kernel-core/1".to_owned(),
        }),
    })
}

fn receipt_handle(outcome: &OperationOutcome) -> EvidenceHandle {
    match &outcome.payload {
        Payload::EvidenceLink(response) => response.receipt.clone(),
        other => panic!(
            "expected a link payload, got {other:?}; {:?}",
            outcome.error_code()
        ),
    }
}

fn keystore_signer(name: &str) -> ReceiptSigner {
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "continuumd-receipt-signing-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    let opened = LocalKeystore::new(dir.join("store"))
        .open_or_mint(
            &SigningActor::new("human:solo-dev").expect("actor"),
            &mut OsEntropy::new(),
        )
        .expect("the local key is minted on first use");
    let (registry, signer) = opened.into_parts();
    ReceiptSigner::new(registry, signer)
}

#[test]
fn a_receipt_evidence_link_produces_is_signed_and_verifies() {
    let signer = keystore_signer("verifies");
    let identity = signer.identity().clone();
    let registry = signer.registry().clone();
    let mut world = world(Some(signer));
    let subject = subject(&mut world);
    let outcome = link(&mut world, &subject);
    assert_eq!(outcome.error_code(), None);
    let receipt = receipt_handle(&outcome);

    let signature = world
        .daemon
        .state()
        .receipt_signature(&receipt)
        .cloned()
        .expect("the produced receipt carries a signature");
    assert_eq!(signature.signer(), &identity);
    assert_eq!(signature.kind(), SignedArtifactKind::Receipt);

    let allowed = AllowedSigners::new().allow(identity, [SignedArtifactKind::Receipt]);
    let head = registry.head();
    let verifier = SignatureVerifier::new(&allowed, &registry, &head);
    let artifact = signed_receipt_identity(RECEIPT_BLOB.as_bytes());
    assert!(
        verifier
            .verify_encoded(
                SignedArtifactKind::Receipt,
                &artifact,
                Some(&signature.encode())
            )
            .verified()
            .is_some(),
        "the receipt the daemon produced verifies from its wire bytes"
    );
    assert!(
        verifier
            .verify_for_ci_acceptance(
                SignedArtifactKind::Receipt,
                &artifact,
                Some(&signature.encode())
            )
            .is_ok()
    );
}

#[test]
fn a_receipt_tampered_after_production_does_not_verify() {
    let signer = keystore_signer("tamper");
    let identity = signer.identity().clone();
    let registry = signer.registry().clone();
    let mut world = world(Some(signer));
    let subject = subject(&mut world);
    let receipt = receipt_handle(&link(&mut world, &subject));
    let signature = world
        .daemon
        .state()
        .receipt_signature(&receipt)
        .cloned()
        .expect("signed");

    let allowed = AllowedSigners::new().allow(identity, [SignedArtifactKind::Receipt]);
    let head = registry.head();
    let verifier = SignatureVerifier::new(&allowed, &registry, &head);
    let mut tampered = RECEIPT_BLOB.as_bytes().to_vec();
    tampered[3] ^= 0x01;
    assert_eq!(
        verifier
            .verify(
                SignedArtifactKind::Receipt,
                &signed_receipt_identity(&tampered),
                Some(&signature)
            )
            .unverified()
            .map(|u| u.reason().clone()),
        Some(UnverifiedReason::SignatureMismatch)
    );
    assert!(
        verifier
            .verify_for_ci_acceptance(
                SignedArtifactKind::Receipt,
                &signed_receipt_identity(&tampered),
                Some(&signature.encode())
            )
            .is_err()
    );
}

#[test]
fn a_daemon_without_a_signer_publishes_unsigned_and_a_verifier_says_so() {
    let mut world = world(None);
    let subject = subject(&mut world);
    let receipt = receipt_handle(&link(&mut world, &subject));
    assert!(world.daemon.state().receipt_signature(&receipt).is_none());
    let registry = continuum_evidence::signing::SigningRegistry::new();
    let allowed = AllowedSigners::new();
    let head = registry.head();
    let verifier = SignatureVerifier::new(&allowed, &registry, &head);
    assert_eq!(
        verifier
            .verify_encoded(
                SignedArtifactKind::Receipt,
                &signed_receipt_identity(RECEIPT_BLOB.as_bytes()),
                None
            )
            .unverified()
            .map(|u| u.reason().clone()),
        Some(UnverifiedReason::Unsigned)
    );
}

#[test]
fn a_revoked_signing_identity_refuses_the_link_rather_than_publishing_unsigned() {
    let signer = keystore_signer("revoked");
    let identity = signer.identity().clone();
    let mut registry = signer.registry().clone();
    registry
        .revoke(
            &identity,
            &SigningActor::new("human:solo-dev").expect("actor"),
            RevocationReason::Compromised,
        )
        .expect("revoke");
    // Rebuild the signer over the revoked registry, keeping the key: restore it from the
    // same store rather than reaching into the signer.
    let dir = std::env::temp_dir().join(format!(
        "continuumd-receipt-signing-{}-revoked",
        std::process::id()
    ));
    let key = LocalKeystore::new(dir.join("store"))
        .open()
        .expect("reopens")
        .into_parts()
        .1;
    let mut world = world(Some(ReceiptSigner::new(registry, key)));
    let subject = subject(&mut world);
    let outcome = link(&mut world, &subject);
    assert_eq!(
        outcome.error_code(),
        Some(ErrorCode::UnsupportedSemanticFeature)
    );
    assert!(
        world.daemon.state().evidence(&subject).is_some(),
        "the subject is untouched"
    );
}

/// Differential: the daemon (`continuumd`, subject) against the signing library
/// (`continuum_evidence::signing`, oracle). Ed25519 is deterministic, so the signature
/// `evidence.link` recorded must be byte-identical to the one the library makes when the
/// same key signs `signed_receipt_identity` of the same receipt directly.
#[test]
fn the_daemons_receipt_signature_matches_the_library_signing_the_same_bytes() {
    let signer = keystore_signer("differential");
    let registry = signer.registry().clone();
    let dir = std::env::temp_dir().join(format!(
        "continuumd-receipt-signing-{}-differential",
        std::process::id()
    ));
    let oracle_key = LocalKeystore::new(dir.join("store"))
        .open()
        .expect("reopens")
        .into_parts()
        .1;
    let mut world = world(Some(signer));
    let subject = subject(&mut world);
    let receipt = receipt_handle(&link(&mut world, &subject));
    let produced = world
        .daemon
        .state()
        .receipt_signature(&receipt)
        .cloned()
        .expect("signed");
    let oracle = continuum_evidence::signing::SigningRegistry::sign(
        &registry,
        &oracle_key,
        SignedArtifactKind::Receipt,
        &signed_receipt_identity(RECEIPT_BLOB.as_bytes()),
    )
    .expect("the library signs");
    assert_eq!(produced, oracle);
    assert_eq!(produced.encode(), oracle.encode());
}
