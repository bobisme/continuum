//! The `evidence` and `observe` operation families, the capability-negotiation surface,
//! and INV-004.
//!
//! # What these tests are evidence for
//!
//! The bone this file lands with carries `req:inv-004`, and the dimension it owes is the
//! one no other Bone could reach: **a producer may not promote its own claim**. Everything
//! below runs through [`Daemon::dispatch`] or [`Daemon::welcome`] — the same two entry
//! points a transport calls — with no filesystem, no clock beyond the reading the daemon is
//! handed, no runtime and no network.
//!
//! The INV-004 claim is made in four independent ways, deliberately, because a single test
//! of an authority rule proves only that one path was closed:
//!
//! 1. a producer's append lands at the lattice's bottom and no request field reaches the
//!    status ([`a_producers_append_lands_at_the_lattices_bottom`]);
//! 2. the verification service is the only status-advancing path, and the status it writes
//!    is what it checked rather than what the caller asked for
//!    ([`the_status_written_is_what_the_checker_established_not_what_the_caller_named`]);
//! 3. a service that produced a claim may not verify it
//!    ([`a_verification_service_may_not_promote_a_claim_it_produced_itself`]);
//! 4. a class whose checker this process cannot reach is never promoted
//!    ([`a_class_whose_independent_checker_has_not_shipped_is_never_promoted`]), and the one
//!    class it *can* reach is promoted by the kernel's verdict rather than by this daemon's
//!    opinion of it (the certificate lane below, bn-dtg61).
//!
//! A fifth is not a test at all and is stronger than any of them: `daemon::observe` cannot
//! construct a `daemon::evidence::Promotion`, so the producer-side code *cannot be written*
//! to promote. That is checked by the compiler on every build.

use continuum_certificate::{KernelVerdict, Outcome, continuum_kernel_core};
use continuum_evidence::claim_status::{ClaimStatus, verify_promotion_history};
use continuum_value::epoch::ProtocolWindow;
use continuumd::codec;
use continuumd::daemon::capability::{ConnectionPolicy, Refusal};
use continuumd::daemon::evidence::{
    DEFAULT_SERVICE, EDGE_IDENTITY_DOMAIN, EvidenceFamily, lattice_status, wire_status,
};
use continuumd::daemon::family::{Arguments, ErrorData, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::daemon::state::{EvidenceNode, StatusWrite};
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest, errors, evidence, observe};
use continuumd::protocol::envelope::{
    Budget, EnvelopeDimension, Redacted, RequestEnvelope, Verdict,
};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, ServerLimits, VersionRange,
    negotiate,
};
use continuumd::protocol::operations::evidence::{
    EvidenceGetRequest, EvidenceLinkRequest, EvidenceQueryRequest, EvidenceSubscribeRequest,
    EvidenceVerifyRequest,
};
use continuumd::protocol::operations::observe::{
    ObserveClassifyRequest, ObserveIngestRequest, ObserveResultRequest,
};
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, ByteCount, CapabilityHandle, Commitment, DurationMs, EvidenceHandle, Opaque,
    OperationName, ProtocolVersion, RequestId, Timestamp,
};
use continuumd::protocol::shared::EvidenceQuery;
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{
    AssuranceClass, AuthorityLevel, DataGrant, Encoding, ErrorCode, EvidenceEdgeKind,
    EvidenceEventKind, EvidenceKind, EvidenceNodeKind, EvidenceStatus, InconclusiveReason,
    OmissionReason, RedactionReason, ResultStatus, SemanticVerdict,
};
use continuumd::transport::{Server, decode_result, encode_request};

use continuum_workspace::snapshot::WorkspacePath;

/// A production trace, as a captured bundle would arrive.
const TRACE: &str = "{\"events\":[{\"at\":0,\"op\":\"fill\"},{\"at\":1,\"op\":\"pour\"}]}\n";

/// A second, different trace, so a query has more than one node to filter.
const OTHER_TRACE: &str = "{\"events\":[{\"at\":0,\"op\":\"empty\"}]}\n";

/// The instrumentation profile the traces above were captured under (plan §18.4).
const PROFILE: &str = "otel-1.0/sampled";

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

fn name(operation: &str) -> OperationName {
    OperationName::new(operation).expect("a well-formed operation name")
}

fn when(text: &str) -> Timestamp {
    Timestamp::new(text).expect("a well-formed timestamp")
}

/// The reading every daemon in this file is built with, unless a test needs otherwise.
fn now() -> Timestamp {
    when("2026-08-01T00:00:00.000Z")
}

fn traced(grants: &[DataGrant]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: Vec::new(),
        denied_operations: Vec::new(),
        data_grants: grants.to_vec(),
        cross_principal_sharing: false,
    }
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

fn negotiated() -> Negotiated {
    negotiate(
        &[version()],
        ProtocolWindow::new(3),
        ENCODINGS,
        &hello("cap_root", "service:continuumd", version(), version()),
    )
    .expect("3.1 is served")
}

fn hello(
    capability: &str,
    actor: &str,
    low: ProtocolVersion,
    high: ProtocolVersion,
) -> ClientHello {
    ClientHello {
        protocol_versions: VersionRange { low, high },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-daemon-evidence-test".to_owned(),
        actor: who(actor),
        capability: cap(capability),
        features: Optional::Absent,
    }
}

