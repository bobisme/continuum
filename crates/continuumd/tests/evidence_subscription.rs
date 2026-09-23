//! `evidence.subscribe`, end to end: the frontier at this layer and the delta frames at the
//! byte boundary.
//!
//! # What this file is evidence for
//!
//! RFC 0038's evidence graph declares a notification channel — "`evidence.subscribe` streams
//! typed evidence-graph deltas for a declared scope" — and until bn-3080b the daemon refused
//! the operation, honestly: `@streaming` had said since protocol 3.0 that the operation
//! "delivers `events` frames on the same connection" and its `events` clause had named the
//! struct, but nothing said what an event frame *is*, how a client tells one from a
//! `ResultEnvelope`, or which committed deltas a declared scope selects. A frontier answered
//! into that gap would have been the empty success `rule errors.unsupported_surface` forbids.
//!
//! `rule subscription.delivery` (IDL 1.7) decides those three things, and this file is the
//! check that the daemon and the transport implement what it decided. Every delta below
//! crosses a real byte boundary: the client writes a request frame, the server reads bytes it
//! did not construct, and the frames that come back are decoded by shape — no tag, no
//! out-of-band value, no typed hand-off.
//!
//! # The four clauses, and where each is held
//!
//! | Clause of `rule subscription.delivery` | Held by |
//! |---|---|
//! | an event frame is the declared event struct, told apart from a result by its required members | [`an_event_frame_and_a_result_frame_never_validate_as_each_other`] |
//! | a delta selected by two scopes on one connection is delivered once | [`a_delta_two_scopes_select_is_delivered_once`] |
//! | a subscription delivers only what was committed after it opened | [`the_frontier_covers_what_came_before_and_the_frames_what_came_after`] |
//! | a delta is in scope exactly when the scope's query selects the artifact its kind names | [`a_delta_outside_the_declared_scope_reaches_no_frame`], [`a_scope_clause_is_read_against_the_graph_when_the_frame_is_written`] |
//! | …including the `roots`/`max_depth` traversal of `rule evidence.traversal` (IDL 1.8) | [`a_rooted_scope_delivers_the_deltas_its_traversal_reaches_and_no_others`], [`a_zero_depth_scope_is_membership_and_a_new_neighbor_reaches_no_frame`] |
//! | the predicate runs once per delta, and an unselected delta is not reconsidered | [`a_delta_the_scope_did_not_select_when_examined_is_not_reconsidered`] |
//!
//! The daemon-side half — that the frontier and `evidence.query` are one predicate — lives in
//! `daemon_evidence.rs`, beside the query it must agree with.

use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::codec::cbor::Cbor;
use continuumd::codec::json::Json;
use continuumd::codec::{Document, from_bytes};
use continuumd::daemon::Daemon;
use continuumd::daemon::evidence::EvidenceFamily;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::protocol::envelope::{Budget, RequestEnvelope, ResultEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::evidence::{
    EvidenceLinkRequest, EvidenceSubscribeRequest, EvidenceVerifyRequest,
};
use continuumd::protocol::operations::observe::ObserveIngestRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, EvidenceHandle, Opaque, OperationName, ProtocolVersion,
    RequestId, Timestamp,
};
use continuumd::protocol::shared::EvidenceQuery;
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::task::EvidenceEvent;
use continuumd::protocol::vocabulary::{
    AuthorityLevel, DataGrant, Encoding, ErrorCode, EvidenceEventKind, EvidenceNodeKind,
    EvidenceStatus, ResultStatus,
};
use continuumd::transport::{
    LocalPair, Server, ServerFrame, client_send_in, decode_server_frame_in,
};

/// The same Die Hard trace the operation-layer evidence suites ingest.
const TRACE: &str = "{\"events\":[{\"at\":0,\"op\":\"fill\"},{\"at\":1,\"op\":\"pour\"}]}\n";

/// A second trace, so a scope has more than one node to choose between.
const OTHER_TRACE: &str = "{\"events\":[{\"at\":0,\"op\":\"empty\"}]}\n";

/// The instrumentation profile the traces were captured under (plan §18.4).
const PROFILE: &str = "otel-1.0/sampled";

// --- fixtures -----------------------------------------------------------------------------

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 4)
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

fn now() -> Timestamp {
    Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp")
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
        delegation_depth: 3,
        profile,
        instances: Optional::Absent,
    }
}

fn hello(encodings: Vec<Encoding>) -> ClientHello {
    ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings,
        client: "continuumd-evidence-subscription-test".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    }
}

fn negotiated(encoding: Encoding) -> Negotiated {
    negotiate(
        &[version()],
        ProtocolWindow::new(3),
        ENCODINGS,
        &hello(vec![encoding]),
    )
    .expect("3.4 is served")
}