/// The capability tree, with the connection capability at its root.
///
/// Every non-root capability is a registered *delegation* of `cap_root`: RFC 0027 defines a
/// narrower capability as "the connection's own capability or a descendant of it in the
/// delegation tree", so a suite whose capabilities were unrelated roots would exercise
/// D6/D7 nowhere.
fn daemon_with(clock: Option<Timestamp>) -> Daemon {
    let root = Some(cap("cap_root"));
    let mut builder = Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .capability(
            {
                let mut descriptor = grant(
                    "cap_root",
                    "service:continuumd",
                    AuthorityLevel::Promote,
                    4,
                    Optional::Present(traced(&[DataGrant::ProductionTrace])),
                );
                descriptor.delegation_depth = 4;
                descriptor
            },
            None,
        )
        // The producer: `execute` plus the plan §18.2 production-trace grant `observe.ingest`
        // requires beyond its level (RFC 0027 R-4).
        .capability(
            grant(
                "cap_observer",
                "agent:observer",
                AuthorityLevel::Execute,
                3,
                Optional::Present(traced(&[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        )
        // A second producer, so two nodes can carry two different producers.
        .capability(
            grant(
                "cap_second",
                "agent:second-observer",
                AuthorityLevel::Execute,
                3,
                Optional::Present(traced(&[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        )
        // A reader: enough for every `evidence` operation, which is entirely `read`.
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
        // `execute` and no production-trace grant: the level admits `observe.ingest` and the
        // profile does not.
        .capability(
            grant(
                "cap_ungranted",
                "agent:ungranted",
                AuthorityLevel::Execute,
                3,
                Optional::Present(traced(&[])),
            ),
            root.clone(),
        )
        // The self-certification fixture: a producer whose actor *is* the daemon's
        // verification service identity.
        .capability(
            grant(
                "cap_service",
                DEFAULT_SERVICE,
                AuthorityLevel::Execute,
                3,
                Optional::Present(traced(&[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        )
        // A checker: a `service:` actor at `execute`, for `evidence.link`. RFC 0038
        // "Authority" gates a check edge on the actor *scheme*, not on the ladder, so this
        // capability differs from `cap_observer` in exactly the thing under test.
        .capability(
            grant(
                "cap_checker",
                "service:kernel-core",
                AuthorityLevel::Execute,
                3,
                Optional::Present(traced(&[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        )
        // A *second* checker, so the graph can hold a two-edge chain: a receipt produced by
        // `service:kernel-core` cannot be checked by `service:kernel-core` (INV-004), and a
        // path of length two is what tells a bounded traversal from an unbounded one.
        .capability(
            grant(
                "cap_checker_smt",
                "service:kernel-smt",
                AuthorityLevel::Execute,
                3,
                Optional::Present(traced(&[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        )
        // Registered so it can be revoked mid-connection (RFC 0027 R1).
        .capability(
            grant(
                "cap_revocable",
                "agent:revocable",
                AuthorityLevel::Execute,
                3,
                Optional::Present(traced(&[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        )
        // A mis-provisioned delegation: a child of the *reader* claiming `execute` and a
        // data grant its parent does not hold. D6/D7 make a child's admission set a subset
        // of its parent's, so admission refuses it on the chain.
        .capability(
            grant(
                "cap_escalating",
                "agent:escalating",
                AuthorityLevel::Execute,
                2,
                Optional::Present(traced(&[DataGrant::ProductionTrace])),
            ),
            Some(cap("cap_reader")),
        )
        // Expiring, so expiry can be judged against a reading or refused without one.
        .capability(
            {
                let mut expiring = grant(
                    "cap_expiring",
                    "agent:expiring",
                    AuthorityLevel::Execute,
                    3,
                    Optional::Present(traced(&[DataGrant::ProductionTrace])),
                );
                expiring.expires_at = Nullable::Value(when("2026-07-01T00:00:00.000Z"));
                expiring
            },
            root,
        )
        .family(EvidenceFamily::new())
        .family(ObserveFamily);
    if let Some(reading) = clock {
        builder = builder.now(reading);
    }
    builder.build()
}

/// The daemon every test uses unless it needs a different clock.
fn daemon() -> Daemon {
    daemon_with(Some(now()))
}

/// A daemon holding both traces as staged content.
struct Fixture {
    daemon: Daemon,
    trace: Commitment,
    other: Commitment,
}

fn fixture() -> Fixture {
    fixture_with(Some(now()))
}

fn fixture_with(clock: Option<Timestamp>) -> Fixture {
    let mut daemon = daemon_with(clock);
    let stage = |daemon: &mut Daemon, path: &str, content: &str| {
        daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new(path).expect("a workspace path"),
                content.as_bytes().to_vec(),
            )
            .expect("staging names its content")
    };
    let trace = stage(&mut daemon, "traces/die-hard.jsonl", TRACE);
    let other = stage(&mut daemon, "traces/other.jsonl", OTHER_TRACE);
    Fixture {
        daemon,
        trace,
        other,
    }
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

fn budget() -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Absent,
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

/// A `@mutation @task_starting` envelope: a non-empty idempotency key and a budget, which
/// `obligation` requires before any family runs.
fn started(mut envelope: RequestEnvelope, key: &str) -> RequestEnvelope {
    envelope.idempotency_key = Optional::Present(key.to_owned());
    envelope.budget = Optional::Present(budget());
    envelope
}

fn ingest_with(
    fixture: &mut Fixture,
    actor: &str,
    capability: &str,
    trace: &Commitment,
    request: &str,
    key: &str,
) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: started(envelope("observe.ingest", actor, capability, request), key),
        arguments: Arguments::ObserveIngest(ObserveIngestRequest {
            trace: trace.clone(),
            instrumentation_profile: PROFILE.to_owned(),
        }),
    })
}

/// The green-path ingest: the observer appends the Die Hard trace.
fn ingest(fixture: &mut Fixture, request: &str, key: &str) -> OperationOutcome {
    let trace = fixture.trace.clone();
    ingest_with(
        fixture,
        "agent:observer",
        "cap_observer",
        &trace,
        request,
        key,
    )
}

fn ingested_handle(outcome: &OperationOutcome) -> EvidenceHandle {
    match &outcome.payload {
        Payload::ObserveIngest(response) => response
            .evidence
            .first()
            .cloned()
            .expect("an ingest names the node it appended"),
        other => panic!("expected an observe.ingest payload, got {other:?}"),
    }
}

fn verify_with(
    fixture: &mut Fixture,
    handle: &EvidenceHandle,
    expected: Optional<EvidenceStatus>,
    request: &str,
    key: &str,
) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: started(
            envelope("evidence.verify", "agent:reader", "cap_reader", request),
            key,
        ),
        arguments: Arguments::EvidenceVerify(EvidenceVerifyRequest {
            evidence: handle.clone(),
            expected_status: expected,
        }),
    })
}

fn verify(
    fixture: &mut Fixture,
    handle: &EvidenceHandle,
    request: &str,
    key: &str,
) -> OperationOutcome {
    verify_with(fixture, handle, Optional::Absent, request, key)
}

fn get(fixture: &mut Fixture, handle: &EvidenceHandle, request: &str) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("evidence.get", "agent:reader", "cap_reader", request),
        arguments: Arguments::EvidenceGet(EvidenceGetRequest {
            evidence: handle.clone(),
            inline: Optional::Absent,
        }),
    })
}

fn empty_query() -> EvidenceQuery {
    EvidenceQuery {
        node_kinds: Optional::Absent,
        edge_kinds: Optional::Absent,
        statuses: Optional::Absent,
        claim_id: Optional::Absent,
        roots: Optional::Absent,
        max_depth: Optional::Absent,
    }
}

fn query(fixture: &mut Fixture, query: EvidenceQuery, request: &str) -> OperationOutcome {
    fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("evidence.query", "agent:reader", "cap_reader", request),
        arguments: Arguments::EvidenceQuery(EvidenceQueryRequest { query }),
    })
}

fn code(outcome: &OperationOutcome) -> ErrorCode {
    outcome.error_code().expect("an error result")
}

fn node(fixture: &Fixture, handle: &EvidenceHandle) -> EvidenceNode {
    fixture
        .daemon
        .state()
        .evidence(handle)
        .expect("the graph holds the node")
        .clone()
}

fn verify_response(
    outcome: &OperationOutcome,
) -> &continuumd::protocol::operations::evidence::EvidenceVerifyResponse {
    match &outcome.payload {
        Payload::EvidenceVerify(response) => response,
        other => panic!("expected an evidence.verify payload, got {other:?}"),
    }
}

fn redaction(reason: RedactionReason, commitment: &Commitment) -> Redacted {
    Redacted {
        redacted: true,
        reason,
        commitment: commitment.clone(),
        original_class: "ev".to_owned(),
    }
}

// --- the green path ----------------------------------------------------------------------

#[test]
fn a_trace_is_ingested_and_then_independently_verified_through_the_operation_layer() {
    let mut fixture = fixture();
    let ingested = ingest(&mut fixture, "req_ingest", "idem-ingest");
    assert_eq!(
        ingested.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        ingested.envelope.error
    );
    let handle = ingested_handle(&ingested);
    assert!(handle.as_str().starts_with("ev_"));

    // `@audit_recorded`: the result cites the correlation identity, and the store audited
    // the publication the append performed.
    assert!(!ingested.envelope.audit.is_absent());
    assert!(!fixture.daemon.store_audit().is_empty());
    assert_eq!(ingested.envelope.artifacts.len(), 1);
    assert_eq!(ingested.envelope.artifacts[0].kind, "ev");

    let verified = verify(&mut fixture, &handle, "req_verify", "idem-verify");
    assert_eq!(
        verified.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        verified.envelope.error
    );
    let response = verify_response(&verified);
    assert_eq!(response.evidence, handle);
    assert_eq!(response.status, EvidenceStatus::Observed);
    assert_eq!(response.evidence_kind, EvidenceKind::ProductionObservation);
    assert_eq!(response.checker, DEFAULT_SERVICE);
}

// --- INV-004 ------------------------------------------------------------------------------

#[test]
fn a_producers_append_lands_at_the_lattices_bottom() {
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));
    let appended = node(&fixture, &handle);

    assert_eq!(appended.status(), ClaimStatus::BOTTOM);
    assert_eq!(appended.status(), ClaimStatus::Proposed);
    assert_eq!(
        appended.history.len(),
        1,
        "an append writes exactly one status"
    );
    // The producer's own write names no service identity, which is why the four
    // assurance-bearing statuses are unreachable from it: their schema conditional
    // requires one.
    assert_eq!(appended.service_identity(), None);
    assert_eq!(appended.history[0].validation_basis, None);
    // `provenance.actor` comes from the admitted grant, not from the request body.
    assert_eq!(appended.producer, who("agent:observer"));
}

#[test]
fn the_producers_request_body_has_no_field_that_could_name_a_status() {
    // The structural half of the claim above: `observe.ingest`'s declared request is two
    // fields, and neither is a status, a confidence, or a service identity. A caller
    // therefore cannot *express* a promoted append — the refusal is the wire's shape rather
    // than a validation a handler performs.
    let spec = registry::operation("observe.ingest").expect("the registry declares it");
    let fields: Vec<&str> = spec.request.fields.iter().map(|field| field.name).collect();
    assert_eq!(fields, vec!["trace", "instrumentation_profile"]);

    // And the one evidence operation that *can* move a status takes the caller's status
    // only as the compare-and-set guard.
    let verify = registry::operation("evidence.verify").expect("the registry declares it");
    let fields: Vec<&str> = verify
        .request
        .fields
        .iter()
        .map(|field| field.name)
        .collect();
    assert_eq!(fields, vec!["evidence", "expected_status"]);
}

#[test]
fn the_status_written_is_what_the_checker_established_not_what_the_caller_named() {
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));

    // The caller names `proposed` as the status it believes the claim holds — a truthful
    // guard — and the daemon writes `observed`, which is what its own check supports. The
    // guard is never the written value.
    let verified = verify_with(
        &mut fixture,
        &handle,
        Optional::Present(EvidenceStatus::Proposed),
        "req_verify",
        "idem-verify",
    );
    assert_eq!(verified.envelope.status, ResultStatus::Ok);
    assert_eq!(verify_response(&verified).status, EvidenceStatus::Observed);

    let promoted = node(&fixture, &handle);
    assert_eq!(promoted.status(), ClaimStatus::Observed);
    assert_eq!(
        promoted.history.len(),
        2,
        "a promotion appends, never edits"
    );
    // The append's record is untouched: the first write is still the producer's.
    assert_eq!(promoted.history[0].status, ClaimStatus::Proposed);
    assert_eq!(promoted.history[0].service_identity, None);
}

#[test]
fn a_caller_naming_a_promoted_status_as_its_guard_loses_the_compare_and_set() {
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));

    // `validated` is not what the claim holds, and naming it does not make it so: the CAS
    // is decided against the claim's *current* status, so the promotion is refused and
    // nothing is written.
    let lost = verify_with(
        &mut fixture,
        &handle,
        Optional::Present(EvidenceStatus::Validated),
        "req_lie",
        "idem-lie",
    );
    assert_eq!(code(&lost), ErrorCode::StatusConflict);

    let unchanged = node(&fixture, &handle);
    assert_eq!(unchanged.status(), ClaimStatus::Proposed);
    assert_eq!(unchanged.history.len(), 1);
}

#[test]
fn a_verification_service_may_not_promote_a_claim_it_produced_itself() {
    // The self-certification case the wire cannot prevent: every step obeys the protocol,
    // and the producer's identity happens to be the identity the daemon verifies under.
    let mut fixture = fixture();
    let trace = fixture.trace.clone();
    let ingested = ingest_with(
        &mut fixture,
        DEFAULT_SERVICE,
        "cap_service",
        &trace,
        "req_self_ingest",
        "idem-self-ingest",
    );
    assert_eq!(ingested.envelope.status, ResultStatus::Ok);
    let handle = ingested_handle(&ingested);
    assert_eq!(node(&fixture, &handle).producer.as_str(), DEFAULT_SERVICE);

    let refused = verify(&mut fixture, &handle, "req_self_verify", "idem-self-verify");
    assert_eq!(code(&refused), ErrorCode::InsufficientEvidence);
    assert_eq!(
        node(&fixture, &handle).status(),
        ClaimStatus::Proposed,
        "a refused self-certification writes nothing"
    );
    assert_eq!(node(&fixture, &handle).history.len(), 1);
}

#[test]
fn the_same_claim_produced_by_another_identity_is_promoted() {
    // The control for the test above: the refusal is about *who produced the claim*, not
    // about the trace, the caller, or the class of evidence. One byte of the fixture
    // changes — the capability the append ran under — and the promotion succeeds.
    let mut fixture = fixture();
    let trace = fixture.trace.clone();
    let handle = ingested_handle(&ingest_with(
        &mut fixture,
        "agent:second-observer",
        "cap_second",
        &trace,
        "req_ingest",
        "idem-ingest",
    ));
    let verified = verify(&mut fixture, &handle, "req_verify", "idem-verify");
    assert_eq!(
        verified.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        verified.envelope.error
    );
    assert_eq!(node(&fixture, &handle).status(), ClaimStatus::Observed);
}

#[test]
fn the_checker_a_verification_names_is_the_service_and_never_the_caller() {
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));
    let verified = verify(&mut fixture, &handle, "req_verify", "idem-verify");

    let response = verify_response(&verified);
    assert_eq!(response.checker, DEFAULT_SERVICE);
    assert_ne!(
        response.checker, "agent:reader",
        "who asked is not who checked"
    );
    assert_ne!(
        response.checker, "agent:observer",
        "who produced is not who checked"
    );
}

#[test]
fn a_deployments_own_service_identity_is_what_the_refusal_compares_against() {
    // The service identity is a property of the family, not a constant, so a deployment
    // running two checkers gets two answers for the same graph. `agent:observer` produced
    // the node; a service verifying *as* `agent:observer` is self-certifying even though
    // the identity is not the default one.
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));

    let mut impersonating = Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Present(traced(&[DataGrant::ProductionTrace])),
            ),
            None,
        )
        .capability(
            grant(
                "cap_reader",
                "agent:reader",
                AuthorityLevel::Read,
                3,
                Optional::Absent,
            ),
            Some(cap("cap_root")),
        )
        .now(now())
        .family(EvidenceFamily::verifying_as(who("agent:observer")))
        .build();
    let appended = node(&fixture, &handle);
    impersonating
        .state_mut()
        .append_evidence(handle.clone(), appended);

    let refused = impersonating.dispatch(&OperationRequest {
        envelope: started(
            envelope(
                "evidence.verify",
                "agent:reader",
                "cap_reader",
                "req_verify",
            ),
            "idem-verify",
        ),
        arguments: Arguments::EvidenceVerify(EvidenceVerifyRequest {
            evidence: handle.clone(),
            expected_status: Optional::Absent,
        }),
    });
    assert_eq!(code(&refused), ErrorCode::InsufficientEvidence);
}

#[test]
fn a_class_whose_independent_checker_has_not_shipped_is_never_promoted() {
    // `proved` needs the Lean kernel, which is out of this process, and `sampled`/`bounded`
    // name producing engines this daemon does not run. A node offering one of those classes
    // is refused rather than promoted on the strength of a check that did not run — which is
    // INV-004 one level below authority.
    //
    // The class this test used to carry was `certificate`, and bn-dtg61 moved it: that lane
    // now reaches a checker (`continuum-certificate`), so asserting it is unreachable would
    // be asserting something false. `inductive` is one of the ten classes still without a
    // checker here — the thirteen `EvidenceKind` members less `example`,
    // `production-observation` and `certificate` — and the claim the test makes is unchanged.
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));
    let mut inductive = node(&fixture, &handle);
    inductive.evidence_kind = Some(EvidenceKind::Inductive);
    let inductive_handle =
        EvidenceHandle::new("ev_inductive-fixture").expect("a well-formed handle");
    fixture
        .daemon
        .state_mut()
        .append_evidence(inductive_handle.clone(), inductive);

    let refused = verify(&mut fixture, &inductive_handle, "req_cert", "idem-cert");
    assert_eq!(code(&refused), ErrorCode::InsufficientEvidence);
    assert_eq!(
        node(&fixture, &inductive_handle).status(),
        ClaimStatus::Proposed
    );
}

// --- the certificate lane (bn-dtg61) ------------------------------------------------------
//
// `evidence.verify` over a certificate-class node hands the referenced bytes to
// `continuum-certificate`, which routes them by their leading magic to the
// `continuum-kernel-*` crate that owns the format and returns that kernel's own verdict.
// Four outcomes come back — `Verified`, `Rejected`, `Unsupported`, and a routing failure —
// and the tests below hold all four to reaching the caller as four different wire answers.
//
// Every certificate here is a *real* one: `continuum-engine-reference` explores Die Hard and
// emits the `CONTCERT` wire form, exactly as `certificate_kernel_differential.rs` does. The
// engine and the kernel share no code, so what the daemon relays is an agreement between two
// independent implementations rather than a fixture agreeing with itself.

/// The instrumentation profile a certificate node is filed under.
const CERTIFICATE_PROFILE: &str = "continuum-engine-reference/finite-closure";

/// A real `CONTCERT` artifact: Die Hard explored, closed, and emitted as wire bytes.
fn certificate_bytes() -> Vec<u8> {
    use continuum_engine_reference::bfs::{self, Bounds};
    use continuum_engine_reference::certificate::{self, ClaimEnvelope, ClosedSet, PRODUCER};
    use continuum_engine_reference::diehard;

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

/// Stage `bytes` and file a certificate-class node over them, under the identity it derives.
///
/// Filed under `node_identity(commitment, profile)` deliberately: steps 3a and 3b of the
/// daemon's check run on this lane too, so a node filed anywhere else would be refused for
/// the misfiling and never reach a kernel at all.
fn certificate_node(fixture: &mut Fixture, path: &str, bytes: Vec<u8>) -> EvidenceHandle {
    let artifact = fixture
        .daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new(path).expect("a workspace path"),
            bytes,
        )
        .expect("staging names its content");
    let handle = evidence::node_identity(fixture.daemon.services(), &artifact, CERTIFICATE_PROFILE)
        .expect("the identity seam names the node");
    let record = EvidenceNode {
        kind: EvidenceNodeKind::Certificate,
        evidence_kind: Some(EvidenceKind::Certificate),
        labels: Vec::new(),
        claim_id: "claim:die-hard-closure".to_owned(),
        artifact,
        // Not the verification service: INV-004's fourth dimension refuses a producer that
        // verifies its own claim before any byte is read, and that refusal is tested above.
        producer: who("agent:observer"),
        tool: CERTIFICATE_PROFILE.to_owned(),
        created_at: now(),
        inputs: Vec::new(),
        idempotency_key: "idem-certificate-append".to_owned(),
        history: vec![StatusWrite {
            status: ClaimStatus::BOTTOM,
            service_identity: None,
            validation_basis: None,
            inconclusive_reason: None,
        }],
        redaction: None,
        // Appended out of band: no store publication, so no receipt-tied name (bn-283p6).
        publication: None,
    };
    fixture
        .daemon
        .state_mut()
        .append_evidence(handle.clone(), record);
    handle
}

/// The nine dimensions, keyed by name, so a test can name the one it means.
fn produced_engine(outcome: &OperationOutcome, dimension: &str) -> Option<String> {
    let assurance = match &outcome.envelope.assurance {
        Optional::Present(envelope) => envelope,
        Optional::Absent => panic!("a semantic verdict carries the nine-dimension envelope"),
    };
    let selected = match dimension {
        "values" => &assurance.values,
        "proof_status" => &assurance.proof_status,
        other => panic!("no such dimension: {other}"),
    };
    match selected {
        EnvelopeDimension::Produced(value) => Some(value.engine.clone()),
        EnvelopeDimension::Unsupported(_) => None,
    }
}

fn unsupported_reason(outcome: &OperationOutcome, dimension: &str) -> Option<String> {
    let assurance = match &outcome.envelope.assurance {
        Optional::Present(envelope) => envelope,
        Optional::Absent => panic!("a semantic verdict carries the nine-dimension envelope"),
    };
    let selected = match dimension {
        "proof_status" => &assurance.proof_status,
        other => panic!("no such dimension: {other}"),
    };
    match selected {
        EnvelopeDimension::Unsupported(value) => Some(value.reason.clone()),
        EnvelopeDimension::Produced(_) => None,
    }
}

#[test]
fn a_real_certificate_is_validated_by_the_kernel_that_owns_its_family() {
    let mut fixture = fixture();
    let bytes = certificate_bytes();

    // What the composition says about these bytes, read here so the daemon's answer is held
    // against the checker's rather than against this test's expectation of it.
    assert!(matches!(
        continuum_certificate::check_certificate(&bytes),
        Outcome::Checked(KernelVerdict::Core(
            continuum_kernel_core::Verdict::Verified(_)
        ))
    ));

    let handle = certificate_node(&mut fixture, "certs/die-hard.cert", bytes);
    let verified = verify(&mut fixture, &handle, "req_cert", "idem-cert");
    assert_eq!(
        verified.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        verified.envelope.error
    );

    // The status the checker established, and the basis it rests on. `validated` is
    // unreachable in this daemon by any other path.
    let response = verify_response(&verified);
    assert_eq!(response.status, EvidenceStatus::Validated);
    assert_eq!(response.evidence_kind, EvidenceKind::Certificate);
    assert_eq!(response.validation_basis, "checked-certificate");
    assert_eq!(node(&fixture, &handle).status(), ClaimStatus::Validated);

    // The written history records the basis, which the node schema requires of `validated`.
    let written = node(&fixture, &handle);
    let last = written.history.last().expect("history is never empty");
    assert_eq!(last.status, ClaimStatus::Validated);
    assert_eq!(last.service_identity.as_deref(), Some(DEFAULT_SERVICE));
    assert_eq!(
        last.validation_basis,
        Some(continuum_value::assurance::ValidationBasis::CheckedCertificate)
    );

    match &verified.envelope.verdict {
        Nullable::Value(Verdict::Semantic(value)) => {
            assert_eq!(value.verdict, SemanticVerdict::Established);
            assert_eq!(value.inconclusive_reason, Optional::Absent);
            assert_eq!(value.assurance_class, AssuranceClass::Validated);
        }
        other => panic!("expected a semantic verdict, got {other:?}"),
    }

    // The engine credited with the proof obligation is the kernel, not this daemon. A
    // service that named itself here would be claiming a check it routed rather than ran.
    assert_eq!(
        produced_engine(&verified, "proof_status").as_deref(),
        Some("continuum-kernel-core")
    );
    assert_eq!(
        produced_engine(&verified, "values").as_deref(),
        Some(DEFAULT_SERVICE),
        "the reference re-derivation is still the daemon's own"
    );
}

#[test]
fn a_mutated_certificate_is_rejected_because_the_kernel_rejected_it() {
    let mut fixture = fixture();
    // One trailing byte. The magic is untouched, so routing succeeds and a kernel really
    // does answer — which is what makes this a rejection rather than a routing failure.
    // The kernel's own answer over the same bytes is captured, not just matched: the F19
    // assertions below hold the daemon to *relaying* the kernel's reason rather than to
    // agreeing with this test's guess about it.
    let mut mutated = certificate_bytes();
    mutated.push(0x00);
    let (kernel_reason, kernel_field) = match continuum_certificate::check_certificate(&mutated) {
        Outcome::Checked(KernelVerdict::Core(continuum_kernel_core::Verdict::Rejected(
            rejection,
        ))) => (
            rejection.reason(),
            rejection
                .field()
                .map(continuum_kernel_core::verdict::Field::as_str),
        ),
        other => panic!("expected the core kernel's own rejection, got {other:?}"),
    };

    let handle = certificate_node(&mut fixture, "certs/mutated.cert", mutated);
    let rejected = verify(&mut fixture, &handle, "req_mutated", "idem-mutated");
    assert_eq!(code(&rejected), ErrorCode::CertificateRejected);
    // The detail names the checker that spoke, so the answer is traceable to a kernel rather
    // than to this daemon's opinion of the bytes.
    let error = rejected
        .envelope
        .error
        .value()
        .expect("an error result carries one");
    assert!(
        error.detail.contains("continuum-kernel-core"),
        "the rejection names the trusted checker that reached it: {}",
        error.detail
    );
    assert!(!error.retryable);
    // The kernel's reason WITHIN `Rejected` travels too, typed (RFC 0026 F19, protocol
    // 3.4, bn-3jrtz): the outcome carries the declared `CertificateRejection` value, its
    // tokens byte-equal to the ones the kernel itself spelled over the same bytes. The
    // envelope's own `data` field still reads absent at this layer for the same reason
    // `payload` reads null — it is `Opaque`, bytes of the negotiated encoding, and the
    // transport is where it becomes real (`the_kernels_rejection_reason_reaches_the_client_frame_typed`
    // holds that half).
    match &rejected.data {
        ErrorData::CertificateRejection(data) => {
            assert_eq!(
                data.checker, "continuum-kernel-core",
                "the data names the kernel whose verdict is relayed"
            );
            assert_eq!(
                data.reason, kernel_reason,
                "the reason token is the kernel's own, relayed verbatim"
            );
            assert_eq!(
                data.field.value().map(String::as_str),
                kernel_field,
                "the field token travels exactly when the kernel's rejection names one"
            );
        }
        ErrorData::None => panic!("a CertificateRejected outcome carries its declared data"),
    }
    assert!(
        error.data.is_absent(),
        "the envelope's `data` is encoded by the transport, not by the dispatch"
    );
    // Nothing is promoted, and the refusal is not the identity re-derivation's: the node is
    // filed under the identity it derives and its bytes hash to the commitment it names.
    assert_eq!(node(&fixture, &handle).status(), ClaimStatus::Proposed);

    // The same node with a *green* certificate verifies, so the refusal is about the bytes
    // and about nothing else.
    let green = certificate_node(&mut fixture, "certs/green.cert", certificate_bytes());
    let verified = verify(&mut fixture, &green, "req_green", "idem-green");
    assert_eq!(verified.envelope.status, ResultStatus::Ok);
}

/// The client-visible half of F19: a real kernel rejection travels to the *frame* a
/// client decodes, as the declared `Error.data` shape (RFC 0026 F19, protocol 3.4,
/// bn-3jrtz).
///
/// The daemon-layer test above holds the typed value on the outcome; this one holds the
/// wire. The same mutated certificate goes through [`Server::answer`] — the negotiated
/// encoding, the same splice that fills `payload` — and the frame that comes back carries
/// `error.data` present, decodable through the code's declared shape into the kernel's
/// own tokens. Before this bundle the field was absent daemon-wide, so this test fails
/// without the shape: that is the anti-vacuity the rider owes.
#[test]
fn the_kernels_rejection_reason_reaches_the_client_frame_typed() {
    let mut fixture = fixture();
    let mut mutated = certificate_bytes();
    mutated.push(0x00);
    let kernel_reason = match continuum_certificate::check_certificate(&mutated) {
        Outcome::Checked(KernelVerdict::Core(continuum_kernel_core::Verdict::Rejected(
            rejection,
        ))) => rejection.reason(),
        other => panic!("expected the core kernel's own rejection, got {other:?}"),
    };
    let handle = certificate_node(&mut fixture, "certs/mutated-frame.cert", mutated);

    // The same daemon, behind the transport a client actually talks to.
    let mut server = Server::new(fixture.daemon, negotiated());
    let request = encode_request(
        &started(
            envelope("evidence.verify", "agent:reader", "cap_reader", "req_frame"),
            "idem-frame",
        ),
        &Arguments::EvidenceVerify(EvidenceVerifyRequest {
            evidence: handle,
            expected_status: Optional::Absent,
        }),
    )
    .expect("the request encodes");
    let answer = server.answer(&request).expect("the daemon answers");
    let (result, payload) =
        decode_result("evidence.verify", &answer).expect("the client decodes the frame");

    assert_eq!(result.status, ResultStatus::Error);
    assert_eq!(
        payload,
        Payload::None,
        "`payload` is null on `status = error`"
    );
    let error = result.error.value().expect("an error result carries one");
    assert_eq!(error.code, ErrorCode::CertificateRejected);
    let opaque = error
        .data
        .value()
        .expect("the frame carries the declared `Error.data` (RFC 0026 F19)");
    // Resolved the way `rule encoding.opaque_payloads` says: by the carrying object's own
    // `code`, a function of the message alone.
    match codec::operations::decode_error_data(error.code, opaque)
        .expect("the data decodes through the shape its code declares")
    {
        ErrorData::CertificateRejection(data) => {
            assert_eq!(data.checker, "continuum-kernel-core");
            assert_eq!(
                data.reason, kernel_reason,
                "the client reads the kernel's own reason token, relayed verbatim"
            );
        }
        ErrorData::None => panic!("`CertificateRejected` declares a data shape"),
    }
}

#[test]
fn a_certificate_naming_a_contract_the_kernel_does_not_implement_is_typed_inconclusive() {
    let mut fixture = fixture();
    // The wire epoch. A certificate may be perfectly valid under a contract this build does
    // not implement, so the kernel answers `Unsupported` rather than `Rejected` — and the
    // daemon must not weaken that into "invalid".
    let mut other_epoch = certificate_bytes();
    *other_epoch
        .get_mut(9)
        .expect("the epoch is inside the header") = 2;
    assert!(matches!(
        continuum_certificate::check_certificate(&other_epoch),
        Outcome::Checked(KernelVerdict::Core(
            continuum_kernel_core::Verdict::Unsupported(_)
        ))
    ));

    let handle = certificate_node(&mut fixture, "certs/other-epoch.cert", other_epoch);
    let answered = verify(&mut fixture, &handle, "req_epoch", "idem-epoch");

    // A result, not an error: `verdict` is null on `status = error`, and INV-008's typed
    // reason has no other channel on the wire.
    assert_eq!(
        answered.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        answered.envelope.error
    );
    match &answered.envelope.verdict {
        Nullable::Value(Verdict::Semantic(value)) => {
            assert_eq!(value.verdict, SemanticVerdict::Inconclusive);
            assert_eq!(
                value.inconclusive_reason,
                Optional::Present(InconclusiveReason::Unsupported),
                "INV-008: inconclusive is never untyped, and this reason is not \
                 `InsufficientTelemetry`, which is the redaction lane's"
            );
        }
        other => panic!("expected a semantic verdict, got {other:?}"),
    }
    // Nothing is promoted: an artifact naming a contract its own checker does not implement
    // supports no status.
    assert_eq!(node(&fixture, &handle).status(), ClaimStatus::Proposed);
    assert_eq!(verify_response(&answered).status, EvidenceStatus::Proposed);
    // …and the basis is the weaker of the two the wire admits, because no certificate was
    // checked (plan §11.4: "the two never render identically").
    assert_eq!(
        verify_response(&answered).validation_basis,
        "trusted-solver"
    );
    // The dimension that did not move names the checker whose surface it was.
    assert_eq!(
        unsupported_reason(&answered, "proof_status").as_deref(),
        Some("feature-unsupported-by-continuum-kernel-core")
    );
}

#[test]
fn bytes_no_checker_owns_are_typed_as_unroutable_and_never_as_a_verdict() {
    // The distinction the composition exists to protect: an unrecognised magic could be a
    // corrupted `CONTCERT`, a family a later kernel will own, or a JPEG, and *nothing
    // decoded it*. `CertificateRejected` would call a possibly-valid artifact invalid;
    // `inconclusive` with a semantic reason would credit a checker that never ran.
    for (index, bytes) in [
        b"NOTMAGIC and then some certificate-shaped bytes".to_vec(),
        b"CONT".to_vec(),
    ]
    .into_iter()
    .enumerate()
    {
        let mut fixture = fixture();
        assert!(matches!(
            continuum_certificate::check_certificate(&bytes),
            Outcome::Unroutable(_)
        ));

        let handle = certificate_node(&mut fixture, "certs/unroutable.bin", bytes);
        let refused = verify(&mut fixture, &handle, "req_unroutable", "idem-unroutable");
        assert_eq!(
            code(&refused),
            ErrorCode::EpochUnsupported,
            "case {index}: a routing failure is neither a rejection nor an unsupported \
             feature"
        );
        assert_ne!(code(&refused), ErrorCode::CertificateRejected);
        assert_eq!(node(&fixture, &handle).status(), ClaimStatus::Proposed);
        // `verdict` is null on an error, so no semantic claim rides along with it.
        assert_eq!(refused.envelope.verdict, Nullable::Null);
    }
}

#[test]
fn the_two_routing_faults_are_one_code_and_two_details() {
    // `TooShortForMagic` and `UnknownFamily` are one fact to a caller — no member of the
    // trusted checking base owns these bytes — and the recovery is identical, so they share
    // a code. They are still two different things about the artifact, so they do not share a
    // detail.
    let mut fixture = fixture();
    let short = certificate_node(&mut fixture, "certs/short.bin", b"CONT".to_vec());
    let unknown = certificate_node(
        &mut fixture,
        "certs/unknown.bin",
        b"NOTMAGIC and then some more".to_vec(),
    );
    let short = verify(&mut fixture, &short, "req_short", "idem-short");
    let unknown = verify(&mut fixture, &unknown, "req_unknown", "idem-unknown");
    assert_eq!(code(&short), code(&unknown));
    assert_ne!(
        short
            .envelope
            .error
            .value()
            .expect("an error result carries one")
            .detail,
        unknown
            .envelope
            .error
            .value()
            .expect("an error result carries one")
            .detail
    );
}

#[test]
fn a_certificate_class_node_whose_artifact_is_absent_is_still_insufficient_evidence() {
    // The arm the certificate lane must not swallow. A node the daemon holds no bytes for
    // reaches no kernel at all, and answering `EpochUnsupported` would say something about
    // an artifact nobody has.
    let mut fixture = fixture();
    let handle = certificate_node(&mut fixture, "certs/present.cert", certificate_bytes());
    let mut dangling = node(&fixture, &handle);
    dangling.artifact = Commitment::new("ws_nothing-is-staged-here");
    let dangling_handle =
        EvidenceHandle::new("ev_dangling-certificate").expect("a well-formed handle");
    fixture
        .daemon
        .state_mut()
        .append_evidence(dangling_handle.clone(), dangling);

    let refused = verify(&mut fixture, &dangling_handle, "req_absent", "idem-absent");
    assert_eq!(code(&refused), ErrorCode::InsufficientEvidence);
    assert_eq!(
        node(&fixture, &dangling_handle).status(),
        ClaimStatus::Proposed
    );
}