/// The daemon behind every connection here: the two families that write the graph, and the
/// four capabilities the writes need, serving a connection that settled on `connection`.
/// `extra` capabilities are provisioned at build time under `cap_root`, so the publication
/// store knows them and they can publish.
fn daemon_with(encoding: Encoding, connection: &str, extra: Vec<CapabilityDescriptor>) -> Daemon {
    let root = Some(cap("cap_root"));
    let mut builder = Daemon::builder(Blake3Identity, negotiated(encoding), cap(connection))
        .capability(
            {
                let mut descriptor = grant(
                    "cap_root",
                    "service:continuumd",
                    AuthorityLevel::Promote,
                    Optional::Present(traced(&[DataGrant::ProductionTrace])),
                );
                descriptor.delegation_depth = 4;
                descriptor
            },
            None,
        )
        // The producer.
        .capability(
            grant(
                "cap_observer",
                "agent:observer",
                AuthorityLevel::Execute,
                Optional::Present(traced(&[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        )
        // The reader: every `evidence` operation is `read` authority.
        .capability(
            grant(
                "cap_reader",
                "agent:reader",
                AuthorityLevel::Read,
                Optional::Absent,
            ),
            root.clone(),
        )
        // The checker: a `service:` actor, which is what `evidence.link` gates on.
        .capability(
            grant(
                "cap_checker",
                "service:kernel-core",
                AuthorityLevel::Execute,
                Optional::Present(traced(&[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        )
        .family(EvidenceFamily::new())
        .family(ObserveFamily)
        .now(now());
    for descriptor in extra {
        builder = builder.capability(descriptor, root.clone());
    }
    builder.build()
}

/// One connection, plus the two staged traces its ingests reference.
struct World {
    server: Server,
    pair: LocalPair,
    trace: Commitment,
    other: Commitment,
}

fn world(encoding: Encoding) -> World {
    world_on(encoding, "cap_root")
}

/// [`world`], on a connection whose capability is `connection`.
fn world_on(encoding: Encoding, connection: &str) -> World {
    world_with(encoding, connection, Vec::new())
}

/// [`world_on`], with `extra` capabilities provisioned at build time.
fn world_with(encoding: Encoding, connection: &str, extra: Vec<CapabilityDescriptor>) -> World {
    let mut daemon = daemon_with(encoding, connection, extra);
    let mut stage = |path: &str, content: &str| {
        daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new(path).expect("a workspace path"),
                content.as_bytes().to_vec(),
            )
            .expect("staging names its content")
    };
    let trace = stage("traces/die-hard.jsonl", TRACE);
    let other = stage("traces/other.jsonl", OTHER_TRACE);
    World {
        server: Server::new(daemon, negotiated(encoding)),
        pair: LocalPair::new(),
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

/// A `@mutation` envelope: an idempotency key, and a budget for the `@task_starting` ones.
fn started(mut envelope: RequestEnvelope, key: &str) -> RequestEnvelope {
    envelope.idempotency_key = Optional::Present(key.to_owned());
    envelope.budget = Optional::Present(budget());
    envelope
}

fn empty_scope() -> EvidenceQuery {
    EvidenceQuery {
        node_kinds: Optional::Absent,
        edge_kinds: Optional::Absent,
        statuses: Optional::Absent,
        claim_id: Optional::Absent,
        roots: Optional::Absent,
        max_depth: Optional::Absent,
    }
}

// --- the client half ----------------------------------------------------------------------

/// Write one request frame, generic in the connection's encoding.
fn send<D: Document>(
    world: &mut World,
    envelope: &RequestEnvelope,
    arguments: &Arguments,
) -> &'static str {
    client_send_in::<D>(&mut world.pair, envelope, arguments).expect("the request encodes");
    "sent"
}

/// Serve every pending request and decode every frame that came back.
///
/// The operation name is needed only to decode a *result* payload; an event frame is decoded
/// by its own shape, which is the point of `rule subscription.delivery`'s discrimination.
fn exchange<D: Document>(world: &mut World, operation: &str) -> Vec<ServerFrame> {
    world
        .server
        .serve(&mut world.pair)
        .expect("the exchange completes");
    let mut frames = Vec::new();
    while let Some(frame) = world.pair.to_client.take_frame().expect("a whole frame") {
        frames.push(decode_server_frame_in::<D>(operation, &frame).expect("a declared shape"));
    }
    frames
}

/// The deltas among a batch of frames, in the order they arrived.
fn deltas(frames: &[ServerFrame]) -> Vec<EvidenceEvent> {
    frames
        .iter()
        .filter_map(|frame| match frame {
            ServerFrame::Event(event) => Some(event.clone()),
            ServerFrame::Result(..) => None,
        })
        .collect()
}

/// The one result among a batch of frames.
fn result(frames: &[ServerFrame]) -> &ResultEnvelope {
    let mut results = frames.iter().filter_map(|frame| match frame {
        ServerFrame::Result(envelope, _) => Some(envelope.as_ref()),
        ServerFrame::Event(_) => None,
    });
    let first = results.next().expect("a request is answered");
    assert!(results.next().is_none(), "one request, one result");
    first
}

/// The frontier a subscription answered with.
fn frontier(frames: &[ServerFrame]) -> Vec<EvidenceHandle> {
    frames
        .iter()
        .find_map(|frame| match frame {
            ServerFrame::Result(_, payload) => match payload.as_ref() {
                Payload::EvidenceSubscribe(body) => Some(body.frontier.clone()),
                _ => None,
            },
            _ => None,
        })
        .expect("the subscription answers its declared response body")
}

// --- the exchanges the tests are written in terms of ---------------------------------------

fn subscribe<D: Document>(
    world: &mut World,
    scope: EvidenceQuery,
    request: &str,
) -> Vec<ServerFrame> {
    send::<D>(
        world,
        &envelope("evidence.subscribe", "agent:reader", "cap_reader", request),
        &Arguments::EvidenceSubscribe(EvidenceSubscribeRequest { scope }),
    );
    exchange::<D>(world, "evidence.subscribe")
}

fn ingest<D: Document>(
    world: &mut World,
    trace: &Commitment,
    request: &str,
    key: &str,
) -> Vec<ServerFrame> {
    send::<D>(
        world,
        &started(
            envelope("observe.ingest", "agent:observer", "cap_observer", request),
            key,
        ),
        &Arguments::ObserveIngest(ObserveIngestRequest {
            trace: trace.clone(),
            instrumentation_profile: PROFILE.to_owned(),
        }),
    );
    exchange::<D>(world, "observe.ingest")
}

fn verify<D: Document>(
    world: &mut World,
    evidence: &EvidenceHandle,
    request: &str,
    key: &str,
) -> Vec<ServerFrame> {
    send::<D>(
        world,
        &started(
            envelope("evidence.verify", "agent:reader", "cap_reader", request),
            key,
        ),
        &Arguments::EvidenceVerify(EvidenceVerifyRequest {
            evidence: evidence.clone(),
            expected_status: Optional::Absent,
        }),
    );
    exchange::<D>(world, "evidence.verify")
}

fn link<D: Document>(
    world: &mut World,
    subject: &EvidenceHandle,
    receipt: &Commitment,
    request: &str,
    key: &str,
) -> Vec<ServerFrame> {
    let mut envelope = envelope(
        "evidence.link",
        "service:kernel-core",
        "cap_checker",
        request,
    );
    envelope.idempotency_key = Optional::Present(key.to_owned());
    send::<D>(
        world,
        &envelope,
        &Arguments::EvidenceLink(EvidenceLinkRequest {
            subject: subject.clone(),
            receipt: receipt.clone(),
            checker_profile: "kernel-core/1".to_owned(),
        }),
    );
    exchange::<D>(world, "evidence.link")
}

/// The node handle an ingest of [`TRACE`] under [`PROFILE`] lands on, learned on a throwaway
/// connection.
///
/// A node's wire identity is a function of the artifact it is about and the profile it was
/// captured under, and of nothing else (RFC 0038 D4) — so the identity a scratch daemon
/// derives is the identity the daemon under test will derive, and a test can name it before
/// the write it is about has happened. Recomputing the preimage here instead would be a
/// second spelling of the derivation.
fn node_identity() -> EvidenceHandle {
    let mut scratch = world(Encoding::CanonicalJson);
    let trace = scratch.trace.clone();
    ingested(&ingest::<Json>(
        &mut scratch,
        &trace,
        "req_ingest",
        "idem-ingest",
    ))
}

/// The node handle an ingest appended.
fn ingested(frames: &[ServerFrame]) -> EvidenceHandle {
    frames
        .iter()
        .find_map(|frame| match frame {
            ServerFrame::Result(_, payload) => match payload.as_ref() {
                Payload::ObserveIngest(body) => body.evidence.first().cloned(),
                _ => None,
            },
            _ => None,
        })
        .expect("an ingest names the node it appended")
}

// =========================================================================================
// positive: the channel delivers
// =========================================================================================

/// A delta committed after a subscription opened arrives as its own frame on the same
/// connection.
///
/// This is the whole of what the operation promised and could not keep before bn-3080b. The
/// frame is decoded from bytes by shape alone, so what it demonstrates is not that the daemon
/// recorded a delta — `daemon_evidence.rs` has held that since the graph landed — but that a
/// client on the far side of the boundary can read one.
#[test]
fn a_delta_committed_after_a_subscription_arrives_as_a_frame_on_the_same_connection() {
    let mut world = world(Encoding::CanonicalJson);
    let opened = subscribe::<Json>(&mut world, empty_scope(), "req_sub");
    assert_eq!(result(&opened).status, ResultStatus::Ok);
    assert!(
        frontier(&opened).is_empty(),
        "nothing was committed before this subscription"
    );
    assert!(
        deltas(&opened).is_empty(),
        "opening a subscription is not itself a delta"
    );

    let trace = world.trace.clone();
    let appended = ingest::<Json>(&mut world, &trace, "req_ingest", "idem-ingest");
    let node = ingested(&appended);

    let delivered = deltas(&appended);
    assert_eq!(delivered.len(), 1, "one committed delta, one frame");
    assert_eq!(delivered[0].kind, EvidenceEventKind::NodePublished);
    assert_eq!(delivered[0].node, Optional::Present(node));
    assert_eq!(delivered[0].edge, Optional::Absent);
}

/// A subscription delivers only what was committed after it opened; everything earlier is
/// the frontier its own response carried.
///
/// The two halves are checked against each other rather than separately, because the property
/// the rule states is that they *partition*: an artifact reported as frontier is not also
/// delivered as a delta, so a client counting artifacts never doubles one.
#[test]
fn the_frontier_covers_what_came_before_and_the_frames_what_came_after() {
    let mut world = world(Encoding::CanonicalJson);
    let trace = world.trace.clone();
    let appended = ingest::<Json>(&mut world, &trace, "req_ingest", "idem-ingest");
    let node = ingested(&appended);
    assert!(
        deltas(&appended).is_empty(),
        "no subscription was open, so the delta reaches no frame"
    );

    let opened = subscribe::<Json>(&mut world, empty_scope(), "req_sub");
    assert_eq!(
        frontier(&opened),
        vec![node.clone()],
        "the node committed before the subscription is frontier, not a delta"
    );
    assert!(deltas(&opened).is_empty());

    // A second write, after the subscription: this one is a delta and not frontier.
    let verified = verify::<Json>(&mut world, &node, "req_verify", "idem-verify");
    let delivered = deltas(&verified);
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].kind, EvidenceEventKind::StatusTransition);
    assert_eq!(
        delivered[0].status,
        Optional::Present(EvidenceStatus::Observed)
    );
    assert_eq!(delivered[0].node, Optional::Present(node));
}

/// `evidence.link` commits two artifacts as one write, and the channel carries two deltas —
/// the receipt node and the check edge — each naming its own half.
///
/// The edge delta is the one worth having: it is the only place in the protocol where an
/// `EvidenceEvent` names an `edge`, and it exercises the branch of the scope predicate that
/// resolves an edge rather than a node.
#[test]
fn a_check_edge_and_its_receipt_arrive_as_two_deltas() {
    let mut world = world(Encoding::CanonicalJson);
    let trace = world.trace.clone();
    let subject = ingested(&ingest::<Json>(
        &mut world,
        &trace,
        "req_ingest",
        "idem-ingest",
    ));

    subscribe::<Json>(&mut world, empty_scope(), "req_sub");

    let receipt = world.other.clone();
    let linked = link::<Json>(&mut world, &subject, &receipt, "req_link", "idem-link");
    assert_eq!(result(&linked).status, ResultStatus::Ok);

    let delivered = deltas(&linked);
    assert_eq!(delivered.len(), 2, "one write, two committed artifacts");
    assert_eq!(delivered[0].kind, EvidenceEventKind::NodePublished);
    assert!(matches!(delivered[0].node, Optional::Present(_)));
    assert_eq!(delivered[1].kind, EvidenceEventKind::EdgePublished);
    assert!(matches!(delivered[1].edge, Optional::Present(_)));
    assert_eq!(
        delivered[1].node,
        Optional::Absent,
        "an edge delta names the edge, and the node member stays absent"
    );
}

/// The same deltas reach a client on either negotiated encoding.
///
/// "Both encodings carry the *same* canonical field order; JSON is for debugging and CBOR for
/// performance, and neither is a different protocol" (RFC 0026). An event frame is spelled in
/// the negotiated encoding like every other frame, so this is the check that the channel did
/// not quietly become a JSON-only surface.
#[test]
fn canonical_cbor_delivers_the_deltas_canonical_json_delivers() {
    let json = {
        let mut world = world(Encoding::CanonicalJson);
        subscribe::<Json>(&mut world, empty_scope(), "req_sub");
        let trace = world.trace.clone();
        deltas(&ingest::<Json>(
            &mut world,
            &trace,
            "req_ingest",
            "idem-ingest",
        ))
    };
    let cbor = {
        let mut world = world(Encoding::CanonicalCbor);
        subscribe::<Cbor>(&mut world, empty_scope(), "req_sub");
        let trace = world.trace.clone();
        deltas(&ingest::<Cbor>(
            &mut world,
            &trace,
            "req_ingest",
            "idem-ingest",
        ))
    };
    assert_eq!(json.len(), 1);
    assert_eq!(json, cbor, "one protocol, two spellings");
}

// =========================================================================================
// negative: the channel withholds
// =========================================================================================

/// A delta the declared scope does not select reaches no frame.
///
/// The scope names a claim identity nothing in the graph carries, so the node the ingest
/// commits is outside it. The subscription is open and the delta is committed; what does not
/// happen is the delivery, which is the difference between a channel and a firehose.
#[test]
fn a_delta_outside_the_declared_scope_reaches_no_frame() {
    let mut world = world(Encoding::CanonicalJson);
    let scoped = EvidenceQuery {
        claim_id: Optional::Present("no-claim-carries-this-identity".to_owned()),
        ..empty_scope()
    };
    let opened = subscribe::<Json>(&mut world, scoped, "req_sub");
    assert_eq!(result(&opened).status, ResultStatus::Ok);
    assert_eq!(world.server.subscriptions(), 1);

    let trace = world.trace.clone();
    let appended = ingest::<Json>(&mut world, &trace, "req_ingest", "idem-ingest");
    assert_eq!(result(&appended).status, ResultStatus::Ok);
    assert!(
        deltas(&appended).is_empty(),
        "the write committed; the scope did not select it"
    );
}

/// A scope that names a node kind the delta's artifact is not is a refusal too, and by the
/// same predicate `evidence.query` filters with.
#[test]
fn a_scope_naming_another_node_kind_reaches_no_frame() {
    let mut world = world(Encoding::CanonicalJson);
    let scoped = EvidenceQuery {
        // An ingested trace is a `run` node; a `patch` is a different kind.
        node_kinds: Optional::Present(vec![EvidenceNodeKind::Patch]),
        ..empty_scope()
    };
    subscribe::<Json>(&mut world, scoped, "req_sub");

    let trace = world.trace.clone();
    assert!(
        deltas(&ingest::<Json>(
            &mut world,
            &trace,
            "req_ingest",
            "idem-ingest"
        ))
        .is_empty()
    );
}

/// `rule evidence.traversal` at the byte boundary: `roots` and `max_depth` decide which
/// committed deltas reach a client, over a real connection.
///
/// `rule subscription.delivery` binds a scope to "the same predicate `evidence.query` answers
/// with", so the traversal IDL 1.8 declared is a subscription's selection too — and the place
/// that is worth checking is here, where the scope was decoded from bytes the server did not
/// construct. The append that lands one edge from the root is delivered; the append that
/// lands in no neighborhood of it is not.
#[test]
fn a_rooted_scope_delivers_the_deltas_its_traversal_reaches_and_no_others() {
    let mut world = world(Encoding::CanonicalJson);
    let trace = world.trace.clone();
    let other = world.other.clone();

    // The root exists before the subscription, so it is frontier rather than delta.
    let subject = ingested(&ingest::<Json>(
        &mut world,
        &trace,
        "req_ingest",
        "idem-ingest",
    ));
    let opened = subscribe::<Json>(
        &mut world,
        EvidenceQuery {
            roots: Optional::Present(vec![subject.clone()]),
            max_depth: Optional::Present(1),
            ..empty_scope()
        },
        "req_sub",
    );
    assert_eq!(result(&opened).status, ResultStatus::Ok);
    assert_eq!(frontier(&opened), vec![subject.clone()]);

    // An unrelated append: committed, and in no neighborhood of the root.
    let unrelated = ingest::<Json>(&mut world, &other, "req_other", "idem-other");
    assert_eq!(result(&unrelated).status, ResultStatus::Ok);
    assert!(
        deltas(&unrelated).is_empty(),
        "no edge connects this node to the root"
    );

    // The check: a receipt one edge from the root, and the edge that reaches it. Both are
    // inside `max_depth = 1` — the receipt at depth 1, the edge at the greater of its two
    // endpoints' depths, which is also 1.
    let checked = link::<Json>(&mut world, &subject, &other, "req_link", "idem-link");
    assert_eq!(result(&checked).status, ResultStatus::Ok);
    let kinds: Vec<EvidenceEventKind> = deltas(&checked).iter().map(|event| event.kind).collect();
    assert_eq!(
        kinds,
        [
            EvidenceEventKind::NodePublished,
            EvidenceEventKind::EdgePublished
        ],
        "the receipt and the edge that reaches it"
    );
}

/// The boundary, over the wire: `max_depth = 0` is exactly handle membership, so an artifact
/// one edge away reaches no frame.
///
/// This is the reading a pre-1.8 daemon served for every `roots`, and it is still expressible
/// — it is now the bound it always was rather than the whole of the field.
#[test]
fn a_zero_depth_scope_is_membership_and_a_new_neighbor_reaches_no_frame() {
    let mut world = world(Encoding::CanonicalJson);
    let trace = world.trace.clone();
    let other = world.other.clone();

    let subject = ingested(&ingest::<Json>(
        &mut world,
        &trace,
        "req_ingest",
        "idem-ingest",
    ));
    let opened = subscribe::<Json>(
        &mut world,
        EvidenceQuery {
            roots: Optional::Present(vec![subject.clone()]),
            max_depth: Optional::Present(0),
            ..empty_scope()
        },
        "req_sub",
    );
    assert_eq!(frontier(&opened), vec![subject.clone()]);

    let checked = link::<Json>(&mut world, &subject, &other, "req_link", "idem-link");
    assert_eq!(result(&checked).status, ResultStatus::Ok);
    assert!(
        deltas(&checked).is_empty(),
        "the receipt and the edge are one step out, and the scope declared none"
    );

    // A promotion of the root itself is still delivered: the bound excludes the neighborhood,
    // not the root.
    let verified = verify::<Json>(&mut world, &subject, "req_verify", "idem-verify");
    assert_eq!(result(&verified).status, ResultStatus::Ok);
    let kinds: Vec<EvidenceEventKind> = deltas(&verified).iter().map(|event| event.kind).collect();
    assert_eq!(kinds, [EvidenceEventKind::StatusTransition]);
}

/// A connection that never subscribed is sent no event frames, however much the graph grows.
///
/// The honest reading of "delivers `events` frames on the same connection": the frames belong
/// to a subscription, not to a connection, and a client that asked for none is owed none.
#[test]
fn a_connection_that_never_subscribed_is_sent_no_frames() {
    let mut world = world(Encoding::CanonicalJson);
    assert_eq!(world.server.subscriptions(), 0);

    let trace = world.trace.clone();
    let appended = ingest::<Json>(&mut world, &trace, "req_ingest", "idem-ingest");
    let node = ingested(&appended);
    assert!(deltas(&appended).is_empty());

    let verified = verify::<Json>(&mut world, &node, "req_verify", "idem-verify");
    assert!(deltas(&verified).is_empty());
}

/// A refused subscription opens no channel.
///
/// The capability is the connection's *root*, delegated to a producer whose actor does not
/// match the request's — admission refuses it, and the connection must not come away holding
/// a cursor. A transport that registered the scope before reading the outcome would give a
/// denied client a stream, which is the failure mode worth a test of its own.
#[test]
fn a_refused_subscription_opens_no_channel() {
    let mut world = world(Encoding::CanonicalJson);
    send::<Json>(
        &mut world,
        // `cap_observer` is bound to `agent:observer`; this request claims to be someone else.
        &envelope(
            "evidence.subscribe",
            "agent:impostor",
            "cap_observer",
            "req_sub",
        ),
        &Arguments::EvidenceSubscribe(EvidenceSubscribeRequest {
            scope: empty_scope(),
        }),
    );
    let refused = exchange::<Json>(&mut world, "evidence.subscribe");
    let envelope = result(&refused);
    assert_eq!(envelope.status, ResultStatus::Error);
    assert_eq!(
        envelope
            .error
            .value()
            .expect("an error result carries the error")
            .code,
        ErrorCode::CapabilityDenied
    );
    assert_eq!(
        world.server.subscriptions(),
        0,
        "a denial is not a subscription"
    );

    let trace = world.trace.clone();
    assert!(
        deltas(&ingest::<Json>(
            &mut world,
            &trace,
            "req_ingest",
            "idem-ingest"
        ))
        .is_empty()
    );
}

/// The two server-sent shapes never validate as each other.
///
/// > a frame and a `ResultEnvelope` are told apart by their required members, which is the
/// > device `ServerWelcome` and `ServerReject` already use.
/// >
/// > — `rule subscription.delivery`
///
/// So the discrimination is checked the way `transport_local.rs` checks the handshake pair:
/// from both directions, over real frames, with no tag to fall back on. `ResultEnvelope`
/// requires `request_id` and `status`; `EvidenceEvent` requires `at` and `kind`; neither
/// document can satisfy the other's requirements.
#[test]
fn an_event_frame_and_a_result_frame_never_validate_as_each_other() {
    let mut world = world(Encoding::CanonicalJson);
    subscribe::<Json>(&mut world, empty_scope(), "req_sub");

    let trace = world.trace.clone();
    send::<Json>(
        &mut world,
        &started(
            envelope(
                "observe.ingest",
                "agent:observer",
                "cap_observer",
                "req_ingest",
            ),
            "idem-ingest",
        ),
        &Arguments::ObserveIngest(ObserveIngestRequest {
            trace,
            instrumentation_profile: PROFILE.to_owned(),
        }),
    );
    world
        .server
        .serve(&mut world.pair)
        .expect("the exchange completes");

    let result_frame = world
        .pair
        .to_client
        .take_frame()
        .expect("a whole frame")
        .expect("the result comes first");
    let event_frame = world
        .pair
        .to_client
        .take_frame()
        .expect("a whole frame")
        .expect("the delta follows it");

    // Each decodes as itself.
    assert!(from_bytes::<ResultEnvelope>(&result_frame).is_ok());
    assert!(from_bytes::<EvidenceEvent>(&event_frame).is_ok());
    // And as nothing else.
    assert!(
        from_bytes::<EvidenceEvent>(&result_frame).is_err(),
        "a result frame carries no `at` and no `kind`"
    );
    assert!(
        from_bytes::<ResultEnvelope>(&event_frame).is_err(),
        "an event frame carries no `request_id` and no `status`"
    );
    // Which is what lets one reader classify both without being told which is which.
    assert!(matches!(
        decode_server_frame_in::<Json>("observe.ingest", &result_frame).expect("a shape"),
        ServerFrame::Result(..)
    ));
    assert!(matches!(
        decode_server_frame_in::<Json>("observe.ingest", &event_frame).expect("a shape"),
        ServerFrame::Event(_)
    ));
}

/// A frame that is neither shape is an error, not a silently-dropped byte string.
///
/// The classifier tries two declared shapes and reports the failure of the last one rather
/// than guessing at a third: "no best-effort decoding anywhere" (docs/09 T13) applies to a
/// frame's *kind* as much as to its contents.
#[test]
fn a_frame_of_neither_shape_is_a_typed_decode_failure() {
    assert!(decode_server_frame_in::<Json>("observe.ingest", b"{}").is_err());
    assert!(decode_server_frame_in::<Json>("observe.ingest", b"not a document").is_err());
}

// =========================================================================================
// boundary
// =========================================================================================

/// A delta two of one connection's scopes select is delivered once.
///
/// An event frame carries no subscription identity, so two frames naming one committed
/// artifact would be indistinguishable from two committed artifacts — the graph would look to
/// the client as if it had grown by two. The two scopes below both select the ingested node
/// (one names the whole graph, the other names its kind), and exactly one frame arrives.
#[test]
fn a_delta_two_scopes_select_is_delivered_once() {
    let mut world = world(Encoding::CanonicalJson);
    subscribe::<Json>(&mut world, empty_scope(), "req_sub_a");
    subscribe::<Json>(
        &mut world,
        EvidenceQuery {
            node_kinds: Optional::Present(vec![EvidenceNodeKind::Run]),
            ..empty_scope()
        },
        "req_sub_b",
    );
    assert_eq!(world.server.subscriptions(), 2);

    let trace = world.trace.clone();
    let delivered = deltas(&ingest::<Json>(
        &mut world,
        &trace,
        "req_ingest",
        "idem-ingest",
    ));
    assert_eq!(
        delivered.len(),
        1,
        "one committed artifact is one frame, however many scopes select it"
    );
}

/// Two subscriptions opened at different times partition the log between them the same way
/// one does: the later one's frontier covers what the earlier one was sent as frames.
///
/// > Reconnecting and re-reading MUST yield a superset of what the stream delivered.
/// >
/// > — RFC 0026, "A subscription is not a lock and not a transaction"
///
/// This is that sentence with the reconnection made explicit in [`a_dropped_connection_loses
/// _the_cursor_and_no_artifact`]; here it is the weaker in-connection form, which is what
/// makes the frontier/delta partition checkable without closing anything.
#[test]
fn a_later_subscription_sees_as_frontier_what_an_earlier_one_saw_as_frames() {
    let mut world = world(Encoding::CanonicalJson);
    subscribe::<Json>(&mut world, empty_scope(), "req_sub_a");

    let trace = world.trace.clone();
    let appended = ingest::<Json>(&mut world, &trace, "req_ingest", "idem-ingest");
    let node = ingested(&appended);
    assert_eq!(deltas(&appended).len(), 1, "the first stream got the delta");

    let later = subscribe::<Json>(&mut world, empty_scope(), "req_sub_b");
    assert_eq!(
        frontier(&later),
        vec![node],
        "and the later subscription reads the same artifact as frontier"
    );
    assert!(
        deltas(&later).is_empty(),
        "the first subscription's cursor is already past it"
    );
}

/// Closing a connection loses every cursor and no artifact.
///
/// [`Server::close`] consumes the connection and hands the daemon back, which is a dropped
/// subscription in the only form this transport has one. The reconnected client re-subscribes
/// and its frontier covers what the dropped stream had delivered — RFC 0026's "reconnecting
/// and re-reading MUST yield a superset of what the stream delivered", and INV-002's "a
/// dropped subscription changes nothing" from the other side.
#[test]
fn a_dropped_connection_loses_the_cursor_and_no_artifact() {
    let mut world = world(Encoding::CanonicalJson);
    subscribe::<Json>(&mut world, empty_scope(), "req_sub");
    let trace = world.trace.clone();
    let appended = ingest::<Json>(&mut world, &trace, "req_ingest", "idem-ingest");
    let node = ingested(&appended);
    let streamed: Vec<EvidenceHandle> = deltas(&appended)
        .into_iter()
        .filter_map(|event| event.node.value().cloned())
        .collect();
    assert_eq!(streamed, vec![node.clone()]);

    // The connection drops. The daemon — and the graph — outlive it.
    let mut reconnected = World {
        server: Server::new(world.server.close(), negotiated(Encoding::CanonicalJson)),
        pair: LocalPair::new(),
        trace: world.trace,
        other: world.other,
    };
    assert_eq!(
        reconnected.server.subscriptions(),
        0,
        "cursors do not survive"
    );

    let opened = subscribe::<Json>(&mut reconnected, empty_scope(), "req_resub");
    let recovered = frontier(&opened);
    assert_eq!(recovered, vec![node]);
    for handle in &streamed {
        assert!(
            recovered.contains(handle),
            "re-reading yields a superset of what the stream delivered"
        );
    }
}

/// A scope clause is read against the graph as it stands when the frame is written, not
/// against the graph at the instant of the delta.
///
/// Both writes are dispatched before any delivery pass runs, so both deltas are classified
/// after the promotion landed. A scope naming `statuses: [observed]` therefore selects
/// **both** — the `node_published` delta included, though the node stood at the lattice's
/// bottom when that delta was committed. That is `rule subscription.delivery`'s clause 3 and
/// it is the honest reading: the graph is authoritative and a delta is a pointer into it,
/// never a copy of it, so a scope follows the claim rather than freezing a status the claim
/// has since left behind.
///
/// The two requests are answered through [`Server::answer`] rather than [`Server::serve`]
/// because `serve` writes frames after each request, which is a choice about *when* to
/// deliver and not about what a scope means. Separating them is what lets this test say
/// which of the two the assertion is about; the other choice is the next test's subject.
#[test]
fn a_scope_clause_is_read_against_the_graph_when_the_frame_is_written() {
    let mut world = world(Encoding::CanonicalJson);
    let trace = world.trace.clone();
    let node = node_identity();

    subscribe::<Json>(
        &mut world,
        EvidenceQuery {
            statuses: Optional::Present(vec![EvidenceStatus::Observed]),
            ..empty_scope()
        },
        "req_sub",
    );

    send::<Json>(
        &mut world,
        &started(
            envelope(
                "observe.ingest",
                "agent:observer",
                "cap_observer",
                "req_ingest",
            ),
            "idem-ingest",
        ),
        &Arguments::ObserveIngest(ObserveIngestRequest {
            trace,
            instrumentation_profile: PROFILE.to_owned(),
        }),
    );
    send::<Json>(
        &mut world,
        &started(
            envelope(
                "evidence.verify",
                "agent:reader",
                "cap_reader",
                "req_verify",
            ),
            "idem-verify",
        ),
        &Arguments::EvidenceVerify(EvidenceVerifyRequest {
            evidence: node.clone(),
            expected_status: Optional::Absent,
        }),
    );
    // Both dispatched, neither delivered: the log now holds two deltas and the graph holds an
    // observed node.
    while let Some(frame) = world.pair.to_server.take_frame().expect("a whole frame") {
        let answered = world
            .server
            .answer(&frame)
            .expect("the request is answered");
        assert_eq!(
            from_bytes::<ResultEnvelope>(&answered)
                .expect("a result frame")
                .status,
            ResultStatus::Ok
        );
    }

    let delivered: Vec<EvidenceEvent> = world
        .server
        .deliver()
        .expect("delivery encodes")
        .iter()
        .map(|frame| from_bytes::<EvidenceEvent>(frame).expect("an event frame"))
        .collect();
    assert_eq!(
        delivered
            .iter()
            .map(|event| event.kind)
            .collect::<Vec<EvidenceEventKind>>(),
        vec![
            EvidenceEventKind::NodePublished,
            EvidenceEventKind::StatusTransition
        ],
        "the publish delta is in an `observed` scope because the node is observed by the \
         time the frame is written"
    );
    assert!(
        delivered
            .iter()
            .all(|event| event.node == Optional::Present(node.clone()))
    );
}

/// A delta the scope did not select when it was examined is not reconsidered.
///
/// The other half of clause 3, and the limit it names out loud: the predicate is evaluated
/// once per delta per subscription, at the first delivery after that delta was committed. Here
/// `serve` delivers after each request, so the `node_published` delta is examined while the
/// node still stands at `proposed` — outside a `statuses: [observed]` scope — and the
/// promotion that would have brought it inside arrives too late. The stream is silent about
/// it forever.
///
/// That is not a bug to be fixed by holding unselected deltas back: a subscription is a
/// cursor, not a filter re-run over history, and the thing that makes the limit acceptable is
/// asserted here too — the artifact is still reachable by re-reading, which is what
/// `rule subscription.hints_only` means by "the graph itself remains authoritative".
#[test]
fn a_delta_the_scope_did_not_select_when_examined_is_not_reconsidered() {
    let mut world = world(Encoding::CanonicalJson);
    subscribe::<Json>(
        &mut world,
        EvidenceQuery {
            statuses: Optional::Present(vec![EvidenceStatus::Observed]),
            ..empty_scope()
        },
        "req_sub",
    );

    let trace = world.trace.clone();
    let appended = ingest::<Json>(&mut world, &trace, "req_ingest", "idem-ingest");
    let node = ingested(&appended);
    assert!(
        deltas(&appended).is_empty(),
        "a freshly appended node is `proposed`, which the scope does not name"
    );

    let verified = verify::<Json>(&mut world, &node, "req_verify", "idem-verify");
    assert_eq!(
        deltas(&verified)
            .iter()
            .map(|event| event.kind)
            .collect::<Vec<EvidenceEventKind>>(),
        vec![EvidenceEventKind::StatusTransition],
        "the promotion is in scope; the publish delta it would have brought inside is past"
    );

    // And the recovery the rule points at: the artifact the stream never named is frontier to
    // a subscription opened now, so nothing was lost — only unsent.
    let reopened = subscribe::<Json>(
        &mut world,
        EvidenceQuery {
            statuses: Optional::Present(vec![EvidenceStatus::Observed]),
            ..empty_scope()
        },
        "req_resub",
    );
    assert_eq!(frontier(&reopened), vec![node]);
}

/// A delta whose kind's member is absent names no artifact, and is in no scope.
///
/// Unreachable through any operation — every delta this daemon records names its artifact —
/// so it is injected directly, which is the only way to check a refusal that exists to stop a
/// future write from smuggling an unnamed delta past a scope. The scope below is the widest
/// one there is; an event naming nothing is still outside it, because "in scope" is a property
/// of the artifact a delta names and not of the delta's mere existence.
#[test]
fn a_delta_naming_no_artifact_is_in_no_scope() {
    let mut world = world(Encoding::CanonicalJson);
    subscribe::<Json>(&mut world, empty_scope(), "req_sub");

    world
        .server
        .daemon_mut()
        .state_mut()
        .record_evidence_event(EvidenceEvent {
            at: now(),
            kind: EvidenceEventKind::NodePublished,
            node: Optional::Absent,
            edge: Optional::Absent,
            status: Optional::Absent,
            claim_id: Optional::Present("a-claim".to_owned()),
        });

    assert!(
        world.server.deliver().expect("delivery encodes").is_empty(),
        "a delta that names no artifact reaches no scope"
    );
}

/// Draining twice yields the frames once.
///
/// The cursor is the thing that makes a subscription a subscription: without it a client that
/// stayed connected would be re-sent the whole log after every request, and "a delta is
/// delivered once" would hold only for a client that never asked twice.
#[test]
fn a_second_drain_yields_nothing() {
    let mut world = world(Encoding::CanonicalJson);
    subscribe::<Json>(&mut world, empty_scope(), "req_sub");
    let trace = world.trace.clone();
    assert_eq!(
        deltas(&ingest::<Json>(
            &mut world,
            &trace,
            "req_ingest",
            "idem-ingest"
        ))
        .len(),
        1
    );
    assert!(world.server.deliver().expect("delivery encodes").is_empty());
    assert!(world.server.deliver().expect("delivery encodes").is_empty());
}

// =========================================================================================
// negative: a subscription does not outlive its standing (cr-3hcpn4)
// =========================================================================================

mod standing {
    //! A subscription outlives the request that opened it, so delivery re-decides T4 for the
    //! subscribing capability before every pass: registered, bound to its actor, unexpired
    //! at the daemon's latest time reading, and descended from the connection's capability
    //! through hops that all still exist and still narrow. Each test opens a subscription
    //! under a two-hop delegate (`cap_root` -> `cap_reader` -> `cap_sub`), shows one delta
    //! delivered as the control, changes the chain *between a commit and its delivery*, and
    //! shows the next delta is not delivered and the subscription is closed with its typed
    //! reason.

    use continuumd::daemon::OperationRequest;
    use continuumd::transport::SubscriptionClosed;

    use super::{
        Arguments, AuthorityLevel, Encoding, EvidenceSubscribeRequest, Json, Nullable,
        ObserveIngestRequest, Optional, PROFILE, ResultStatus, Timestamp, World, cap, empty_scope,
        envelope, exchange, grant, result, send, started, world,
    };

    /// Register `cap_sub` under `cap_reader`, subscribe with it, and deliver one delta.
    fn subscribed(expires_at: Option<&str>) -> World {
        let mut world = world(Encoding::CanonicalJson);
        let mut sub = grant(
            "cap_sub",
            "agent:sub",
            AuthorityLevel::Read,
            Optional::Absent,
        );
        sub.delegation_depth = 2;
        if let Some(at) = expires_at {
            sub.expires_at = Nullable::Value(Timestamp::new(at).expect("a timestamp"));
        }
        world
            .server
            .daemon_mut()
            .state_mut()
            .register_capability(sub, Some(cap("cap_reader")))
            .expect("a well-formed delegate");
        send::<Json>(
            &mut world,
            &envelope("evidence.subscribe", "agent:sub", "cap_sub", "req_sub"),
            &Arguments::EvidenceSubscribe(EvidenceSubscribeRequest {
                scope: empty_scope(),
            }),
        );
        let opened = exchange::<Json>(&mut world, "evidence.subscribe");
        assert_eq!(result(&opened).status, ResultStatus::Ok);

        // The control: a delta committed now is delivered.
        let trace = world.trace.clone();
        commit(&mut world, &trace, "req_commit_control", "idem-control");
        assert_eq!(
            world.server.deliver().expect("delivery encodes").len(),
            1,
            "the delegate's subscription delivers while its chain stands"
        );
        world
    }

    /// Commit an ingest without delivering, so the chain can change in between.
    fn commit(
        world: &mut World,
        trace: &continuumd::protocol::scalar::Commitment,
        request: &str,
        key: &str,
    ) {
        let outcome = world.server.daemon_mut().dispatch(&OperationRequest {
            envelope: started(
                envelope("observe.ingest", "agent:observer", "cap_observer", request),
                key,
            ),
            arguments: Arguments::ObserveIngest(ObserveIngestRequest {
                trace: trace.clone(),
                instrumentation_profile: PROFILE.to_owned(),
            }),
        });
        assert_eq!(outcome.error_code(), None, "the ingest commits");
    }

    fn assert_closed(world: &mut World) {
        let other = world.other.clone();
        commit(world, &other, "req_commit_after", "idem-after");
        assert!(
            world.server.deliver().expect("delivery encodes").is_empty(),
            "a subscription whose standing is gone delivers nothing"
        );
        assert_eq!(world.server.subscriptions(), 0, "and it is closed");
        assert_eq!(
            world.server.closed_subscriptions(),
            [SubscriptionClosed::StandingLost],
            "with its typed reason"
        );
        assert_eq!(world.server.suppressed_deliveries().standing_lost, 1);
    }

    /// Revoking the parent revokes the delegate (D8), for delivery as for requests.
    #[test]
    fn revoking_the_parent_stops_delivery_to_the_delegate() {
        let mut world = subscribed(None);
        world
            .server
            .daemon_mut()
            .state_mut()
            .revoke_capability(&cap("cap_reader"));
        assert!(
            world
                .server
                .daemon()
                .state()
                .grant(&cap("cap_sub"))
                .is_some(),
            "the leaf itself is still registered: only the chain is broken"
        );
        assert_closed(&mut world);
    }

    /// An expiry passed between commit and delivery, judged at the deployment's new
    /// reading (INV-005: the daemon reads no clock of its own).
    #[test]
    fn an_expiry_passed_before_delivery_stops_it() {
        let mut world = subscribed(Some("2026-08-02T00:00:00.000Z"));
        world
            .server
            .daemon_mut()
            .set_now(Timestamp::new("2026-08-03T00:00:00.000Z").expect("a timestamp"));
        assert_closed(&mut world);
    }

    /// A parent re-provisioned narrower than its delegate no longer admits it.
    #[test]
    fn a_parent_narrowed_below_the_delegate_stops_delivery() {
        let mut world = subscribed(None);
        let mut narrowed = grant(
            "cap_reader",
            "agent:reader",
            AuthorityLevel::Read,
            Optional::Absent,
        );
        narrowed.artifact_classes = vec!["ws".to_owned()];
        world
            .server
            .daemon_mut()
            .state_mut()
            .register_capability(narrowed, Some(cap("cap_root")))
            .expect("a well-formed parent");
        assert_closed(&mut world);
    }

    /// The same token re-registered narrower is decided at the next delivery by the whole
    /// admission of `evidence.subscribe`, not only its standing (cr-3hcpn4): a grant that
    /// lost the `ev` class, or whose profile now denies the operation, is closed before a
    /// frame. The control re-registers it unchanged and keeps delivering.
    #[test]
    fn a_grant_re_registered_narrower_closes_its_subscription() {
        enum Narrowing {
            Unchanged,
            ClassRemoved,
            OperationDenied,
        }
        for narrowing in [
            Narrowing::Unchanged,
            Narrowing::ClassRemoved,
            Narrowing::OperationDenied,
        ] {
            let mut world = subscribed(None);
            let mut replaced = grant(
                "cap_sub",
                "agent:sub",
                AuthorityLevel::Read,
                Optional::Absent,
            );
            replaced.delegation_depth = 2;
            match narrowing {
                Narrowing::Unchanged => {}
                Narrowing::ClassRemoved => replaced.artifact_classes = vec!["ws".to_owned()],
                Narrowing::OperationDenied => {
                    replaced.profile =
                        Optional::Present(continuumd::protocol::handshake::CapabilityProfile {
                            privileged_operations: Vec::new(),
                            denied_operations: vec![
                                continuumd::protocol::scalar::OperationName::new(
                                    "evidence.subscribe",
                                )
                                .expect("an operation"),
                            ],
                            data_grants: Vec::new(),
                            cross_principal_sharing: false,
                        });
                }
            }
            let unchanged = matches!(narrowing, Narrowing::Unchanged);
            world
                .server
                .daemon_mut()
                .state_mut()
                .register_capability(replaced, Some(cap("cap_reader")))
                .expect("a well-formed delegate");
            let other = world.other.clone();
            commit(&mut world, &other, "req_commit_after", "idem-after");
            let frames = world.server.deliver().expect("delivery encodes");
            assert_eq!(
                frames.len(),
                usize::from(unchanged),
                "a frame only for the control"
            );
            assert_eq!(world.server.subscriptions(), usize::from(unchanged));
            assert_eq!(
                world.server.suppressed_deliveries().standing_lost,
                u64::from(!unchanged)
            );
        }
    }

    /// A delegated grant used as the connection's own capability is held to its stored
    /// chain too (D8, R1; cr-3hcpn4). `cap_observer` is a child of `cap_root` and is the
    /// connection capability here. After its parent is revoked, re-provisioned narrower,
    /// or it expires, a request on the connection is denied, a delta is not delivered and
    /// the subscription is closed, and a new handshake presenting it is refused.
    #[test]
    fn a_delegated_connection_capability_does_not_outlive_its_parent() {
        use continuumd::daemon::capability::ConnectionPolicy;
        use continuumd::protocol::handshake::{ClientHello, ServerLimits, VersionRange};
        use continuumd::protocol::scalar::{ByteCount, DurationMs};

        enum Change {
            RevokeParent,
            NarrowParent,
            Expire,
        }
        for change in [Change::RevokeParent, Change::NarrowParent, Change::Expire] {
            let mut world = super::world_on(Encoding::CanonicalJson, "cap_observer");
            if matches!(change, Change::Expire) {
                let mut expiring = grant(
                    "cap_observer",
                    "agent:observer",
                    AuthorityLevel::Execute,
                    Optional::Present(super::traced(&[super::DataGrant::ProductionTrace])),
                );
                expiring.expires_at =
                    Nullable::Value(Timestamp::new("2026-08-02T00:00:00.000Z").expect("a time"));
                world
                    .server
                    .daemon_mut()
                    .state_mut()
                    .register_capability(expiring, Some(cap("cap_root")))
                    .expect("a well-formed delegate");
            }
            send::<Json>(
                &mut world,
                &envelope(
                    "evidence.subscribe",
                    "agent:observer",
                    "cap_observer",
                    "req_sub",
                ),
                &Arguments::EvidenceSubscribe(EvidenceSubscribeRequest {
                    scope: empty_scope(),
                }),
            );
            let opened = exchange::<Json>(&mut world, "evidence.subscribe");
            assert_eq!(
                result(&opened).status,
                ResultStatus::Ok,
                "the delegate's own connection"
            );
            let trace = world.trace.clone();
            commit(&mut world, &trace, "req_commit_control", "idem-control");
            assert_eq!(world.server.deliver().expect("delivery encodes").len(), 1);

            match change {
                Change::RevokeParent => world
                    .server
                    .daemon_mut()
                    .state_mut()
                    .revoke_capability(&cap("cap_root")),
                Change::NarrowParent => {
                    let mut narrowed = grant(
                        "cap_root",
                        "service:continuumd",
                        AuthorityLevel::Promote,
                        Optional::Present(super::traced(&[super::DataGrant::ProductionTrace])),
                    );
                    narrowed.delegation_depth = 4;
                    narrowed.artifact_classes = vec!["ws".to_owned()];
                    world
                        .server
                        .daemon_mut()
                        .state_mut()
                        .register_capability(narrowed, None)
                        .expect("a well-formed root");
                }
                Change::Expire => world
                    .server
                    .daemon_mut()
                    .set_now(Timestamp::new("2026-08-03T00:00:00.000Z").expect("a time")),
            }

            // The next request on the connection is denied.
            let other = world.other.clone();
            let denied = world.server.daemon_mut().dispatch(&OperationRequest {
                envelope: started(
                    envelope(
                        "observe.ingest",
                        "agent:observer",
                        "cap_observer",
                        "req_after",
                    ),
                    "idem-after",
                ),
                arguments: Arguments::ObserveIngest(ObserveIngestRequest {
                    trace: other,
                    instrumentation_profile: PROFILE.to_owned(),
                }),
            });
            assert_eq!(
                denied.error_code(),
                Some(super::ErrorCode::CapabilityDenied)
            );

            // A delta committed by the root's own authority is not delivered.
            if matches!(change, Change::Expire) {
                // The root still stands; commit under a root-connection daemon is not
                // reachable from this connection, so the log is unchanged and the pass
                // below decides only the subscription's standing.
            }
            assert!(world.server.deliver().expect("delivery encodes").is_empty());
            assert_eq!(world.server.subscriptions(), 0);
            assert_eq!(
                world.server.closed_subscriptions(),
                [SubscriptionClosed::StandingLost]
            );

            // A new handshake presenting the delegate is refused.
            let policy = ConnectionPolicy::new(
                vec![super::version()],
                continuum_value::epoch::ProtocolWindow::new(3),
                continuumd::protocol::registry::ENCODINGS.to_vec(),
                ServerLimits {
                    idempotency_retention_ms: DurationMs::new(86_400_000),
                    max_page_size: 100,
                    max_result_bytes: ByteCount::new(1_048_576),
                    max_concurrent_tasks: 4,
                },
                "continuumd-evidence-subscription".to_owned(),
            );
            let hello = ClientHello {
                protocol_versions: VersionRange {
                    low: super::version(),
                    high: super::version(),
                },
                encodings: vec![Encoding::CanonicalJson],
                client: "continuumd-evidence-subscription".to_owned(),
                actor: super::who("agent:observer"),
                capability: cap("cap_observer"),
                features: Optional::Absent,
            };
            assert!(
                world.server.daemon().welcome(&policy, &hello).is_err(),
                "a delegate whose chain is gone is not welcomed"
            );
        }
    }
}

// =========================================================================================
// the derived-handle sweep: evidence and observe (cr-3hcpn4)
// =========================================================================================

mod derived {
    //! Every handle an evidence or observe operation reaches without the request naming it is
    //! decided against the grant before it is used. Each test runs one table: two grants that
    //! differ only in whether the derived handle is in scope. The out-of-scope grant is refused
    //! `CapabilityDenied` and leaves no trace — nothing appended, nothing published — and the
    //! in-scope grant is served. The handles are content identities, so they are learned once
    //! on a scratch world and are the same on the world under test.

    use continuumd::daemon::OperationRequest;
    use continuumd::daemon::family::Payload;
    use continuumd::protocol::operations::evidence::{
        EvidenceGetRequest, EvidenceLinkRequest, EvidenceQueryRequest, EvidenceSubscribeRequest,
    };

    use super::{
        Arguments, AuthorityLevel, CapabilityDescriptor, DataGrant, Encoding, ErrorCode,
        EvidenceHandle, EvidenceQuery, Json, ObserveIngestRequest, Optional, PROFILE, World, cap,
        empty_scope, envelope, grant, ingest, ingested, link, started, traced, world, world_with,
    };

    /// A grant under `cap_root`, instance-scoped to `listed`.
    fn scoped(
        capability: &str,
        actor: &str,
        level: AuthorityLevel,
        listed: &[&str],
    ) -> CapabilityDescriptor {
        let mut descriptor = grant(
            capability,
            actor,
            level,
            Optional::Present(traced(&[DataGrant::ProductionTrace])),
        );
        descriptor.delegation_depth = 2;
        descriptor.instances = Optional::Present(
            listed
                .iter()
                .map(|text| {
                    continuumd::protocol::scalar::ArtifactHandle::new(text).expect("a handle")
                })
                .collect(),
        );
        descriptor
    }

    /// The node an ingest of the first trace lands on, and the receipt node and edge a link
    /// of the second trace to it lands on.
    struct Learned {
        node: EvidenceHandle,
        receipt: EvidenceHandle,
        edge: EvidenceHandle,
    }

    fn linked(world: &mut World) -> Learned {
        let trace = world.trace.clone();
        let other = world.other.clone();
        let node = ingested(&ingest::<Json>(world, &trace, "req_ingest", "idem-ingest"));
        let frames = link::<Json>(world, &node, &other, "req_link", "idem-link");
        let (edge, receipt) = frames
            .iter()
            .find_map(|frame| match frame {
                super::ServerFrame::Result(_, payload) => match payload.as_ref() {
                    Payload::EvidenceLink(body) => Some((body.edge.clone(), body.receipt.clone())),
                    _ => None,
                },
                super::ServerFrame::Event(_) => None,
            })
            .expect("the link answers its edge and receipt");
        Learned {
            node,
            receipt,
            edge,
        }
    }

    fn learned() -> Learned {
        linked(&mut world(Encoding::CanonicalJson))
    }

    fn register(world: &mut World, descriptor: CapabilityDescriptor) {
        world
            .server
            .daemon_mut()
            .state_mut()
            .register_capability(descriptor, Some(cap("cap_root")))
            .expect("a well-formed delegate");
    }

    fn get(
        world: &mut World,
        capability: &str,
        actor: &str,
        evidence: &EvidenceHandle,
    ) -> continuumd::daemon::OperationOutcome {
        world.server.daemon_mut().dispatch(&OperationRequest {
            envelope: envelope("evidence.get", actor, capability, "req_get"),
            arguments: Arguments::EvidenceGet(EvidenceGetRequest {
                evidence: evidence.clone(),
                inline: Optional::Absent,
            }),
        })
    }

    /// `evidence.get` reports an edge's endpoints and a record's handle-spelled provenance
    /// inputs only when the grant admits them.
    #[test]
    fn evidence_get_reports_endpoints_and_provenance_only_in_scope() {
        let mut world = world(Encoding::CanonicalJson);
        let handles = linked(&mut world);

        // A node whose provenance names a `task_`, beside the real ones.
        let mut synthetic = world
            .server
            .daemon()
            .state()
            .evidence(&handles.node)
            .expect("the ingested node")
            .clone();
        synthetic.inputs = vec!["task_elsewhere".to_owned()];
        let synthetic_handle = EvidenceHandle::new("ev_synthetic").expect("a handle");
        world
            .server
            .daemon_mut()
            .state_mut()
            .append_evidence(synthetic_handle.clone(), synthetic);

        let edge = handles.edge.as_str();
        let node = handles.node.as_str();
        let receipt = handles.receipt.as_str();
        // (capability, the handle read, what the grant lists, served?)
        let table: [(&str, &EvidenceHandle, Vec<&str>, bool); 4] = [
            ("cap_edgeout", &handles.edge, vec![edge, node], false),
            ("cap_edgein", &handles.edge, vec![edge, node, receipt], true),
            ("cap_provout", &synthetic_handle, vec!["task_mine"], false),
            (
                "cap_provin",
                &synthetic_handle,
                vec!["task_elsewhere"],
                true,
            ),
        ];
        for (capability, read, listed, served) in table {
            register(
                &mut world,
                scoped(capability, "agent:reader", AuthorityLevel::Read, &listed),
            );
            let outcome = get(&mut world, capability, "agent:reader", read);
            if served {
                assert_eq!(outcome.error_code(), None, "{capability}");
                assert!(matches!(outcome.payload, Payload::EvidenceGet(_)));
            } else {
                assert_eq!(
                    outcome.error_code(),
                    Some(ErrorCode::CapabilityDenied),
                    "{capability}"
                );
                assert_eq!(outcome.payload, Payload::None, "{capability}: no record");
            }
        }
    }

    /// `evidence.link` decides the receipt node and the edge it would append before any
    /// lookup, and `observe.ingest` decides the node it would append.
    #[test]
    fn link_and_ingest_append_only_derived_nodes_and_edges_in_scope() {
        let handles = learned();
        let (node, receipt, edge) = (
            handles.node.as_str(),
            handles.receipt.as_str(),
            handles.edge.as_str(),
        );
        let mut world = world_with(
            Encoding::CanonicalJson,
            "cap_root",
            vec![
                scoped(
                    "cap_ingestout",
                    "agent:observer",
                    AuthorityLevel::Execute,
                    &["ev_elsewhere"],
                ),
                scoped(
                    "cap_ingestin",
                    "agent:observer",
                    AuthorityLevel::Execute,
                    &[node],
                ),
                scoped(
                    "cap_linkout",
                    "service:kernel-core",
                    AuthorityLevel::Execute,
                    &[node],
                ),
                scoped(
                    "cap_linknoreceipt",
                    "service:kernel-core",
                    AuthorityLevel::Execute,
                    &[node, edge],
                ),
                scoped(
                    "cap_linknoedge",
                    "service:kernel-core",
                    AuthorityLevel::Execute,
                    &[node, receipt],
                ),
                scoped(
                    "cap_linkin",
                    "service:kernel-core",
                    AuthorityLevel::Execute,
                    &[node, receipt, edge],
                ),
            ],
        );
        let ingest_as = |world: &mut World, capability: &str, key: &str| {
            let trace = world.trace.clone();
            world.server.daemon_mut().dispatch(&OperationRequest {
                envelope: started(
                    envelope("observe.ingest", "agent:observer", capability, key),
                    key,
                ),
                arguments: Arguments::ObserveIngest(ObserveIngestRequest {
                    trace,
                    instrumentation_profile: PROFILE.to_owned(),
                }),
            })
        };
        let link_as = |world: &mut World, capability: &str, key: &str| {
            let receipt_trace = world.other.clone();
            let mut sent = envelope("evidence.link", "service:kernel-core", capability, key);
            sent.idempotency_key = Optional::Present(key.to_owned());
            world.server.daemon_mut().dispatch(&OperationRequest {
                envelope: sent,
                arguments: Arguments::EvidenceLink(EvidenceLinkRequest {
                    subject: handles.node.clone(),
                    receipt: receipt_trace,
                    checker_profile: "kernel-core/1".to_owned(),
                }),
            })
        };
        let trace_of = |world: &World| {
            (
                world.server.daemon().store_audit().len(),
                world.server.daemon().state().evidence_events().len(),
            )
        };

        // observe.ingest: the node it would append.
        let before = trace_of(&world);
        let refused = ingest_as(&mut world, "cap_ingestout", "req_ingest_out");
        assert_eq!(refused.error_code(), Some(ErrorCode::CapabilityDenied));
        assert!(
            world
                .server
                .daemon()
                .state()
                .evidence(&handles.node)
                .is_none()
        );
        assert_eq!(
            trace_of(&world),
            before,
            "nothing published, nothing appended"
        );
        let served = ingest_as(&mut world, "cap_ingestin", "req_ingest_in");
        assert_eq!(served.error_code(), None);
        assert!(
            world
                .server
                .daemon()
                .state()
                .evidence(&handles.node)
                .is_some()
        );

        // evidence.link: the receipt node and the edge it would append, each left out alone.
        for capability in ["cap_linkout", "cap_linknoreceipt", "cap_linknoedge"] {
            let before = trace_of(&world);
            let refused = link_as(&mut world, capability, &format!("req_{capability}"));
            assert_eq!(
                refused.error_code(),
                Some(ErrorCode::CapabilityDenied),
                "{capability}"
            );
            let state = world.server.daemon().state();
            assert!(state.evidence(&handles.receipt).is_none(), "{capability}");
            assert!(state.evidence_edge(&handles.edge).is_none(), "{capability}");
            assert_eq!(
                trace_of(&world),
                before,
                "{capability}: nothing published, nothing appended"
            );
        }
        let served = link_as(&mut world, "cap_linkin", "req_link_in");
        assert_eq!(served.error_code(), None);
        assert!(
            world
                .server
                .daemon()
                .state()
                .evidence_edge(&handles.edge)
                .is_some()
        );
    }

    /// `evidence.query` and `evidence.subscribe` walk only edges in scope with both
    /// endpoints, so a node reachable only through an out-of-scope edge is not reached.
    #[test]
    fn a_traversal_reaches_nothing_through_an_out_of_scope_edge() {
        let mut world = world(Encoding::CanonicalJson);
        let handles = linked(&mut world);
        let (node, receipt, edge) = (
            handles.node.as_str(),
            handles.receipt.as_str(),
            handles.edge.as_str(),
        );
        let scope = EvidenceQuery {
            roots: Optional::Present(vec![handles.node.clone()]),
            max_depth: Optional::Present(1),
            ..empty_scope()
        };
        // (capability, what the grant lists, is the receipt reached?)
        let table = [
            ("cap_walkout", vec![node, receipt], false),
            ("cap_walkin", vec![node, receipt, edge], true),
        ];
        for (capability, listed, reached) in table {
            register(
                &mut world,
                scoped(capability, "agent:reader", AuthorityLevel::Read, &listed),
            );
            let queried = world.server.daemon_mut().dispatch(&OperationRequest {
                envelope: envelope("evidence.query", "agent:reader", capability, "req_query"),
                arguments: Arguments::EvidenceQuery(EvidenceQueryRequest {
                    query: scope.clone(),
                }),
            });
            let (nodes, edges) = match queried.payload {
                Payload::EvidenceQuery(body) => (body.nodes, body.edges),
                other => panic!("expected an evidence.query payload, got {other:?}"),
            };
            assert!(nodes.contains(&handles.node), "{capability}: the root");
            assert_eq!(nodes.contains(&handles.receipt), reached, "{capability}");
            assert_eq!(edges.contains(&handles.edge), reached, "{capability}");

            let subscribed = world.server.daemon_mut().dispatch(&OperationRequest {
                envelope: envelope("evidence.subscribe", "agent:reader", capability, "req_sub"),
                arguments: Arguments::EvidenceSubscribe(EvidenceSubscribeRequest {
                    scope: scope.clone(),
                }),
            });
            let frontier = match subscribed.payload {
                Payload::EvidenceSubscribe(body) => body.frontier,
                other => panic!("expected an evidence.subscribe payload, got {other:?}"),
            };
            assert_eq!(frontier.contains(&handles.receipt), reached, "{capability}");
        }
    }

    /// A delta outside an open subscription's instance scope is withheld and counted, with
    /// nothing on the wire.
    #[test]
    fn a_withheld_delta_is_counted_and_not_sent() {
        let handles = learned();
        let mut world = world(Encoding::CanonicalJson);
        register(
            &mut world,
            scoped(
                "cap_narrow",
                "agent:reader",
                AuthorityLevel::Read,
                &["ev_elsewhere"],
            ),
        );
        super::send::<Json>(
            &mut world,
            &envelope(
                "evidence.subscribe",
                "agent:reader",
                "cap_narrow",
                "req_sub",
            ),
            &Arguments::EvidenceSubscribe(EvidenceSubscribeRequest {
                scope: empty_scope(),
            }),
        );
        let opened = super::exchange::<Json>(&mut world, "evidence.subscribe");
        assert_eq!(super::result(&opened).status, super::ResultStatus::Ok);
        let trace = world.trace.clone();
        let appended = ingest::<Json>(&mut world, &trace, "req_ingest", "idem-ingest");
        assert_eq!(ingested(&appended), handles.node);
        assert!(
            super::deltas(&appended).is_empty(),
            "the node is outside the scope"
        );
        let counted = world.server.suppressed_deliveries();
        assert_eq!(counted.out_of_scope, 1);
        assert_eq!(counted.standing_lost, 0);
    }
}