#[test]
fn verifying_a_certificate_appends_no_edge_of_its_own() {
    // bn-3sypm's recorded negative, held live against the lane that could most plausibly
    // break it: a successful certificate check is exactly the thing a `CHECKED_BY` edge
    // records, and `evidence.verify` still does not append one. RFC 0038 keeps the two
    // apart — an edge is evidence that a check ran, and what it licenses is `evidence.verify`'s
    // separate decision — so a verify that minted its own edge would be a checker recording
    // its own check with no `evidence.link` call and no receipt to point at.
    let mut fixture = fixture();
    let handle = certificate_node(&mut fixture, "certs/die-hard.cert", certificate_bytes());
    assert_eq!(fixture.daemon.state().evidence_edges().count(), 0);
    let nodes_before = fixture.daemon.state().evidence_nodes().count();

    let verified = verify(&mut fixture, &handle, "req_no_edge", "idem-no-edge");
    assert_eq!(verified.envelope.status, ResultStatus::Ok);
    assert_eq!(node(&fixture, &handle).status(), ClaimStatus::Validated);

    assert_eq!(
        fixture.daemon.state().evidence_edges().count(),
        0,
        "evidence.verify appends no evidence-graph edge"
    );
    assert_eq!(
        fixture.daemon.state().evidence_nodes().count(),
        nodes_before,
        "and no receipt node either: nothing is created by a check that only reads"
    );
    // The one delta it does commit is the status transition it just performed.
    let kinds: Vec<EvidenceEventKind> = fixture
        .daemon
        .state()
        .evidence_events()
        .iter()
        .map(|event| event.kind)
        .collect();
    assert!(!kinds.contains(&EvidenceEventKind::EdgePublished));
    assert!(kinds.contains(&EvidenceEventKind::StatusTransition));
    // The response carries no artifact: `evidence.verify` publishes nothing.
    assert!(verified.envelope.artifacts.is_empty());
}

#[test]
fn a_node_whose_reference_does_not_resolve_is_refused_rather_than_believed() {
    // RFC 0038's whiteboard compiler "rejects nonexistent references"; this is that rule
    // where the graph is written. The daemon does not take the producer's word that the
    // identity a node names is content it holds.
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));

    let mut dangling = node(&fixture, &handle);
    dangling.artifact = Commitment::new("ws_nothing-is-staged-here");
    let dangling_handle = EvidenceHandle::new("ev_dangling").expect("a well-formed handle");
    fixture
        .daemon
        .state_mut()
        .append_evidence(dangling_handle.clone(), dangling);

    let refused = verify(
        &mut fixture,
        &dangling_handle,
        "req_dangling",
        "idem-dangling",
    );
    assert_eq!(code(&refused), ErrorCode::InsufficientEvidence);
    assert_eq!(
        node(&fixture, &dangling_handle).status(),
        ClaimStatus::Proposed
    );
}

#[test]
fn a_node_filed_under_an_identity_it_does_not_derive_is_rejected() {
    // The half of the check that makes it a check rather than a formality. A node's identity
    // is a function of what it is *about*, so a node filed under an identity it does not
    // derive is claiming to be about something other than what it is — and the daemon
    // re-derives instead of believing it.
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));

    // Correct in every other respect: the class is one this daemon checks, and the reference
    // resolves. Only the identity it is filed under is someone else's.
    let honest = node(&fixture, &handle);
    let misfiled =
        EvidenceHandle::new("ev_not-the-identity-this-node-derives").expect("a well-formed handle");
    fixture
        .daemon
        .state_mut()
        .append_evidence(misfiled.clone(), honest);

    let rejected = verify(&mut fixture, &misfiled, "req_misfiled", "idem-misfiled");
    assert_eq!(code(&rejected), ErrorCode::CertificateRejected);
    assert_eq!(node(&fixture, &misfiled).status(), ClaimStatus::Proposed);

    // The same node under the identity it *does* derive verifies, so the refusal is about
    // the identity and about nothing else.
    let accepted = verify(&mut fixture, &handle, "req_filed", "idem-filed");
    assert_eq!(
        accepted.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        accepted.envelope.error
    );
}

#[test]
fn a_node_that_changed_what_it_is_about_no_longer_derives_its_own_identity() {
    // The same check from the other direction: leave the node where it is and change what it
    // references. An identity that is a function of the content cannot follow the content
    // silently, which is what makes tampering visible.
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));

    let mut retargeted = node(&fixture, &handle);
    // A real, staged, re-derivable referent — just not the one this node's identity commits
    // to.
    retargeted.artifact = fixture.other.clone();
    let retargeted_handle = EvidenceHandle::new("ev_retargeted").expect("a well-formed handle");
    fixture
        .daemon
        .state_mut()
        .append_evidence(retargeted_handle.clone(), retargeted);

    let rejected = verify(
        &mut fixture,
        &retargeted_handle,
        "req_retarget",
        "idem-retarget",
    );
    assert_eq!(code(&rejected), ErrorCode::CertificateRejected);
}

#[test]
fn a_promotion_of_a_claim_that_does_not_exist_is_byte_identical_to_one_that_does() {
    // RFC 0027 X2: "a read of an artifact that does not exist and a read of one that exists
    // but is out of scope return byte-identical envelopes". The two daemons below differ in
    // exactly one thing — whether the graph holds the node — and are sent the same request.
    let mut holding = fixture();
    let handle = ingested_handle(&ingest(&mut holding, "req_ingest", "idem-ingest"));
    let mut empty = fixture();

    // The holding daemon's reader is scoped away from the node by presenting a capability
    // whose actor does not match, which admission refuses; the empty daemon simply does not
    // hold it.
    let request = OperationRequest {
        envelope: started(
            envelope("evidence.verify", "agent:reader", "cap_reader", "req_probe"),
            "idem-probe",
        ),
        arguments: Arguments::EvidenceVerify(EvidenceVerifyRequest {
            evidence: EvidenceHandle::new("ev_never-appended").expect("handle"),
            expected_status: Optional::Absent,
        }),
    };
    let absent_from_holding = holding.daemon.dispatch(&request);
    let absent_from_empty = empty.daemon.dispatch(&request);
    assert_eq!(absent_from_holding, absent_from_empty);
    assert_eq!(code(&absent_from_holding), ErrorCode::CapabilityDenied);

    // And the same probe against a handle that *does* exist in one daemon and not the other
    // is the same envelope again, because the existing one is out of the caller's scope.
    let existing = OperationRequest {
        envelope: envelope(
            "evidence.get",
            "agent:escalating",
            "cap_escalating",
            "req_probe2",
        ),
        arguments: Arguments::EvidenceGet(EvidenceGetRequest {
            evidence: handle.clone(),
            inline: Optional::Absent,
        }),
    };
    let denied_present = holding.daemon.dispatch(&existing);
    let denied_absent = empty.daemon.dispatch(&existing);
    assert_eq!(denied_present, denied_absent);
    assert_eq!(code(&denied_present), ErrorCode::CapabilityDenied);
}

// --- append-only ---------------------------------------------------------------------------

#[test]
fn re_ingesting_one_trace_converges_on_the_original_node_identity() {
    let mut fixture = fixture();
    let first = ingested_handle(&ingest(&mut fixture, "req_first", "idem-first"));
    // A different request identity and a different idempotency key, so the ledger's replay
    // path is not what makes the two agree: the identity is derived from the content.
    let second = ingested_handle(&ingest(&mut fixture, "req_second", "idem-second"));
    assert_eq!(first, second);
    assert_eq!(
        fixture.daemon.state().evidence_nodes().count(),
        1,
        "a re-ingest appends no second node"
    );
    // And exactly one `node_published` delta was committed.
    let published = fixture
        .daemon
        .state()
        .evidence_events()
        .iter()
        .filter(|event| event.kind == EvidenceEventKind::NodePublished)
        .count();
    assert_eq!(published, 1);
}

#[test]
fn a_re_ingest_after_a_promotion_does_not_lower_the_claim() {
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_first", "idem-first"));
    verify(&mut fixture, &handle, "req_verify", "idem-verify");
    assert_eq!(node(&fixture, &handle).status(), ClaimStatus::Observed);

    // The producer appends again. An append that overwrote would silently regress a
    // promoted claim to `proposed`, which is the failure the append-only model exists to
    // prevent.
    let again = ingest(&mut fixture, "req_again", "idem-again");
    assert_eq!(again.envelope.status, ResultStatus::Ok);
    assert_eq!(ingested_handle(&again), handle);

    let after = node(&fixture, &handle);
    assert_eq!(after.status(), ClaimStatus::Observed);
    assert_eq!(after.history.len(), 2);
}

#[test]
fn the_status_history_grows_and_never_regresses() {
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));
    assert_eq!(
        node(&fixture, &handle).status_history(),
        vec![ClaimStatus::Proposed]
    );

    verify(&mut fixture, &handle, "req_verify", "idem-verify");
    // A second verification is a reassertion — equal statuses — which the lattice permits
    // "because a replayed idempotency key must return the original node identity rather
    // than an error".
    let again = verify(&mut fixture, &handle, "req_again", "idem-again");
    assert_eq!(again.envelope.status, ResultStatus::Ok);
    assert_eq!(verify_response(&again).status, EvidenceStatus::Observed);

    let history = node(&fixture, &handle).status_history();
    assert_eq!(
        history,
        vec![
            ClaimStatus::Proposed,
            ClaimStatus::Observed,
            ClaimStatus::Observed
        ]
    );
    // Checked by the crate that owns the order, not by this file's reading of it.
    assert!(verify_promotion_history(&history).is_ok());
}

#[test]
fn no_operation_in_either_family_removes_or_edits_an_appended_node() {
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));
    let before = node(&fixture, &handle);

    // Every operation both families serve, run against the node. The status-advancing one
    // is deliberately excluded: it is the *only* write, and it appends.
    // The outcomes are deliberately unread: what this test asserts is what the *graph*
    // looks like afterwards.
    get(&mut fixture, &handle, "req_get");
    query(&mut fixture, empty_query(), "req_query");
    let _ = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope(
            "evidence.subscribe",
            "agent:reader",
            "cap_reader",
            "req_sub",
        ),
        arguments: Arguments::EvidenceSubscribe(EvidenceSubscribeRequest {
            scope: empty_query(),
        }),
    });
    let _ = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("observe.classify", "agent:reader", "cap_reader", "req_cls"),
        arguments: Arguments::ObserveClassify(ObserveClassifyRequest {
            evidence: handle.clone(),
        }),
    });
    let _ = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("observe.result", "agent:reader", "cap_reader", "req_res"),
        arguments: Arguments::ObserveResult(ObserveResultRequest {
            evidence: handle.clone(),
        }),
    });
    ingest(&mut fixture, "req_reingest", "idem-reingest");

    assert_eq!(node(&fixture, &handle), before);
    assert_eq!(fixture.daemon.state().evidence_nodes().count(), 1);
}

// --- redaction -----------------------------------------------------------------------------

#[test]
fn a_redacted_reference_reads_back_as_the_typed_stub_and_names_its_omission() {
    for reason in [
        RedactionReason::Summarized,
        RedactionReason::Purged,
        RedactionReason::Lost,
    ] {
        let mut fixture = fixture();
        let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));
        let commitment = fixture.trace.clone();
        fixture
            .daemon
            .state_mut()
            .redact_evidence(&handle, redaction(reason, &commitment));

        let read = get(&mut fixture, &handle, "req_get");
        assert_eq!(read.envelope.status, ResultStatus::Ok);
        let response = match &read.payload {
            Payload::EvidenceGet(response) => response,
            other => panic!("expected an evidence.get payload, got {other:?}"),
        };
        let stub = match &response.redacted {
            Optional::Present(stub) => stub,
            Optional::Absent => panic!("a redacted reference is a typed stub, never an absence"),
        };
        assert!(stub.redacted);
        assert_eq!(stub.reason, reason);
        assert_eq!(stub.commitment, commitment);
        assert_eq!(stub.original_class, "ev");
        // "a client MUST be able to tell 'withheld' from 'absent' structurally": the node
        // record is still returned beside the stub.
        assert!(matches!(response.node, Nullable::Value(_)));
        // "Every redacted value MUST also appear in the result's `omissions` manifest with
        // reason `redaction` (INV-007)."
        assert!(
            read.envelope
                .omissions
                .iter()
                .any(|omission| omission.reason == OmissionReason::Redaction)
        );
    }
}

#[test]
fn verifying_over_a_redacted_reference_returns_the_structural_result_and_the_redaction() {
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));
    let commitment = fixture.trace.clone();
    fixture
        .daemon
        .state_mut()
        .redact_evidence(&handle, redaction(RedactionReason::Summarized, &commitment));

    let verified = verify(&mut fixture, &handle, "req_verify", "idem-verify");

    // Not a bare failure: the receipt is intact.
    assert_eq!(
        verified.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        verified.envelope.error
    );
    // Not a bare success: the claim is no longer fully supported.
    match &verified.envelope.verdict {
        Nullable::Value(Verdict::Semantic(value)) => {
            assert_eq!(value.verdict, SemanticVerdict::Inconclusive);
            assert_eq!(
                value.inconclusive_reason,
                Optional::Present(InconclusiveReason::InsufficientTelemetry),
                "INV-008: inconclusive is never untyped"
            );
        }
        other => panic!("expected a semantic verdict, got {other:?}"),
    }
    // The redaction travels in the response, which is the field IDL 1.1 added for it.
    assert!(matches!(
        verify_response(&verified).redacted,
        Optional::Present(_)
    ));
    assert!(
        verified
            .envelope
            .omissions
            .iter()
            .any(|omission| omission.reason == OmissionReason::Redaction)
    );
    // Nothing is promoted over content the daemon cannot read.
    assert_eq!(node(&fixture, &handle).status(), ClaimStatus::Proposed);
    // "any claim that required the hidden data MUST downgrade in the `assurance` envelope".
    let assurance = match &verified.envelope.assurance {
        Optional::Present(envelope) => envelope,
        Optional::Absent => panic!("a semantic verdict carries the nine-dimension envelope"),
    };
    for dimension in dimensions(assurance) {
        assert!(
            matches!(dimension, EnvelopeDimension::Unsupported(_)),
            "a redacted input downgrades every dimension"
        );
    }
}

// --- the assurance envelope ----------------------------------------------------------------

/// The nine dimensions of an envelope, in the IDL's declaration order.
fn dimensions(
    envelope: &continuumd::protocol::envelope::AssuranceEnvelope,
) -> [&EnvelopeDimension; 9] {
    [
        &envelope.bounds,
        &envelope.faults,
        &envelope.fairness,
        &envelope.values,
        &envelope.schedules,
        &envelope.memory_model,
        &envelope.observer,
        &envelope.proof_status,
        &envelope.unknowns,
    ]
}

#[test]
fn a_semantic_verdict_carries_all_nine_dimensions_and_a_structural_one_carries_none() {
    let mut fixture = fixture();
    let ingested = ingest(&mut fixture, "req_ingest", "idem-ingest");
    // `observe.ingest` declares a `structural` verdict, and `rule
    // envelope.assurance_required` attaches the envelope to `semantic` and `evaluation`
    // verdicts only. Nine dimensions on a snapshot append would be nine claims nothing
    // established.
    assert!(ingested.envelope.assurance.is_absent());

    let handle = ingested_handle(&ingested);
    let verified = verify(&mut fixture, &handle, "req_verify", "idem-verify");
    let assurance = match &verified.envelope.assurance {
        Optional::Present(envelope) => envelope,
        Optional::Absent => panic!("`rule envelope.assurance_required` obliges the envelope"),
    };
    // "every dimension MUST name a producing engine or carry a typed `Unsupported(reason)`
    // […] A dimension is never silently omitted." The union has two variants and both are
    // meaningful; what the rule forbids is a dimension that says nothing.
    let produced = dimensions(assurance)
        .into_iter()
        .filter(|dimension| matches!(dimension, EnvelopeDimension::Produced(_)))
        .count();
    assert_eq!(
        produced, 1,
        "one dimension is established by the reference re-derivation, and eight are not"
    );
    for dimension in dimensions(assurance) {
        match dimension {
            EnvelopeDimension::Produced(value) => {
                assert_eq!(value.engine, DEFAULT_SERVICE);
                assert!(!value.summary.is_empty());
            }
            EnvelopeDimension::Unsupported(value) => assert!(!value.reason.is_empty()),
        }
    }
}

// --- evidence.get / query / subscribe -------------------------------------------------------

#[test]
fn a_read_returns_the_node_record_the_schema_declares_and_no_edge() {
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));
    let read = get(&mut fixture, &handle, "req_get");
    assert_eq!(read.envelope.status, ResultStatus::Ok);

    let response = match &read.payload {
        Payload::EvidenceGet(response) => response,
        other => panic!("expected an evidence.get payload, got {other:?}"),
    };
    let bytes = match &response.node {
        Nullable::Value(node) => node.as_bytes().to_vec(),
        Nullable::Null => panic!("the node record is not null for a node the graph holds"),
    };
    let text = String::from_utf8(bytes).expect("canonical JSON is UTF-8");
    // The six members `evidence-graph-node.schema.json` marks required, plus the two plan
    // §11.7 write-model members it names.
    for required in [
        "\"schema_id\"",
        "\"schema_epoch\"",
        "\"node_id\"",
        "\"kind\"",
        "\"artifact\"",
        "\"status\"",
        "\"provenance\"",
        "\"claim_id\"",
        "\"idempotency_key\"",
    ] {
        assert!(text.contains(required), "node record is missing {required}");
    }
    assert!(text.contains("\"status\":\"proposed\""));
    assert!(text.contains("\"kind\":\"run\""));
    // `additionalProperties` is false and the schema admits `service_identity` only on the
    // four assurance-bearing statuses, so a `proposed` node must not carry one.
    assert!(!text.contains("\"service_identity\""));
    // The handle named a node, so it does not name an edge. `evidence.link` gave the
    // graph edges at 3.3; the two arms stay exclusive because they are two maps.
    assert_eq!(response.edge, Nullable::Null);
}

#[test]
fn a_read_that_asks_for_inline_content_says_what_it_did_not_return() {
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));
    let read = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("evidence.get", "agent:reader", "cap_reader", "req_get"),
        arguments: Arguments::EvidenceGet(EvidenceGetRequest {
            evidence: handle,
            inline: Optional::Present(true),
        }),
    });
    assert_eq!(read.envelope.status, ResultStatus::Ok);
    assert!(
        read.envelope
            .omissions
            .iter()
            .any(|omission| omission.reason == OmissionReason::Unsupported),
        "INV-007: what was left out is named, never implied"
    );
}

#[test]
fn a_query_filters_by_status_kind_claim_and_roots_and_returns_no_edges() {
    let mut fixture = fixture();
    let trace = fixture.trace.clone();
    let other = fixture.other.clone();
    let first = ingested_handle(&ingest_with(
        &mut fixture,
        "agent:observer",
        "cap_observer",
        &trace,
        "req_a",
        "idem-a",
    ));
    let second = ingested_handle(&ingest_with(
        &mut fixture,
        "agent:observer",
        "cap_observer",
        &other,
        "req_b",
        "idem-b",
    ));
    verify(&mut fixture, &first, "req_verify", "idem-verify");

    let all = query(&mut fixture, empty_query(), "req_all");
    let nodes = |outcome: &OperationOutcome| match &outcome.payload {
        Payload::EvidenceQuery(response) => {
            assert!(response.edges.is_empty(), "this graph holds no edges");
            response.nodes.clone()
        }
        other => panic!("expected an evidence.query payload, got {other:?}"),
    };
    let listed = nodes(&all);
    assert_eq!(listed.len(), 2);
    // Deterministic: the container's key order, which is a function of the keys present and
    // of nothing else (`rule pagination.deterministic`).
    let mut sorted = listed.clone();
    sorted.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    assert_eq!(listed, sorted);

    let by_status = query(
        &mut fixture,
        EvidenceQuery {
            statuses: Optional::Present(vec![EvidenceStatus::Observed]),
            ..empty_query()
        },
        "req_status",
    );
    assert_eq!(nodes(&by_status), vec![first.clone()]);

    let by_kind = query(
        &mut fixture,
        EvidenceQuery {
            node_kinds: Optional::Present(vec![EvidenceNodeKind::Certificate]),
            ..empty_query()
        },
        "req_kind",
    );
    assert!(nodes(&by_kind).is_empty());

    let by_claim = query(
        &mut fixture,
        EvidenceQuery {
            claim_id: Optional::Present(other.as_str().to_owned()),
            ..empty_query()
        },
        "req_claim",
    );
    assert_eq!(nodes(&by_claim), vec![second.clone()]);

    let by_root = query(
        &mut fixture,
        EvidenceQuery {
            roots: Optional::Present(vec![second.clone()]),
            ..empty_query()
        },
        "req_root",
    );
    // With no edge to cross, a root's neighborhood is the root. The traversal
    // `rule evidence.traversal` states is exercised over a graph that has edges, below.
    assert_eq!(nodes(&by_root), vec![second]);
}

// --- `rule evidence.traversal` (IDL 1.8, bn-35l6g) ---------------------------------------
//
// `roots` has read "roots to traverse from" since protocol 3.0 and `max_depth` has been
// declared beside it just as long, but until 3.3 the graph held no edges and the daemon
// served `roots` as bare handle membership with the bound dropped on the floor. 3.3's
// `evidence.link` gave the graph edges; `rule evidence.traversal` says how they are walked,
// and the tests below are what says the daemon walks them that way.

/// A two-edge chain plus one artifact nothing connects to it.
///
/// ```text
///   a --E1--> r1 --E2--> r2          b
/// ```
///
/// `a` is `agent:observer`'s ingest, `r1` is `service:kernel-core`'s receipt for checking it,
/// and `r2` is `service:kernel-smt`'s receipt for checking *that* — a second checker, because
/// `kernel-core` producing `r1` is exactly what INV-004 forbids it from checking. `b` is a
/// second ingest, in the graph and out of every neighborhood the chain has.
struct Chain {
    a: EvidenceHandle,
    r1: EvidenceHandle,
    r2: EvidenceHandle,
    e1: EvidenceHandle,
    e2: EvidenceHandle,
    b: EvidenceHandle,
}

fn linked(outcome: &OperationOutcome) -> (EvidenceHandle, EvidenceHandle) {
    match &outcome.payload {
        Payload::EvidenceLink(response) => (response.edge.clone(), response.receipt.clone()),
        other => panic!("expected an evidence.link payload, got {other:?}"),
    }
}

fn chain(fixture: &mut Fixture) -> Chain {
    let trace = fixture.trace.clone();
    let other = fixture.other.clone();
    let a = ingested_handle(&ingest_with(
        fixture,
        "agent:observer",
        "cap_observer",
        &trace,
        "req_chain_a",
        "idem-chain-a",
    ));
    let b = ingested_handle(&ingest_with(
        fixture,
        "agent:second-observer",
        "cap_second",
        &other,
        "req_chain_b",
        "idem-chain-b",
    ));
    let first = link_under(
        fixture,
        "service:kernel-core",
        "cap_checker",
        &a,
        &other,
        "kernel-core/1",
        "req_chain_e1",
        "idem-chain-e1",
    );
    assert_eq!(
        first.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        first.envelope.error
    );
    let (e1, r1) = linked(&first);
    let second = link_under(
        fixture,
        "service:kernel-smt",
        "cap_checker_smt",
        &r1,
        &other,
        "kernel-smt/1",
        "req_chain_e2",
        "idem-chain-e2",
    );
    assert_eq!(
        second.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        second.envelope.error
    );
    let (e2, r2) = linked(&second);
    Chain {
        a,
        r1,
        r2,
        e1,
        e2,
        b,
    }
}

/// The nodes and edges one query selects, as sorted sets, so an assertion names membership
/// rather than the container's key order (which `rule ordering.deterministic` already holds).
fn answered(outcome: &OperationOutcome) -> (Vec<String>, Vec<String>) {
    match &outcome.payload {
        Payload::EvidenceQuery(response) => {
            let mut nodes: Vec<String> = response
                .nodes
                .iter()
                .map(|handle| handle.as_str().to_owned())
                .collect();
            let mut edges: Vec<String> = response
                .edges
                .iter()
                .map(|handle| handle.as_str().to_owned())
                .collect();
            nodes.sort();
            edges.sort();
            (nodes, edges)
        }
        other => panic!("expected an evidence.query payload, got {other:?}"),
    }
}

fn set(handles: &[&EvidenceHandle]) -> Vec<String> {
    let mut sorted: Vec<String> = handles
        .iter()
        .map(|handle| (*handle).as_str().to_owned())
        .collect();
    sorted.sort();
    sorted
}

fn rooted(roots: &[&EvidenceHandle], depth: Optional<u32>) -> EvidenceQuery {
    EvidenceQuery {
        roots: Optional::Present(roots.iter().map(|handle| (*handle).clone()).collect()),
        max_depth: depth,
        ..empty_query()
    }
}

/// Clause 1: an edge is walked in **either** direction, because an edge's direction is what
/// it asserts and not which way relevance runs.
///
/// `a --CHECKED_BY--> r1` is the only edge between them. Rooting at `a` must reach `r1`
/// (with the arrow) and rooting at `r1` must reach `a` (against it). A forward-only walk
/// would fail the second; a reverse-only walk would fail the first. The RFC's own reason for
/// a graph is the general case — every `SUPPORTS`, `REFUTES`, and `COUNTEREXAMPLE_TO` edge
/// names its claim as `to`, so a forward-only walk from a claim reaches none of the evidence
/// offered to it.
#[test]
fn a_traversal_crosses_an_edge_in_either_direction() {
    let mut fixture = fixture();
    let chain = chain(&mut fixture);

    let forward = query(
        &mut fixture,
        rooted(&[&chain.a], Optional::Present(1)),
        "req_forward",
    );
    let (nodes, edges) = answered(&forward);
    assert_eq!(nodes, set(&[&chain.a, &chain.r1]), "with the arrow");
    assert_eq!(edges, set(&[&chain.e1]));

    let backward = query(
        &mut fixture,
        rooted(&[&chain.r1], Optional::Present(1)),
        "req_backward",
    );
    let (nodes, edges) = answered(&backward);
    assert_eq!(
        nodes,
        set(&[&chain.a, &chain.r1, &chain.r2]),
        "against the arrow, and with the next one"
    );
    assert_eq!(edges, set(&[&chain.e1, &chain.e2]));
}

/// Clause 2 and clause 3, at every boundary the bound has: `0`, each interior value, the
/// value that saturates the graph, and absent.
///
/// `0` is exactly the handle-membership reading a pre-1.8 daemon served for every `roots`,
/// so nothing a client could previously ask became unaskable — it just has to say the bound
/// it always meant.
#[test]
fn max_depth_bounds_the_traversal_in_edges() {
    let mut fixture = fixture();
    let chain = chain(&mut fixture);

    for (bound, want_nodes, want_edges) in [
        (Optional::Present(0), set(&[&chain.a]), Vec::new()),
        (
            Optional::Present(1),
            set(&[&chain.a, &chain.r1]),
            set(&[&chain.e1]),
        ),
        (
            Optional::Present(2),
            set(&[&chain.a, &chain.r1, &chain.r2]),
            set(&[&chain.e1, &chain.e2]),
        ),
        (
            Optional::Present(9),
            set(&[&chain.a, &chain.r1, &chain.r2]),
            set(&[&chain.e1, &chain.e2]),
        ),
        (
            Optional::Absent,
            set(&[&chain.a, &chain.r1, &chain.r2]),
            set(&[&chain.e1, &chain.e2]),
        ),
    ] {
        let label = format!("req_depth_{}", bound.value().copied().unwrap_or(99));
        let outcome = query(&mut fixture, rooted(&[&chain.a], bound), &label);
        let (nodes, edges) = answered(&outcome);
        assert_eq!(nodes, want_nodes, "nodes at {bound:?}");
        assert_eq!(edges, want_edges, "edges at {bound:?}");
        // Whatever the bound, `b` is connected to nothing and is in no neighborhood.
        assert!(
            !nodes.contains(&chain.b.as_str().to_owned()),
            "at {bound:?}"
        );
    }
}

/// Clause 4: an answer names no dangling edge, at every bound.
///
/// This is a property of the depth rule rather than of a filter — an edge takes the *greater*
/// of its endpoints' depths, so an edge within the bound has both endpoints within it. A
/// returned edge whose endpoints the same answer omitted would be a reference the client
/// cannot resolve, and "references must resolve" is the graph's own structural refusal.
#[test]
fn a_rooted_answer_names_no_dangling_edge() {
    let mut fixture = fixture();
    let chain = chain(&mut fixture);
    let endpoints = |fixture: &Fixture, handle: &EvidenceHandle| {
        let edge = fixture
            .daemon
            .state()
            .evidence_edge(handle)
            .expect("the graph holds the edge")
            .clone();
        (edge.from, edge.to)
    };

    for bound in [0_u32, 1, 2, 3] {
        for root in [&chain.a, &chain.r1, &chain.r2, &chain.b] {
            let label = format!("req_closed_{bound}_{}", root.as_str());
            let outcome = query(
                &mut fixture,
                rooted(&[root], Optional::Present(bound)),
                &label,
            );
            let (nodes, edges) = answered(&outcome);
            for edge in &edges {
                let handle = EvidenceHandle::new(edge).expect("a returned handle is well-formed");
                let (from, to) = endpoints(&fixture, &handle);
                for endpoint in [from, to] {
                    assert!(
                        nodes.contains(&endpoint.as_str().to_owned()),
                        "edge {edge} at depth {bound} from {} names {} , which the answer omits",
                        root.as_str(),
                        endpoint.as_str()
                    );
                }
            }
        }
    }
}

/// The negative: an artifact no edge connects to a root is out of the scope, at every bound
/// including an unbounded one. A traversal that reached it would be a traversal of nothing.
#[test]
fn an_artifact_no_edge_connects_to_a_root_is_out_of_scope() {
    let mut fixture = fixture();
    let chain = chain(&mut fixture);

    let unbounded = query(
        &mut fixture,
        rooted(&[&chain.b], Optional::Absent),
        "req_isolated",
    );
    let (nodes, edges) = answered(&unbounded);
    assert_eq!(nodes, set(&[&chain.b]));
    assert!(edges.is_empty(), "no edge is incident to it");

    let from_chain = query(
        &mut fixture,
        rooted(&[&chain.a], Optional::Absent),
        "req_isolated_other_way",
    );
    let (nodes, _) = answered(&from_chain);
    assert!(!nodes.contains(&chain.b.as_str().to_owned()));
}

/// The boundary that is not an artifact: a root the graph does not hold seeds nothing, so it
/// reaches nothing. The alternative — treating an unknown handle as a wildcard, or as an
/// absent clause — would invent a neighborhood around an artifact that does not exist, and
/// would make an unheld handle a probe for the graph's contents.
#[test]
fn a_root_the_graph_does_not_hold_reaches_nothing() {
    let mut fixture = fixture();
    let chain = chain(&mut fixture);
    let absent =
        EvidenceHandle::new("ev_0000000000000000000000000000000000000000000000000000000000000000")
            .expect("a well-formed handle the graph does not hold");

    let outcome = query(
        &mut fixture,
        rooted(&[&absent], Optional::Absent),
        "req_unheld_root",
    );
    let (nodes, edges) = answered(&outcome);
    assert!(nodes.is_empty(), "{nodes:?}");
    assert!(edges.is_empty(), "{edges:?}");

    // And it does not widen a root that *is* held: the clause is a union of neighborhoods.
    let beside = query(
        &mut fixture,
        rooted(&[&absent, &chain.a], Optional::Present(1)),
        "req_unheld_beside",
    );
    let (nodes, _) = answered(&beside);
    assert_eq!(nodes, set(&[&chain.a, &chain.r1]));
}

/// Clause 6: absent or empty `roots` is "the whole graph in scope" — the field's own declared
/// sentence — and `max_depth` then selects nothing away, because there is no root to measure
/// a distance from. A daemon that read an empty list as an empty neighborhood would answer
/// nothing to a query the IDL says answers everything.
#[test]
fn an_empty_roots_list_is_the_whole_graph_and_a_depth_bound_selects_nothing_away() {
    let mut fixture = fixture();
    let chain = chain(&mut fixture);
    let every_node = set(&[&chain.a, &chain.b, &chain.r1, &chain.r2]);
    let every_edge = set(&[&chain.e1, &chain.e2]);

    for (label, scope) in [
        (
            "req_roots_absent",
            EvidenceQuery {
                max_depth: Optional::Present(0),
                ..empty_query()
            },
        ),
        (
            "req_roots_empty",
            EvidenceQuery {
                roots: Optional::Present(Vec::new()),
                max_depth: Optional::Present(0),
                ..empty_query()
            },
        ),
    ] {
        let outcome = query(&mut fixture, scope, label);
        let (nodes, edges) = answered(&outcome);
        assert_eq!(nodes, every_node, "{label}");
        assert_eq!(edges, every_edge, "{label}");
    }
}

/// Clause 2's other half: a root naming an **edge** seeds that edge and both its endpoints at
/// depth 0, so `max_depth = 0` over an edge root answers the whole assertion rather than a
/// handle whose endpoints the answer omits. Half of an assertion is not an assertion.
#[test]
fn a_root_naming_an_edge_seeds_the_edge_and_both_its_endpoints() {
    let mut fixture = fixture();
    let chain = chain(&mut fixture);

    let exact = query(
        &mut fixture,
        rooted(&[&chain.e1], Optional::Present(0)),
        "req_edge_root",
    );
    let (nodes, edges) = answered(&exact);
    assert_eq!(nodes, set(&[&chain.a, &chain.r1]));
    assert_eq!(edges, set(&[&chain.e1]));

    // And one step further reaches what `r1` is adjacent to.
    let wider = query(
        &mut fixture,
        rooted(&[&chain.e1], Optional::Present(1)),
        "req_edge_root_wider",
    );
    let (nodes, edges) = answered(&wider);
    assert_eq!(nodes, set(&[&chain.a, &chain.r1, &chain.r2]));
    assert_eq!(edges, set(&[&chain.e1, &chain.e2]));
}

/// Clause 5: the traversal walks the whole graph and the other clauses filter what it
/// reached — they do not restrict the walk.
///
/// `a` is promoted to `observed`; `r1` and `r2` stay at the bottom of the lattice. A scope
/// rooted at `r2` with `statuses: [observed]` must still answer `a`, two edges away *through*
/// `r1`, which the status clause excludes. A daemon that walked only through matching nodes
/// would answer nothing, and would have made one clause change another's meaning rather than
/// conjoin with it.
#[test]
fn the_traversal_walks_the_whole_graph_and_the_other_clauses_filter_what_it_reached() {
    let mut fixture = fixture();
    let chain = chain(&mut fixture);
    let promoted = verify(&mut fixture, &chain.a, "req_promote", "idem-promote");
    assert_eq!(promoted.envelope.status, ResultStatus::Ok);
    assert_eq!(node(&fixture, &chain.a).status(), ClaimStatus::Observed);
    assert_eq!(node(&fixture, &chain.r1).status(), ClaimStatus::BOTTOM);

    let outcome = query(
        &mut fixture,
        EvidenceQuery {
            statuses: Optional::Present(vec![EvidenceStatus::Observed]),
            ..rooted(&[&chain.r2], Optional::Present(2))
        },
        "req_through_excluded",
    );
    let (nodes, _) = answered(&outcome);
    assert_eq!(
        nodes,
        set(&[&chain.a]),
        "the walk passed through `r1`, which the status clause excludes from the answer"
    );

    // The bound is still real: one edge short of `a`, the same scope answers nothing.
    let short = query(
        &mut fixture,
        EvidenceQuery {
            statuses: Optional::Present(vec![EvidenceStatus::Observed]),
            ..rooted(&[&chain.r2], Optional::Present(1))
        },
        "req_through_excluded_short",
    );
    let (nodes, _) = answered(&short);
    assert!(nodes.is_empty(), "{nodes:?}");
}

/// A subscription's frontier is `evidence.query`'s answer over the identical scope, and one
/// list because one handle class names both halves of the graph.
///
/// Until bn-3080b this operation was refused with `UnsupportedSemanticFeature`, and the
/// refusal was right: nothing delivered an `events` frame, so a frontier would have been the
/// "empty success" `rule errors.unsupported_surface` forbids. `rule subscription.delivery`
/// (IDL 1.7) fixed what a frame is and the transport now writes them, so the operation is
/// served — and this test holds the half that lives at *this* layer, which is that the
/// frontier and a query cannot disagree about what a scope selects. The channel itself is
/// `tests/evidence_subscription.rs`.
#[test]
fn a_subscription_answers_the_scope_its_query_would_answer() {
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));

    let answered = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope(
            "evidence.subscribe",
            "agent:reader",
            "cap_reader",
            "req_sub",
        ),
        arguments: Arguments::EvidenceSubscribe(EvidenceSubscribeRequest {
            scope: empty_query(),
        }),
    });
    assert_eq!(answered.envelope.status, ResultStatus::Ok);
    let Payload::EvidenceSubscribe(body) = &answered.payload else {
        panic!("the subscription answers its declared response body");
    };
    assert_eq!(body.frontier, vec![handle]);

    // The same scope, through the other operation that answers it.
    let frontier = body.frontier.clone();
    let queried = query(&mut fixture, empty_query(), "req_query");
    let Payload::EvidenceQuery(listed) = &queried.payload else {
        panic!("the query answers its declared response body");
    };
    let mut expected = listed.nodes.clone();
    expected.extend(listed.edges.clone());
    assert_eq!(
        frontier, expected,
        "the frontier and the query are one predicate, not two"
    );
}

/// A scope that selects nothing answers an empty frontier — and that is not the empty
/// success `rule errors.unsupported_surface` forbids.
///
/// The two are different facts and the difference is worth pinning: "the channel is not
/// served" is a refusal, while "you are subscribed and nothing is in scope yet" is an
/// answer, and a client acts on them differently — the first by giving up, the second by
/// waiting for the frames `tests/evidence_subscription.rs` shows arriving.
#[test]
fn a_scope_that_selects_nothing_answers_an_empty_frontier_rather_than_a_refusal() {
    let mut fixture = fixture();
    let _ = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));

    let answered = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope(
            "evidence.subscribe",
            "agent:reader",
            "cap_reader",
            "req_sub",
        ),
        arguments: Arguments::EvidenceSubscribe(EvidenceSubscribeRequest {
            scope: EvidenceQuery {
                claim_id: Optional::Present("no-claim-has-this-identity".to_owned()),
                ..empty_query()
            },
        }),
    });
    assert_eq!(answered.envelope.status, ResultStatus::Ok);
    let Payload::EvidenceSubscribe(body) = &answered.payload else {
        panic!("the subscription answers its declared response body");
    };
    assert!(body.frontier.is_empty());
}

#[test]
fn every_committed_delta_is_recorded_for_a_transport_to_drain() {
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));
    verify(&mut fixture, &handle, "req_verify", "idem-verify");

    let events = fixture.daemon.state().evidence_events();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].kind, EvidenceEventKind::NodePublished);
    assert_eq!(events[0].node, Optional::Present(handle.clone()));
    assert_eq!(events[1].kind, EvidenceEventKind::StatusTransition);
    assert_eq!(
        events[1].status,
        Optional::Present(EvidenceStatus::Observed),
        "a delta references a committed artifact, not a hint"
    );
}

#[test]
fn the_two_observe_reads_are_refused_typed_rather_than_degraded() {
    let mut fixture = fixture();
    let handle = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));

    let classify = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("observe.classify", "agent:reader", "cap_reader", "req_cls"),
        arguments: Arguments::ObserveClassify(ObserveClassifyRequest {
            evidence: handle.clone(),
        }),
    });
    assert_eq!(code(&classify), ErrorCode::UnsupportedSemanticFeature);

    let result = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope("observe.result", "agent:reader", "cap_reader", "req_res"),
        arguments: Arguments::ObserveResult(ObserveResultRequest { evidence: handle }),
    });
    assert_eq!(code(&result), ErrorCode::UnsupportedSemanticFeature);
}

// --- capability negotiation -----------------------------------------------------------------

fn limits() -> ServerLimits {
    ServerLimits {
        idempotency_retention_ms: DurationMs::new(86_400_000),
        max_page_size: 100,
        max_result_bytes: ByteCount::new(1_048_576),
        max_concurrent_tasks: 4,
    }
}

fn policy() -> ConnectionPolicy {
    ConnectionPolicy::new(
        vec![ProtocolVersion::new(3, 0), version()],
        ProtocolWindow::new(3),
        ENCODINGS.to_vec(),
        limits(),
        "continuumd-test".to_owned(),
    )
    .features(vec!["evidence-subscriptions".to_owned()])
}

#[test]
fn the_welcome_reports_the_authority_the_daemon_holds_and_not_the_one_the_client_claims() {
    let daemon = daemon();
    let (negotiated, welcome) = daemon
        .welcome(
            &policy(),
            &hello("cap_reader", "agent:reader", version(), version()),
        )
        .expect("the reader's capability is registered");

    assert_eq!(negotiated.protocol_version(), version());
    // `ServerWelcome.grant` is the only authority channel, and it is the *registered*
    // descriptor: the hello carries no level, no scope, and no profile to copy from.
    assert_eq!(welcome.grant.capability, cap("cap_reader"));
    assert_eq!(welcome.grant.level, AuthorityLevel::Read);
    assert_eq!(welcome.grant.actor, who("agent:reader"));
    assert_eq!(welcome.majors_served, vec![3, 2]);
    assert_eq!(welcome.limits, limits());
    assert_eq!(welcome.epochs.protocol, version());
}

#[test]
fn a_client_offering_a_range_is_served_the_highest_common_version_and_never_one_outside_it() {
    let daemon = daemon();
    // The full range: the highest the daemon serves.
    let (negotiated, welcome) = daemon
        .welcome(
            &policy(),
            &hello(
                "cap_reader",
                "agent:reader",
                ProtocolVersion::new(3, 0),
                version(),
            ),
        )
        .expect("3.1 is common");
    assert_eq!(negotiated.protocol_version(), version());
    assert_eq!(welcome.protocol_version, version());

    // A client that only speaks 3.0 is downgraded to 3.0, not upgraded past its range.
    let (downgraded, _) = daemon
        .welcome(
            &policy(),
            &hello(
                "cap_reader",
                "agent:reader",
                ProtocolVersion::new(3, 0),
                ProtocolVersion::new(3, 0),
            ),
        )
        .expect("3.0 is common");
    assert_eq!(downgraded.protocol_version(), ProtocolVersion::new(3, 0));

    // A client whose whole range lies outside the daemon's served set is refused with the
    // version code, and is told the window from the refusal itself — it never received a
    // `ServerWelcome` and has no other channel for it.
    let refused = daemon
        .welcome(
            &policy(),
            &hello(
                "cap_reader",
                "agent:reader",
                ProtocolVersion::new(4, 0),
                ProtocolVersion::new(4, 2),
            ),
        )
        .expect_err("major 4 is not served");
    let frame = refused
        .frame
        .expect("a client whose offer reaches 3.1 can parse the frame");
    assert_eq!(frame.code, ErrorCode::ProtocolVersionUnsupported);
    assert!(!frame.retryable);
    assert_eq!(frame.majors_served, vec![3, 2]);

    // A client below the window is refused too, and gets no frame: its offer does not reach
    // the version that defines one, and "a frame the client cannot parse is not a typed
    // refusal".
    let old = daemon
        .welcome(
            &policy(),
            &hello(
                "cap_reader",
                "agent:reader",
                ProtocolVersion::new(1, 0),
                ProtocolVersion::new(1, 9),
            ),
        )
        .expect_err("major 1 is not served");
    assert_eq!(old.frame, None);
}

#[test]
fn an_unknown_feature_is_never_echoed_and_a_known_one_is_intersected() {
    let daemon = daemon();
    let mut asking = hello("cap_reader", "agent:reader", version(), version());
    asking.features = Optional::Present(vec![
        "evidence-subscriptions".to_owned(),
        "promote-my-own-claims".to_owned(),
    ]);
    let (_, welcome) = daemon
        .welcome(&policy(), &asking)
        .expect("the reader's capability is registered");
    assert_eq!(welcome.features, vec!["evidence-subscriptions".to_owned()]);
    assert!(
        !welcome
            .features
            .iter()
            .any(|feature| feature == "promote-my-own-claims"),
        "an identifier the daemon does not implement is never granted by being echoed"
    );

    // A client offering nothing is granted nothing, rather than everything the daemon has.
    let (_, silent) = daemon
        .welcome(
            &policy(),
            &hello("cap_reader", "agent:reader", version(), version()),
        )
        .expect("registered");
    assert!(silent.features.is_empty());
}

#[test]
fn every_capability_failure_at_the_handshake_is_one_answer() {
    let daemon = daemon();
    let refusals: Vec<Refusal> = [
        // never registered
        hello("cap_never-minted", "agent:reader", version(), version()),
        // registered, wrong actor
        hello("cap_reader", "agent:observer", version(), version()),
        // registered, expired against the daemon's reading
        hello("cap_expiring", "agent:expiring", version(), version()),
    ]
    .into_iter()
    .map(|hello| {
        daemon
            .welcome(&policy(), &hello)
            .expect_err("each of these is refused")
    })
    .collect();

    // X1: "a client MUST NOT be able to tell them apart".
    assert_eq!(refusals[0], refusals[1]);
    assert_eq!(refusals[1], refusals[2]);
    let frame = refusals[0].frame.clone().expect("a typed refusal frame");
    assert_eq!(frame.code, ErrorCode::CapabilityDenied);
    assert!(!frame.retryable);

    // And a revoked token joins them: revocation, not purge, is how a capability stops
    // being usable, and a revoked one is indistinguishable from one never registered.
    let mut revoking = daemon_with(Some(now()));
    revoking
        .state_mut()
        .revoke_capability(&cap("cap_revocable"));
    let revoked = revoking
        .welcome(
            &policy(),
            &hello("cap_revocable", "agent:revocable", version(), version()),
        )
        .expect_err("a revoked capability is refused");
    assert_eq!(revoked, refusals[0]);
}

#[test]
fn a_client_too_old_to_parse_the_reject_frame_is_closed_without_one() {
    let daemon = daemon();
    let refused = daemon
        .welcome(
            &policy(),
            &hello(
                "cap_never-minted",
                "agent:reader",
                ProtocolVersion::new(3, 0),
                ProtocolVersion::new(3, 0),
            ),
        )
        .expect_err("an unregistered capability is refused");
    assert_eq!(
        refused.frame, None,
        "a frame the client cannot parse is not a typed refusal"
    );
}

#[test]
fn revocation_takes_effect_on_the_next_request_and_unmakes_nothing() {
    let mut fixture = fixture();
    let trace = fixture.trace.clone();
    let handle = ingested_handle(&ingest_with(
        &mut fixture,
        "agent:revocable",
        "cap_revocable",
        &trace,
        "req_ingest",
        "idem-ingest",
    ));

    fixture
        .daemon
        .state_mut()
        .revoke_capability(&cap("cap_revocable"));

    // R1 / E1: the grant is not cached at handshake time, so the very next request fails.
    let denied = ingest_with(
        &mut fixture,
        "agent:revocable",
        "cap_revocable",
        &trace,
        "req_after",
        "idem-after",
    );
    assert_eq!(code(&denied), ErrorCode::CapabilityDenied);

    // R2 / E3: "Revocation MUST NOT delete or invalidate any published artifact, and MUST
    // NOT retroactively unmake a result the capability lawfully produced (INV-009)."
    let read = get(&mut fixture, &handle, "req_get");
    assert_eq!(read.envelope.status, ResultStatus::Ok);
    assert_eq!(node(&fixture, &handle).producer, who("agent:revocable"));
}

#[test]
fn an_expiring_capability_is_judged_against_a_reading_and_denied_without_one() {
    // Judged: a daemon whose reading is past the expiry denies, at the handshake and again
    // at dispatch. Undecidable: a daemon with no reading denies, because "a daemon that
    // cannot decide admission fails closed".
    for clock in [Some(now()), None] {
        let mut fixture = fixture_with(clock);
        let trace = fixture.trace.clone();
        let handshake = fixture.daemon.welcome(
            &policy(),
            &hello("cap_expiring", "agent:expiring", version(), version()),
        );
        assert!(handshake.is_err());

        let dispatched = ingest_with(
            &mut fixture,
            "agent:expiring",
            "cap_expiring",
            &trace,
            "req_expired",
            "idem-expired",
        );
        assert_eq!(code(&dispatched), ErrorCode::CapabilityDenied);
    }

    // A reading *before* the expiry admits it, which is what makes the two above a decision
    // rather than a blanket refusal.
    let mut early = fixture_with(Some(when("2026-06-01T00:00:00.000Z")));
    let trace = early.trace.clone();
    let admitted = ingest_with(
        &mut early,
        "agent:expiring",
        "cap_expiring",
        &trace,
        "req_early",
        "idem-early",
    );
    assert_eq!(
        admitted.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        admitted.envelope.error
    );
}

#[test]
fn a_delegated_capability_that_exceeds_its_parent_is_refused_before_the_family_runs() {
    // D6/D7: `cap_escalating` is a child of the *reader* and claims `execute` plus a data
    // grant its parent does not hold. Admission walks the chain and refuses it, so the
    // `observe` family never sees the request — asserted by the graph staying empty rather
    // than by inspecting the predicate.
    let mut fixture = fixture();
    let trace = fixture.trace.clone();
    let denied = ingest_with(
        &mut fixture,
        "agent:escalating",
        "cap_escalating",
        &trace,
        "req_escalate",
        "idem-escalate",
    );
    assert_eq!(code(&denied), ErrorCode::CapabilityDenied);
    assert_eq!(fixture.daemon.state().evidence_nodes().count(), 0);
    // "denial precedes semantic work and precedes the index" (X3): nothing was published.
    assert!(fixture.daemon.state().evidence_events().is_empty());
}

#[test]
fn the_production_trace_grant_is_required_beyond_the_execute_level() {
    // R-4: `observe.ingest` requires `production_trace` in addition to `execute`, and a
    // capability without it is denied even though its level admits the operation. The check
    // is `admission::required_grant`, decided from registry data before any family runs.
    let mut fixture = fixture();
    let trace = fixture.trace.clone();
    let denied = ingest_with(
        &mut fixture,
        "agent:ungranted",
        "cap_ungranted",
        &trace,
        "req_ungranted",
        "idem-ungranted",
    );
    assert_eq!(code(&denied), ErrorCode::CapabilityDenied);
    assert_eq!(fixture.daemon.state().evidence_nodes().count(), 0);

    // The same request under a capability that holds the grant succeeds, so the refusal is
    // about the grant and not about the level.
    let admitted = ingest(&mut fixture, "req_granted", "idem-granted");
    assert_eq!(admitted.envelope.status, ResultStatus::Ok);
}

// --- conformance ----------------------------------------------------------------------------

#[test]
fn every_fault_these_families_can_raise_is_inside_its_operations_error_union() {
    for (operation, code) in evidence::FAULTS.iter().chain(observe::FAULTS) {
        let spec = registry::operation(operation).expect("a registered operation");
        assert!(
            errors::admits(spec, *code),
            "{operation} may not answer with {code:?} under `rule errors.common`"
        );
    }
}

#[test]
fn the_payload_beside_the_envelope_is_the_operations_own_response_body() {
    let mut fixture = fixture();
    let ingested = ingest(&mut fixture, "req_ingest", "idem-ingest");
    assert_eq!(ingested.payload.operation(), Some("observe.ingest"));
    let handle = ingested_handle(&ingested);

    assert_eq!(
        get(&mut fixture, &handle, "req_get").payload.operation(),
        Some("evidence.get")
    );
    assert_eq!(
        query(&mut fixture, empty_query(), "req_query")
            .payload
            .operation(),
        Some("evidence.query")
    );
    assert_eq!(
        verify(&mut fixture, &handle, "req_verify", "idem-verify")
            .payload
            .operation(),
        Some("evidence.verify")
    );

    // The envelope's own `payload` reads null at this layer, whatever the typed body is:
    // there is no codec here, and inventing bytes for an `Opaque` would be inventing wire
    // format.
    assert_eq!(ingested.envelope.payload, Nullable::Null);
}

#[test]
fn the_wire_status_vocabulary_and_the_lattice_agree_token_for_token() {
    // Two crates transcribe plan §11.4's nine statuses, and this daemon converts between
    // them on every promotion. The agreement is asserted rather than trusted.
    assert_eq!(EvidenceStatus::ALL.len(), ClaimStatus::ALL.len());
    for (wire, lattice) in EvidenceStatus::ALL.iter().zip(ClaimStatus::ALL) {
        assert_eq!(wire.as_wire(), lattice.as_str());
        assert_eq!(lattice_status(*wire), lattice);
        assert_eq!(wire_status(lattice), *wire);
    }
    // And the lattice's bottom is the status an append lands at.
    assert_eq!(wire_status(ClaimStatus::BOTTOM), EvidenceStatus::Proposed);
}

#[test]
fn the_default_service_identity_is_a_well_formed_service_actor() {
    let family = EvidenceFamily::new();
    assert_eq!(family.service().as_str(), DEFAULT_SERVICE);
    assert!(DEFAULT_SERVICE.starts_with("service:"));
}

#[test]
fn an_unkeyed_mutation_and_an_unbudgeted_task_are_refused_before_the_graph_is_touched() {
    // The obligations `obligation::check_request` reads off the registry, exercised through
    // this family's own operations so the inheritance is evidenced rather than assumed.
    let mut fixture = fixture();
    let trace = fixture.trace.clone();

    let mut unkeyed = envelope("observe.ingest", "agent:observer", "cap_observer", "req_a");
    unkeyed.budget = Optional::Present(budget());
    let refused = fixture.daemon.dispatch(&OperationRequest {
        envelope: unkeyed,
        arguments: Arguments::ObserveIngest(ObserveIngestRequest {
            trace: trace.clone(),
            instrumentation_profile: PROFILE.to_owned(),
        }),
    });
    assert_eq!(code(&refused), ErrorCode::MalformedRequest);

    let mut unbudgeted = envelope("observe.ingest", "agent:observer", "cap_observer", "req_b");
    unbudgeted.idempotency_key = Optional::Present("idem-b".to_owned());
    let refused = fixture.daemon.dispatch(&OperationRequest {
        envelope: unbudgeted,
        arguments: Arguments::ObserveIngest(ObserveIngestRequest {
            trace: trace.clone(),
            instrumentation_profile: PROFILE.to_owned(),
        }),
    });
    assert_eq!(code(&refused), ErrorCode::MalformedRequest);

    // A key on a `@readonly` read is refused too: a caller that believes a read mutates
    // something has misunderstood, and honouring the key would put that misunderstanding in
    // the ledger.
    let mut keyed_read = envelope("evidence.get", "agent:reader", "cap_reader", "req_c");
    keyed_read.idempotency_key = Optional::Present("idem-c".to_owned());
    let refused = fixture.daemon.dispatch(&OperationRequest {
        envelope: keyed_read,
        arguments: Arguments::EvidenceGet(EvidenceGetRequest {
            evidence: EvidenceHandle::new("ev_anything").expect("handle"),
            inline: Optional::Absent,
        }),
    });
    assert_eq!(code(&refused), ErrorCode::MalformedRequest);

    assert_eq!(fixture.daemon.state().evidence_nodes().count(), 0);
}

#[test]
fn a_replayed_ingest_returns_the_first_results_node_identity_verbatim() {
    // `rule idempotency.replay`, inherited from the dispatcher: the same key with the same
    // request returns the recorded outcome rather than appending twice.
    let mut fixture = fixture();
    let first = ingest(&mut fixture, "req_ingest", "idem-ingest");
    let replay = ingest(&mut fixture, "req_ingest", "idem-ingest");
    assert_eq!(first, replay);

    // A different request under the same key is the typed refusal, and it changes nothing.
    let other = fixture.other.clone();
    let reused = ingest_with(
        &mut fixture,
        "agent:observer",
        "cap_observer",
        &other,
        "req_other",
        "idem-ingest",
    );
    assert_eq!(code(&reused), ErrorCode::IdempotencyKeyReused);
    assert_eq!(fixture.daemon.state().evidence_nodes().count(), 1);
}

#[test]
fn a_daemon_with_no_clock_refuses_to_stamp_a_provenance_record_it_cannot_derive() {
    // `provenance.created_at` is required by the node schema and time is an explicit effect
    // (INV-005, ADR-0003). A daemon built with no reading says so rather than inventing one.
    let mut fixture = fixture_with(None);
    let trace = fixture.trace.clone();
    let refused = ingest_with(
        &mut fixture,
        "agent:observer",
        "cap_observer",
        &trace,
        "req_ingest",
        "idem-ingest",
    );
    assert_eq!(code(&refused), ErrorCode::UnsupportedSemanticFeature);
    assert_eq!(fixture.daemon.state().evidence_nodes().count(), 0);
}

// =========================================================================================
// INV-004's edge dimension: `evidence.link` (protocol 3.3, RFC 0038 D1-D3, bn-3sypm)
// =========================================================================================

fn link_as(
    fixture: &mut Fixture,
    actor: &str,
    capability: &str,
    subject: &EvidenceHandle,
    receipt: &Commitment,
    request: &str,
    key: &str,
) -> OperationOutcome {
    link_under(
        fixture,
        actor,
        capability,
        subject,
        receipt,
        "kernel-core/1",
        request,
        key,
    )
}

/// `evidence.link` with the checker profile named, because a receipt node's identity is a
/// function of (content, profile): two checks of one artifact under two profiles are two
/// receipt nodes, which is what lets a test build a chain out of one staged blob.
#[expect(clippy::too_many_arguments, reason = "one parameter per wire field")]
fn link_under(
    fixture: &mut Fixture,
    actor: &str,
    capability: &str,
    subject: &EvidenceHandle,
    receipt: &Commitment,
    checker_profile: &str,
    request: &str,
    key: &str,
) -> OperationOutcome {
    let mut envelope = envelope("evidence.link", actor, capability, request);
    envelope.idempotency_key = Optional::Present(key.to_owned());
    fixture.daemon.dispatch(&OperationRequest {
        envelope,
        arguments: Arguments::EvidenceLink(EvidenceLinkRequest {
            subject: subject.clone(),
            receipt: receipt.clone(),
            checker_profile: checker_profile.to_owned(),
        }),
    })
}

/// The green path, and every INV-004 property the edge surface is supposed to carry.
#[test]
fn a_checker_appends_a_receipt_and_the_check_edge_that_names_it() {
    let mut fixture = fixture();
    let subject = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));
    let receipt_content = fixture.other.clone();

    let linked = link_as(
        &mut fixture,
        "service:kernel-core",
        "cap_checker",
        &subject,
        &receipt_content,
        "req_link",
        "idem-link",
    );
    assert_eq!(
        linked.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        linked.envelope.error
    );
    let (edge_handle, receipt_handle, checker) = match &linked.payload {
        Payload::EvidenceLink(response) => (
            response.edge.clone(),
            response.receipt.clone(),
            response.checker.clone(),
        ),
        other => panic!("expected an evidence.link payload, got {other:?}"),
    };
    // The checker is the admitted actor. There is no request field it could have come from.
    assert_eq!(checker, "service:kernel-core");

    // RFC 0038 D1: the edge's `to` is a `receipt`, and the node landed at the bottom of the
    // lattice — this operation writes no status.
    let receipt = node(&fixture, &receipt_handle);
    assert_eq!(receipt.kind, EvidenceNodeKind::Receipt);
    assert_eq!(receipt.status(), ClaimStatus::BOTTOM);
    assert_eq!(receipt.producer.as_str(), "service:kernel-core");

    // The edge is readable through `evidence.get`, and its record carries `checker` —
    // which, until 3.3, `evidence.get`'s `edge` field could only ever read null.
    let fetched = get(&mut fixture, &edge_handle, "req_get_edge");
    assert_eq!(fetched.envelope.status, ResultStatus::Ok);
    let edge_text = match &fetched.payload {
        Payload::EvidenceGet(response) => match &response.edge {
            Nullable::Value(bytes) => {
                String::from_utf8(bytes.as_bytes().to_vec()).expect("canonical JSON is UTF-8")
            }
            Nullable::Null => panic!("the edge record is not null for an edge the graph holds"),
        },
        other => panic!("expected an evidence.get payload, got {other:?}"),
    };
    for needle in [
        "\"kind\":\"CHECKED_BY\"",
        "\"checker\":\"service:kernel-core\"",
        "\"schema_id\":\"https://continuum.dev/schema/evidence-graph-edge.json\"",
    ] {
        assert!(edge_text.contains(needle), "{needle} in {edge_text}");
    }
    assert!(edge_text.contains(&format!("\"from\":\"{}\"", subject.as_str())));
    assert!(edge_text.contains(&format!("\"to\":\"{}\"", receipt_handle.as_str())));

    // `evidence.query` reports the edge, which it also could not do before 3.3.
    let queried = fixture.daemon.dispatch(&OperationRequest {
        envelope: envelope(
            "evidence.query",
            "agent:reader",
            "cap_reader",
            "req_query_edges",
        ),
        arguments: Arguments::EvidenceQuery(EvidenceQueryRequest {
            query: EvidenceQuery {
                node_kinds: Optional::Absent,
                edge_kinds: Optional::Present(vec![EvidenceEdgeKind::CheckedBy]),
                statuses: Optional::Absent,
                claim_id: Optional::Absent,
                roots: Optional::Absent,
                max_depth: Optional::Absent,
            },
        }),
    });
    match &queried.payload {
        Payload::EvidenceQuery(response) => assert_eq!(response.edges, vec![edge_handle.clone()]),
        other => panic!("expected an evidence.query payload, got {other:?}"),
    }

    // A committed delta was recorded for each artifact, the edge one naming the edge.
    let kinds: Vec<EvidenceEventKind> = fixture
        .daemon
        .state()
        .evidence_events()
        .iter()
        .map(|event| event.kind)
        .collect();
    assert_eq!(
        kinds,
        [
            EvidenceEventKind::NodePublished,
            EvidenceEventKind::NodePublished,
            EvidenceEventKind::EdgePublished
        ]
    );
}

/// INV-004, the edge half: the thing being checked cannot mint its own checked-by.
#[test]
fn a_checker_may_not_record_a_check_of_its_own_production() {
    let mut fixture = fixture();
    // The subject is produced by `service:kernel-core` itself.
    let trace = fixture.trace.clone();
    let subject = ingested_handle(&ingest_with(
        &mut fixture,
        "service:kernel-core",
        "cap_checker",
        &trace,
        "req_ingest",
        "idem-ingest",
    ));
    let receipt_content = fixture.other.clone();

    let refused = link_as(
        &mut fixture,
        "service:kernel-core",
        "cap_checker",
        &subject,
        &receipt_content,
        "req_link",
        "idem-link",
    );
    assert_eq!(code(&refused), ErrorCode::InsufficientEvidence);
    // Nothing was written: no receipt node, and no edge.
    assert_eq!(fixture.daemon.state().evidence_nodes().count(), 1);
    assert_eq!(fixture.daemon.state().evidence_edges().count(), 0);

    // The control: the identical call from a *different* service is admitted, so the
    // refusal is about who checked and not about the request.
    let admitted = link_as(
        &mut fixture,
        "service:continuumd",
        "cap_root",
        &subject,
        &receipt_content,
        "req_link_other",
        "idem-link-other",
    );
    assert_eq!(
        admitted.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        admitted.envelope.error
    );
    assert_eq!(fixture.daemon.state().evidence_edges().count(), 1);
}

/// An untrusted agent cannot append a check edge at any authority level.
#[test]
fn only_a_service_actor_may_append_a_check_edge() {
    let mut fixture = fixture();
    let subject = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));
    let receipt_content = fixture.other.clone();

    // `cap_observer` is `execute` — the same level `cap_checker` holds — and carries the
    // production-trace grant besides. The only thing it lacks is the `service:` scheme.
    let refused = link_as(
        &mut fixture,
        "agent:observer",
        "cap_observer",
        &subject,
        &receipt_content,
        "req_link",
        "idem-link",
    );
    assert_eq!(code(&refused), ErrorCode::CapabilityDenied);
    assert_eq!(fixture.daemon.state().evidence_edges().count(), 0);

    // …and the denial is byte-identical to the one a caller gets for a subject the graph
    // does not hold, so the refusal is not an oracle for either fact (RFC 0027 X1/X2).
    let unknown = EvidenceHandle::new("ev_absent").expect("a well-formed handle");
    let missing = link_as(
        &mut fixture,
        "service:kernel-core",
        "cap_checker",
        &unknown,
        &receipt_content,
        "req_link_missing",
        "idem-link-missing",
    );
    assert_eq!(code(&missing), ErrorCode::CapabilityDenied);
    assert_eq!(refused.envelope.error, missing.envelope.error);
}

/// A replayed check converges on one edge: `rule evidence.edge_identity` keeps provenance
/// out of an edge's identity, so a checker's retry is idempotent by content.
#[test]
fn a_second_identical_check_converges_on_the_edge_already_held() {
    let mut fixture = fixture();
    let subject = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));
    let receipt_content = fixture.other.clone();

    let first = link_as(
        &mut fixture,
        "service:kernel-core",
        "cap_checker",
        &subject,
        &receipt_content,
        "req_link",
        "idem-link",
    );
    // A *different* idempotency key, so the ledger does not answer this one — the
    // convergence has to come from the identity rule.
    let second = link_as(
        &mut fixture,
        "service:kernel-core",
        "cap_checker",
        &subject,
        &receipt_content,
        "req_link_again",
        "idem-link-again",
    );
    assert_eq!(
        second.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        second.envelope.error
    );
    let handles = |outcome: &OperationOutcome| match &outcome.payload {
        Payload::EvidenceLink(response) => (response.edge.clone(), response.receipt.clone()),
        other => panic!("expected an evidence.link payload, got {other:?}"),
    };
    assert_eq!(handles(&first), handles(&second));
    assert_eq!(fixture.daemon.state().evidence_edges().count(), 1);
    // Exactly one `edge_published` delta: the second append wrote nothing.
    assert_eq!(
        fixture
            .daemon
            .state()
            .evidence_events()
            .iter()
            .filter(|event| event.kind == EvidenceEventKind::EdgePublished)
            .count(),
        1
    );
}

/// A node identity and an edge identity are drawn from one seam and cannot collide
/// (`rule evidence.edge_identity`), and `evidence.link` writes no status (INV-004).
#[test]
fn an_edge_identity_is_domain_separated_from_a_node_identity() {
    // The domain tag carries a `/`, which is outside the artifact-handle character class
    // every `Commitment` this daemon derives belongs to — so no node preimage can spell an
    // edge preimage's first part. That is a fact about the tag, checkable here.
    assert!(EDGE_IDENTITY_DOMAIN.contains('/'));
    assert!(
        continuumd::protocol::scalar::ArtifactHandle::new(EDGE_IDENTITY_DOMAIN).is_err(),
        "the domain tag must not be a well-formed artifact handle"
    );

    let mut fixture = fixture();
    let subject = ingested_handle(&ingest(&mut fixture, "req_ingest", "idem-ingest"));
    let before = node(&fixture, &subject).status();
    let receipt_content = fixture.other.clone();
    let linked = link_as(
        &mut fixture,
        "service:kernel-core",
        "cap_checker",
        &subject,
        &receipt_content,
        "req_link",
        "idem-link",
    );
    let (edge_handle, receipt_handle) = match &linked.payload {
        Payload::EvidenceLink(response) => (response.edge.clone(), response.receipt.clone()),
        other => panic!("expected an evidence.link payload, got {other:?}"),
    };
    assert_ne!(edge_handle, receipt_handle);
    assert_ne!(edge_handle, subject);
    // The edge handle names an edge and never a node, and vice versa.
    assert!(fixture.daemon.state().evidence(&edge_handle).is_none());
    assert!(
        fixture
            .daemon
            .state()
            .evidence_edge(&receipt_handle)
            .is_none()
    );
    // The subject's status is exactly what it was: an edge is not a promotion.
    assert_eq!(node(&fixture, &subject).status(), before);
}
