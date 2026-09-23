//! Dedicated exit evidence for `PR-7-EXIT` (`notes/plan/notes/PLAN_REQUIREMENTS.json`, id
//! `PR-7-EXIT`; `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 7's Exit line).
//!
//! > **Exit:** an untrusted client cannot promote a proposal to validated/proved.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 7
//!
//! This file asserts that sentence at its own grain, the way `pr6_exit_evidence.rs` did for
//! PR 6's exit (bn-1n6r) and `pr8_exit_evidence.rs` did for PR 8's: one named witness per
//! clause, every principal a *real* principal — an admitted capability over
//! [`Daemon::dispatch`], never a unit call into a handler — and the exhaustive half cited
//! rather than re-run. The sentence is INV-004's client-facing face, which makes this exit
//! unusual in one way: the evidence discipline the file must satisfy (independent, able to
//! fail, not self-certifying) is the very property it is about.
//!
//! # The promotion paths, enumerated from the governing texts (never invented)
//!
//! "Cannot promote" is a universal over paths, so the paths are read off the IDL and
//! RFC 0038 rather than imagined. A client-reachable promotion would have to be one of:
//!
//! | # | Spelled path | Governing sentence | Refusal | Witness |
//! |---|---|---|---|---|
//! | P0 | a direct status write — some operation whose request carries a status into the graph | the IDL declares every operation; `evidence.verify`: "The daemon MUST NOT accept a client-declared status" | **no such operation exists** — the closed scan over every request shape | [`the_idl_admits_no_client_declared_status_write_anywhere`] |
//! | P1/P2 | the compare-and-set guard used as a write: `expected_status: validated` (or `proved`) on `evidence.verify` | the guard "is never the status written" (RFC 0038 write model; the IDL's own field doc) | `StatusConflict`, nothing written | [`exit_an_untrusted_client_cannot_promote_a_proposal_to_validated_or_proved`] |
//! | P3 | edge-mint masquerade: the client appends the `CHECKED_BY` edge that records a check over another producer's claim | RFC 0038 "Authority": the checker is a `service:` identity (RFC 0027 P3); refusal is X1's | `CapabilityDenied`, no edge | same witness |
//! | P4 | self-certification: the client appends the check edge over its **own** claim | INV-004, "no self-certification" | `CapabilityDenied` at the scheme; the INV-004 layer beneath is held by the mutant and by dx12 B3 | same witness, and [`mutant_service_authority_flips_the_edge_mint_but_never_self_certification`] |
//! | P5/P6 | identity masquerade: the service's actor presented over the client's capability, or the client's actor over the service's capability | RFC 0027 T4: "an attempt to act as another principal is an authorization failure" | `CapabilityDenied`, indistinguishable from any other | same witness |
//! | P7 | replay: the authorized service's own recorded request, re-presented | RFC 0026 idempotency; RFC 0038 "a replayed write returns the original node identity"; dx12 B5 | convergence on the original outcome or denial — never transferred authority | [`attack_replaying_the_authorized_check_grants_the_attacker_nothing`] |
//!
//! Beside the refusals, two positive facts keep the universal honest:
//!
//! - **the ceiling** — the one write a client *can* cause is the daemon's own independent
//!   check, and on every wire-creatable node class it establishes `observed` and no more,
//!   under the service's identity, whoever asked
//!   ([`boundary_the_wire_ceiling_of_an_untrusted_clients_claim_is_observed`]);
//! - **the positive control** — the authorized path genuinely reaches `validated`, or the
//!   exit sentence would be vacuously true of a graph nobody can promote in
//!   ([`positive_control_the_authorized_path_genuinely_promotes_to_validated`]).
//!
//! # What "untrusted" means here, and who the cast are
//!
//! RFC 0038 draws the line at the actor scheme: "status promotion is service-restricted",
//! and an evidence-status write "is performed by a trusted service identity" (RFC 0027 P3).
//! Untrusted therefore means *not a `service:` actor* — and the anti-vacuity mutant is
//! exactly that line crossed: the same principal, same capability profile, same level,
//! re-registered under a `service:` scheme, at which point the edge-mint it was refused
//! lands (dx12's `mutant_a_service_actor_lands_the_check_edge_the_agent_could_not`
//! precedent) while the INV-004 self-certification refusal and the observed ceiling do not
//! move — the grant buys edge authority, never a status.
//!
//! | principal | capability | level | role |
//! |---|---|---|---|
//! | `agent:untrusted` (`service:untrusted` in the mutant) | `cap_untrusted` | `execute` + production-trace grant | the client under test; produces the proposal |
//! | `agent:producer` | `cap_bystander` | `execute` + production-trace grant | a second producer, so the edge-mint has a subject that is not the attacker's own claim |
//! | `service:checker` | `cap_checker` | `promote` + production-trace grant | the daemon's verification service ([`EvidenceFamily::verifying_as`]) and the one authorized checker |
//!
//! # The type-level half, cited rather than duplicated
//!
//! The dispatch-grain refusals here are the *outer* half of the boundary. The inner half is
//! already held type-level in two places, and this file cites it (with the freshness
//! tripwires in [`cited_type_level_and_campaign_evidence_still_stands`]) instead of
//! restating it:
//!
//! - **`crates/continuum-evidence/src/authority.rs`** (bn-2c8l) — three `compile_fail` doc
//!   tests, compiled as an external crate: a `Promotion` cannot be built by hand,
//!   `StatusWrite::promoted` is unreachable outside the crate, and a `TrustedService`
//!   cannot be declared over an `ActorId`; plus `UNTRUSTED_CEILING`, the crate's own
//!   statement that the set of promotions available without a trusted service is empty.
//! - **`crates/continuumd/src/daemon/evidence.rs`** (bn-24i) — INV-004 in four type-level
//!   places: no wire field carries a status into a write; the producer's append
//!   (`observe.ingest`) cannot name a status at all; the promotion write demands a
//!   [`Promotion`] witness whose fields are private to that module
//!   ([`DaemonState::promote_evidence`] takes one and nothing else); and a verification
//!   service may not verify its own production.
//!
//! # The campaign half, cited rather than re-run
//!
//! `dx12_falsification.rs` (the G0-DX-12 campaign) already attacks this boundary at
//! campaign grain — B1 non-service edge mint, B2 promotion forgery, B3 self-certification,
//! B4 edge retargeting, B5 authorized-edge replay — with its own baselines, per-gate
//! controls, and two capability-system mutants. Its subject *is* this exit's subject one
//! grain down, so it is cited with freshness pins rather than duplicated: what this file
//! adds is the exit sentence quantified at exit grain (every spelled path, one proposal,
//! byte-identical before and after) rather than the campaign's channel-by-channel sweep.
//!
//! # Honesty boundaries, stated rather than papered over
//!
//! - **"validated/proved" is witnessed at `validated`.** The authorized path this daemon
//!   ships is the certificate lane: a real `CONTCERT` artifact checked by
//!   `continuum-kernel-core`, the trusted checking base. `proved` names the Lean kernel,
//!   which is out of this process (PR 28); no deployment configuration reachable from this
//!   file can mint it, and the library-level authority table (bn-2c8l) already holds
//!   `proved`'s service restriction. The exit sentence's conjunction is therefore held as:
//!   both statuses unreachable by the client (asserted over the wire), the authorized path
//!   real at `validated` (asserted), real at `proved` one tier down (cited).
//! - **The certificate node is filed through state, not through an operation** — no wire
//!   operation appends a certificate-class node in this protocol version (`observe.ingest`
//!   appends `run`/`production-observation`; `evidence.link` appends `receipt`). That is
//!   `daemon_evidence.rs`'s own fixture idiom for the certificate lane, and it makes the
//!   *positive control* partially out-of-band while every *attack* stays wire-reachable —
//!   the safe direction for an exit about refusals.
//! - **Capability possession is identity at this grain.** A byte-perfect replay presenting
//!   the service's own actor and capability is admitted as the service — over an in-process
//!   dispatch nothing distinguishes it from the service retrying, and what the ledger then
//!   guarantees is convergence on the recorded outcome, never a second write. Keeping a
//!   `cap_*` unforgeable in transit is PR 5's transport layer, not this file's claim.
//! - **One thread, one request at a time.** [`Daemon::dispatch`] takes `&mut self`;
//!   concurrency is G0-DX-13's axis, evidenced in `continuum-workspace`'s campaign.
//!
//! # Evidence map (stable artifact IDs)
//!
//! The artifact IDs are the named tests of this committed file, per the bn-1n6r/bn-3tkw
//! precedent:
//!
//! | Clause of the exit | Test |
//! |---|---|
//! | no operation admits a client-declared status write (P0) | [`the_idl_admits_no_client_declared_status_write_anywhere`] |
//! | every spelled attack refused, typed, proposal byte-identical (P1–P6) | [`exit_an_untrusted_client_cannot_promote_a_proposal_to_validated_or_proved`] |
//! | the client-triggerable ceiling is `observed`, written by the service | [`boundary_the_wire_ceiling_of_an_untrusted_clients_claim_is_observed`] |
//! | the authorized path genuinely promotes (anti-vacuity of the exit itself) | [`positive_control_the_authorized_path_genuinely_promotes_to_validated`] |
//! | replay transfers no authority (P7) | [`attack_replaying_the_authorized_check_grants_the_attacker_nothing`] |
//! | service authority flips the edge-mint and nothing else (anti-vacuity mutant) | [`mutant_service_authority_flips_the_edge_mint_but_never_self_certification`] |
//! | the cited type-level and campaign evidence still stands | [`cited_type_level_and_campaign_evidence_still_stands`] |
//!
//! House rules: `src/` is not touched, no pre-existing test is edited, weakened, or moved;
//! fixtures are `dx12_falsification.rs`'s and `daemon_evidence.rs`'s idioms duplicated
//! locally, because a `tests/*.rs` file is its own crate and cannot `use` a sibling one.
//! Every assertion is on a typed value or on canonical bytes; no clock is read (INV-005 —
//! the one timestamp is the fixture's declared reading); everything iterated is
//! `BTreeMap`/`BTreeSet`-ordered.
//!
//! [`Daemon::dispatch`]: continuumd::daemon::Daemon::dispatch
//! [`DaemonState::promote_evidence`]: continuumd::daemon::state::DaemonState::promote_evidence
//! [`Promotion`]: continuumd::daemon::evidence::Promotion
//! [`EvidenceFamily::verifying_as`]: continuumd::daemon::evidence::EvidenceFamily::verifying_as

use std::collections::BTreeSet;

use continuum_certificate::{KernelVerdict, Outcome, continuum_kernel_core};
use continuum_evidence::actor::ServiceIdentity;
use continuum_evidence::authority::UNTRUSTED_CEILING;
use continuum_evidence::claim_status::ClaimStatus;
use continuum_value::assurance::ValidationBasis;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::evidence::{EvidenceFamily, node_identity};
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::daemon::state::{EvidenceNode, StatusWrite};
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest};
use continuumd::protocol::envelope::{Budget, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::evidence::{
    EvidenceGetRequest, EvidenceLinkRequest, EvidenceQueryRequest, EvidenceVerifyRequest,
};
use continuumd::protocol::operations::observe::ObserveIngestRequest;
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, EvidenceHandle, Opaque, OperationName, ProtocolVersion,
    RequestId, Timestamp,
};
use continuumd::protocol::shared::EvidenceQuery;
use continuumd::protocol::spec::{Annotation, Nullable, Optional, StructSpec};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, DataGrant, Encoding, ErrorCode, EvidenceKind, EvidenceNodeKind, EvidenceStatus,
    ResultStatus,
};

/// The client under test, in the honest configuration.
const AGENT_UNTRUSTED: &str = "agent:untrusted";
/// The same principal wearing service authority, in the mutant configuration.
const SERVICE_UNTRUSTED: &str = "service:untrusted";
/// The bystander producer, so the edge-mint has a subject the attacker did not produce.
const BYSTANDER: &str = "agent:producer";
/// The daemon's verification service — the one authorized checker.
const CHECKER: &str = "service:checker";

/// The untrusted client's production trace: the referent of "a proposal".
const CLAIM_TRACE: &str = "{\"events\":[{\"at\":0,\"op\":\"untrusted-fill\"}]}\n";
/// The bystander's trace.
const BYSTANDER_TRACE: &str = "{\"events\":[{\"at\":0,\"op\":\"bystander-pour\"}]}\n";
/// A receipt blob, the `to` content of a check edge.
const RECEIPT_BLOB: &str = "{\"receipt\":\"the checker re-checked this\"}\n";
/// The instrumentation profile the traces were captured under (plan §18.4).
const PROFILE: &str = "otel-1.0/sampled";
/// The checker's tool identity on a check edge.
const CHECKER_PROFILE: &str = "kernel-core/1";
/// The instrumentation profile a certificate node is filed under
/// (`daemon_evidence.rs`'s certificate-lane idiom).
const CERTIFICATE_PROFILE: &str = "continuum-engine-reference/finite-closure";

// =========================================================================================
// fixture plumbing — the idioms `dx12_falsification.rs` and `daemon_evidence.rs` share
// =========================================================================================

fn version() -> ProtocolVersion {
    // 3.3: the minor at which `evidence.link` exists, so every spelled path is dispatchable.
    ProtocolVersion::new(3, 3)
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

/// A profile with the plan §18.2 production-trace grant `observe.ingest` requires beyond
/// its level (RFC 0027 R-4), and no privileged operation: authority in this file is the
/// scheme and the ladder, never a privilege list.
fn traced() -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: Vec::new(),
        denied_operations: Vec::new(),
        data_grants: vec![DataGrant::ProductionTrace],
        cross_principal_sharing: false,
    }
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
        profile: Optional::Present(traced()),
    }
}

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-pr7-exit-evidence".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello).expect("3.3 is served")
}

/// The daemon under test. `untrusted_actor` is the anti-vacuity switch: the one thing that
/// differs between the honest daemon and the mutant is the scheme of the principal under
/// test — same capability handle, same level, same profile, same delegation.
fn daemon_with(untrusted_actor: &'static str) -> Daemon {
    let root = Some(cap("cap_root"));
    Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .now(now())
        .capability(
            grant("cap_root", "service:continuumd", AuthorityLevel::Promote, 4),
            None,
        )
        // The client under test: `execute` — enough for every operation any attack below
        // names, so each refusal is attributable to authority over *status*, never to the
        // ladder rung.
        .capability(
            grant("cap_untrusted", untrusted_actor, AuthorityLevel::Execute, 3),
            root.clone(),
        )
        .capability(
            grant("cap_bystander", BYSTANDER, AuthorityLevel::Execute, 3),
            root.clone(),
        )
        .capability(
            grant("cap_checker", CHECKER, AuthorityLevel::Promote, 3),
            root,
        )
        // The daemon verifies *as* `service:checker`: RFC 0027 P3's trusted service
        // identity, distinct from every producer in the cast.
        .family(EvidenceFamily::verifying_as(who(CHECKER)))
        .family(ObserveFamily)
        .build()
}

/// The world one test works in: a daemon and the staged content the cast work over.
struct World {
    daemon: Daemon,
    /// The actor string the principal under test presents.
    untrusted: &'static str,
    /// The untrusted client's trace — the proposal's referent.
    claim: Commitment,
    /// The bystander's trace.
    other_claim: Commitment,
    /// A receipt blob.
    receipt: Commitment,
}

fn world() -> World {
    world_with(AGENT_UNTRUSTED)
}

fn world_with(untrusted_actor: &'static str) -> World {
    let mut daemon = daemon_with(untrusted_actor);
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
    let claim = stage("traces/untrusted-claim.jsonl", CLAIM_TRACE);
    let other_claim = stage("traces/bystander-claim.jsonl", BYSTANDER_TRACE);
    let receipt = stage("receipts/checker.json", RECEIPT_BLOB);
    World {
        daemon,
        untrusted: untrusted_actor,
        claim,
        other_claim,
        receipt,
    }
}

// --- envelopes ---------------------------------------------------------------------------

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

/// A `@mutation` envelope: idempotency key only.
fn keyed(mut envelope: RequestEnvelope, key: &str) -> RequestEnvelope {
    envelope.idempotency_key = Optional::Present(key.to_owned());
    envelope
}

/// A `@mutation @task_starting` envelope: key plus a budget.
fn started(mut envelope: RequestEnvelope, key: &str) -> RequestEnvelope {
    envelope.idempotency_key = Optional::Present(key.to_owned());
    envelope.budget = Optional::Present(budget());
    envelope
}

// --- the operations, each as one dispatch ------------------------------------------------

fn ingest_as(
    world: &mut World,
    actor: &str,
    capability: &str,
    trace: &Commitment,
    request: &str,
    key: &str,
) -> OperationOutcome {
    world.daemon.dispatch(&OperationRequest {
        envelope: started(envelope("observe.ingest", actor, capability, request), key),
        arguments: Arguments::ObserveIngest(ObserveIngestRequest {
            trace: trace.clone(),
            instrumentation_profile: PROFILE.to_owned(),
        }),
    })
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

fn verify_as(
    world: &mut World,
    actor: &str,
    capability: &str,
    handle: &EvidenceHandle,
    expected: Optional<EvidenceStatus>,
    request: &str,
    key: &str,
) -> OperationOutcome {
    world.daemon.dispatch(&OperationRequest {
        envelope: started(envelope("evidence.verify", actor, capability, request), key),
        arguments: Arguments::EvidenceVerify(EvidenceVerifyRequest {
            evidence: handle.clone(),
            expected_status: expected,
        }),
    })
}

fn link_as(
    world: &mut World,
    actor: &str,
    capability: &str,
    subject: &EvidenceHandle,
    receipt: &Commitment,
    request: &str,
    key: &str,
) -> OperationOutcome {
    world.daemon.dispatch(&OperationRequest {
        envelope: keyed(envelope("evidence.link", actor, capability, request), key),
        arguments: Arguments::EvidenceLink(EvidenceLinkRequest {
            subject: subject.clone(),
            receipt: receipt.clone(),
            checker_profile: CHECKER_PROFILE.to_owned(),
        }),
    })
}

fn linked_edge(outcome: &OperationOutcome) -> EvidenceHandle {
    match &outcome.payload {
        Payload::EvidenceLink(response) => response.edge.clone(),
        other => panic!("expected an evidence.link payload, got {other:?}"),
    }
}

// --- observers of the record -------------------------------------------------------------

fn code(outcome: &OperationOutcome) -> ErrorCode {
    outcome.error_code().expect("an error result")
}

fn node(world: &World, handle: &EvidenceHandle) -> EvidenceNode {
    world
        .daemon
        .state()
        .evidence(handle)
        .expect("the graph holds the node")
        .clone()
}

/// The node's wire record, byte for byte, read by the untrusted client itself through
/// `evidence.get` — so "the proposal's status is byte-identical before and after" is a fact
/// about what a real client observes over the real dispatch, not about internal state.
fn node_record_bytes(world: &mut World, handle: &EvidenceHandle, request: &str) -> Vec<u8> {
    let (actor, capability) = (world.untrusted, "cap_untrusted");
    let outcome = world.daemon.dispatch(&OperationRequest {
        envelope: envelope("evidence.get", actor, capability, request),
        arguments: Arguments::EvidenceGet(EvidenceGetRequest {
            evidence: handle.clone(),
            inline: Optional::Absent,
        }),
    });
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "the client can read its own node: {:?}",
        outcome.envelope.error
    );
    match &outcome.payload {
        Payload::EvidenceGet(response) => match &response.node {
            Nullable::Value(bytes) => bytes.as_bytes().to_vec(),
            Nullable::Null => panic!("the handle names a node, not an edge"),
        },
        other => panic!("expected an evidence.get payload, got {other:?}"),
    }
}

/// Every node and edge holding one of `statuses`, asked over the wire by the untrusted
/// client — the quantified "nothing in this graph is validated/proved" observer.
fn holding_statuses(
    world: &mut World,
    statuses: Vec<EvidenceStatus>,
    request: &str,
) -> (Vec<EvidenceHandle>, Vec<EvidenceHandle>) {
    let (actor, capability) = (world.untrusted, "cap_untrusted");
    let outcome = world.daemon.dispatch(&OperationRequest {
        envelope: envelope("evidence.query", actor, capability, request),
        arguments: Arguments::EvidenceQuery(EvidenceQueryRequest {
            query: EvidenceQuery {
                node_kinds: Optional::Absent,
                edge_kinds: Optional::Absent,
                statuses: Optional::Present(statuses),
                claim_id: Optional::Absent,
                roots: Optional::Absent,
                max_depth: Optional::Absent,
            },
        }),
    });
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        outcome.envelope.error
    );
    match &outcome.payload {
        Payload::EvidenceQuery(response) => (response.nodes.clone(), response.edges.clone()),
        other => panic!("expected an evidence.query payload, got {other:?}"),
    }
}

/// The proposal: the untrusted client's own claim, appended through the wire, landing at
/// the lattice's bottom exactly as `observe.ingest`'s shape dictates (INV-004 place two).
fn proposal(world: &mut World) -> EvidenceHandle {
    let (actor, claim) = (world.untrusted, world.claim.clone());
    let handle = ingested_handle(&ingest_as(
        world,
        actor,
        "cap_untrusted",
        &claim,
        "req_proposal",
        "idem-proposal",
    ));
    let appended = node(world, &handle);
    assert_eq!(
        appended.status(),
        ClaimStatus::Proposed,
        "creation is not a promotion"
    );
    assert_eq!(appended.history.len(), 1, "one write: the append itself");
    assert_eq!(
        appended.service_identity(),
        None,
        "a producer's append names no service, so a service-attributed status cannot be \
         reached by appending"
    );
    handle
}

/// A real `CONTCERT` artifact: Die Hard explored, closed, and emitted as wire bytes —
/// `daemon_evidence.rs`'s certificate-lane fixture, duplicated locally.
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

/// Stage `bytes` and file a certificate-class node over them, produced by `producer`, under
/// the identity it derives.
///
/// Through state rather than through an operation, because no wire operation appends a
/// certificate-class node in this protocol version — the honesty boundary in the module
/// documentation, and `daemon_evidence.rs`'s own idiom for this lane.
fn certificate_node(
    world: &mut World,
    producer: &str,
    path: &str,
    bytes: Vec<u8>,
) -> EvidenceHandle {
    let artifact = world
        .daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new(path).expect("a workspace path"),
            bytes,
        )
        .expect("staging names its content");
    let handle = node_identity(world.daemon.services(), &artifact, CERTIFICATE_PROFILE)
        .expect("the identity seam names the node");
    let record = EvidenceNode {
        kind: EvidenceNodeKind::Certificate,
        evidence_kind: Some(EvidenceKind::Certificate),
        labels: Vec::new(),
        claim_id: "claim:die-hard-closure".to_owned(),
        artifact,
        producer: who(producer),
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
    world
        .daemon
        .state_mut()
        .append_evidence(handle.clone(), record);
    handle
}

// =========================================================================================
// P0 — no operation admits a client-declared status write (the closed scan)
// =========================================================================================

/// Whether `ty` names `name` as a whole token — `list<EvidenceStatus>` mentions
/// `EvidenceStatus`; `EvidenceStatusish` would not.
fn mentions(ty: &str, name: &str) -> bool {
    ty.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|token| token == name)
}

/// Every request-reachable field path whose declared type involves `EvidenceStatus`,
/// walking named structs transitively, collected as `(operation, path)`.
fn status_request_paths() -> BTreeSet<(String, String)> {
    fn walk(
        spec: &StructSpec,
        prefix: &str,
        operation: &str,
        visited: &mut Vec<&'static str>,
        found: &mut BTreeSet<(String, String)>,
    ) {
        for field in spec.fields {
            let path = if prefix.is_empty() {
                field.name.to_owned()
            } else {
                format!("{prefix}.{}", field.name)
            };
            if mentions(field.ty, "EvidenceStatus") {
                found.insert((operation.to_owned(), path.clone()));
            }
            for named in registry::NAMED_STRUCTS {
                if mentions(field.ty, named.name) && !visited.contains(&named.name) {
                    visited.push(named.name);
                    walk(named, &path, operation, visited, found);
                }
            }
        }
    }

    let mut found = BTreeSet::new();
    for operation in registry::OPERATIONS {
        let mut visited: Vec<&'static str> = vec![operation.request.name];
        walk(
            &operation.request,
            "",
            operation.name,
            &mut visited,
            &mut found,
        );
    }
    found
}

/// **P0.** "Direct status write" is not refused — it is unspellable: quantified over every
/// operation the registry declares (the compiled mirror of
/// `schemas/continuumd-native-protocol.idl`, held to it by `rule
/// conformance.registry_agreement` in `idl_conformance.rs`), no request shape carries an
/// `EvidenceStatus` into a mutation anywhere except `evidence.verify`'s compare-and-set
/// *guard* — and the guard's own governing sentence says it "is never the status written",
/// which [`exit_an_untrusted_client_cannot_promote_a_proposal_to_validated_or_proved`]
/// holds behaviorally. The quantifier is closed over the live registry rather than pinned
/// to an operation count, so a future protocol minor that adds a status-bearing request
/// field fails this test loudly instead of widening the surface in silence.
#[test]
fn the_idl_admits_no_client_declared_status_write_anywhere() {
    // The registry's own self-consistency, so "every operation" below means every operation.
    assert_eq!(registry::OPERATIONS.len(), registry::OPERATION_COUNT);

    let expected: BTreeSet<(String, String)> = [
        // The CAS guard: the status the caller believes the claim already holds.
        ("evidence.verify", "expected_status"),
        // Read filters: which statuses to *select*, on operations that write nothing.
        ("evidence.query", "query.statuses"),
        ("evidence.subscribe", "scope.statuses"),
        ("forge.archive", "statuses"),
    ]
    .into_iter()
    .map(|(operation, path)| (operation.to_owned(), path.to_owned()))
    .collect();
    assert_eq!(
        status_request_paths(),
        expected,
        "a new status-typed request field is a new promotion surface and must be argued \
         into this exit's table, not absorbed"
    );

    // The three filters live on operations that declare themselves incapable of writing.
    for operation in ["evidence.query", "evidence.subscribe", "forge.archive"] {
        let spec = registry::operation(operation).expect("the registry declares it");
        assert!(
            spec.has(Annotation::Readonly),
            "{operation} carries a status filter and must stay @readonly"
        );
    }

    // The guard's operation is a mutation — and its declared errors include the lost-CAS
    // refusal, which is what a guard that is not a write answers with.
    let verify = registry::operation("evidence.verify").expect("the registry declares it");
    assert!(verify.has(Annotation::Mutation));
    assert!(verify.errors.contains(&ErrorCode::StatusConflict));

    // Therefore: among every @mutation operation in the protocol, the only status-typed
    // request field is the guard. Stated as its own assertion so the conclusion is checked,
    // not implied.
    let mutation_paths: Vec<(String, String)> = status_request_paths()
        .into_iter()
        .filter(|(operation, _)| {
            registry::operation(operation)
                .expect("the scan only names declared operations")
                .has(Annotation::Mutation)
        })
        .collect();
    assert_eq!(
        mutation_paths,
        vec![("evidence.verify".to_owned(), "expected_status".to_owned())],
        "no mutation admits a client-declared status; the one status-typed field is the \
         compare-and-set guard"
    );
}

// =========================================================================================
// The exit witness — every spelled attack, one proposal, byte-identical before and after
// =========================================================================================

/// **The exit sentence at exit grain.** A real untrusted principal, over the real dispatch,
/// attempts every spelled promotion path against a proposal (its own claim, at `proposed`)
/// and against a bystander's; every attempt ends in a typed refusal, and the proposal's
/// wire record — status included — is byte-identical before and after, as read back by the
/// attacker itself through `evidence.get`. Closing the quantifier, an `evidence.query` for
/// anything `validated` or `proved` answers empty over the whole graph.
#[test]
fn exit_an_untrusted_client_cannot_promote_a_proposal_to_validated_or_proved() {
    let mut world = world();
    let mine = proposal(&mut world);
    let other_claim = world.other_claim.clone();
    let theirs = ingested_handle(&ingest_as(
        &mut world,
        BYSTANDER,
        "cap_bystander",
        &other_claim,
        "req_bystander",
        "idem-bystander",
    ));
    let receipt = world.receipt.clone();

    let mine_before = node_record_bytes(&mut world, &mine, "req_before_mine");
    let theirs_before = node_record_bytes(&mut world, &theirs, "req_before_theirs");
    assert_eq!(world.daemon.state().evidence_edges().count(), 0);

    // P1 — the guard as a write, naming `validated`. The compare-and-set is lost against
    // the claim's real status and nothing is written: the guard is what the caller believes,
    // never what the daemon installs.
    let forged_validated = verify_as(
        &mut world,
        AGENT_UNTRUSTED,
        "cap_untrusted",
        &mine,
        Optional::Present(EvidenceStatus::Validated),
        "req_p1",
        "idem-p1",
    );
    assert_eq!(code(&forged_validated), ErrorCode::StatusConflict);

    // P2 — the guard naming `proved`, the other half of the exit's pair.
    let forged_proved = verify_as(
        &mut world,
        AGENT_UNTRUSTED,
        "cap_untrusted",
        &mine,
        Optional::Present(EvidenceStatus::Proved),
        "req_p2",
        "idem-p2",
    );
    assert_eq!(code(&forged_proved), ErrorCode::StatusConflict);

    // P3 — edge-mint masquerade: the client records "a check ran" over the bystander's
    // claim. The `execute` level admits the operation; the scheme refuses the actor
    // (RFC 0038 "Authority" — the gate is the scheme, not the ladder).
    let minted = link_as(
        &mut world,
        AGENT_UNTRUSTED,
        "cap_untrusted",
        &theirs,
        &receipt,
        "req_p3",
        "idem-p3",
    );
    assert_eq!(code(&minted), ErrorCode::CapabilityDenied);
    assert_eq!(world.daemon.state().evidence_edges().count(), 0);

    // P4 — self-certification: the identical mint over its *own* claim. Refused at the
    // same scheme gate; the INV-004 producer gate beneath it is what the mutant below and
    // dx12's B3 hold, so elevation to a service still cannot land this one.
    let self_certified = link_as(
        &mut world,
        AGENT_UNTRUSTED,
        "cap_untrusted",
        &mine,
        &receipt,
        "req_p4",
        "idem-p4",
    );
    assert_eq!(code(&self_certified), ErrorCode::CapabilityDenied);
    assert_eq!(world.daemon.state().evidence_edges().count(), 0);

    // P5 — identity masquerade, first direction: the service's *actor* presented over the
    // attacker's capability. RFC 0027 T4: the capability binds to its registered actor, and
    // a mismatch is an authorization failure indistinguishable from any other.
    let stolen_actor = link_as(
        &mut world,
        CHECKER,
        "cap_untrusted",
        &theirs,
        &receipt,
        "req_p5",
        "idem-p5",
    );
    assert_eq!(code(&stolen_actor), ErrorCode::CapabilityDenied);

    // P6 — the other direction: the attacker's actor over the service's capability handle.
    let stolen_capability = link_as(
        &mut world,
        AGENT_UNTRUSTED,
        "cap_checker",
        &theirs,
        &receipt,
        "req_p6",
        "idem-p6",
    );
    assert_eq!(code(&stolen_capability), ErrorCode::CapabilityDenied);
    assert_eq!(world.daemon.state().evidence_edges().count(), 0);

    // The proposal — and the bystander's claim — byte-identical, read back over the wire by
    // the attacker itself. Not "still proposed" as a field: the whole record, to the byte.
    assert_eq!(
        node_record_bytes(&mut world, &mine, "req_after_mine"),
        mine_before,
        "every attack refused, and the proposal's wire record did not move by one byte"
    );
    assert_eq!(
        node_record_bytes(&mut world, &theirs, "req_after_theirs"),
        theirs_before
    );
    assert_eq!(node(&world, &mine).history.len(), 1);
    assert_eq!(node(&world, &theirs).history.len(), 1);

    // The quantifier, closed over the graph the daemon will answer for: nothing anywhere
    // holds `validated` or `proved`.
    let (nodes, edges) = holding_statuses(
        &mut world,
        vec![EvidenceStatus::Validated, EvidenceStatus::Proved],
        "req_quantifier",
    );
    assert_eq!(nodes, Vec::<EvidenceHandle>::new());
    assert_eq!(edges, Vec::<EvidenceHandle>::new());
}

// =========================================================================================
// The ceiling — what a client *can* cause, and why it is below the exit's pair
// =========================================================================================

/// The one status write reachable from an untrusted client is the daemon's own independent
/// check, and over every wire-creatable node class it establishes `observed` — RFC 0038's
/// "one or more concrete executions" — and no more. The write is the service's, whoever
/// asked: the response's `checker` names the service, never the caller, and the written
/// history entry names no service at all because the node schema attributes only the four
/// assurance-bearing statuses. A second forged guard cannot ratchet it.
#[test]
fn boundary_the_wire_ceiling_of_an_untrusted_clients_claim_is_observed() {
    let mut world = world();
    let mine = proposal(&mut world);

    // The client asks for the check on its own claim. Asking is permitted — the caller is
    // not the checker — and what the check establishes is the ceiling: `observed`.
    let checked = verify_as(
        &mut world,
        AGENT_UNTRUSTED,
        "cap_untrusted",
        &mine,
        Optional::Absent,
        "req_ceiling",
        "idem-ceiling",
    );
    assert_eq!(
        checked.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        checked.envelope.error
    );
    match &checked.payload {
        Payload::EvidenceVerify(response) => {
            assert_eq!(response.status, EvidenceStatus::Observed);
            assert_eq!(
                response.checker, CHECKER,
                "the checker is the service, never the caller who asked"
            );
        }
        other => panic!("expected an evidence.verify payload, got {other:?}"),
    }
    let written = node(&world, &mine);
    assert_eq!(written.status(), ClaimStatus::Observed);
    assert_eq!(
        written.service_identity(),
        None,
        "`observed` is not service-attributed: the schema names a service only on the four \
         assurance-bearing statuses, so even the daemon's own write here carries none"
    );

    // From `observed`, the forged guard still cannot ratchet: the CAS is lost and the
    // status the daemon's check supports is unchanged.
    let ratchet = verify_as(
        &mut world,
        AGENT_UNTRUSTED,
        "cap_untrusted",
        &mine,
        Optional::Present(EvidenceStatus::Validated),
        "req_ratchet",
        "idem-ratchet",
    );
    assert_eq!(code(&ratchet), ErrorCode::StatusConflict);
    assert_eq!(node(&world, &mine).status(), ClaimStatus::Observed);

    // The pair the exit names stays empty, before and after the daemon's full cooperation.
    let (nodes, _) = holding_statuses(
        &mut world,
        vec![EvidenceStatus::Validated, EvidenceStatus::Proved],
        "req_ceiling_quantifier",
    );
    assert_eq!(nodes, Vec::<EvidenceHandle>::new());
}

// =========================================================================================
// The positive control — the authorized path genuinely promotes
// =========================================================================================

/// Without this the exit is vacuous: a graph nobody can promote satisfies "an untrusted
/// client cannot promote" trivially. Here the *same untrusted principal* produces an
/// artifact that genuinely supports `validated` — a real `CONTCERT` certificate — and the
/// promotion lands, performed by the trusted service on the strength of
/// `continuum-kernel-core`'s own verdict, attributed to the service in the written history
/// exactly as the node schema requires. The client's honest road to `validated` is to
/// produce something an independent checker verifies; its say-so buys nothing, its work
/// everything. `proved` stays out of reach of every configuration in this file — the Lean
/// kernel is out of process (PR 28) — and is asserted absent rather than assumed.
#[test]
fn positive_control_the_authorized_path_genuinely_promotes_to_validated() {
    let mut world = world();
    let bytes = certificate_bytes();

    // What the trusted checking base says about these bytes, read directly, so the daemon's
    // answer below is held against the checker's rather than against this test's hope.
    assert!(matches!(
        continuum_certificate::check_certificate(&bytes),
        Outcome::Checked(KernelVerdict::Core(
            continuum_kernel_core::Verdict::Verified(_)
        ))
    ));

    // The untrusted principal is the certificate's *producer*: INV-004's producer gate
    // (producer ≠ verification service) passes, and the caller below is the producer too —
    // who asked is not who checked.
    let handle = certificate_node(&mut world, AGENT_UNTRUSTED, "certs/die-hard.cert", bytes);
    let verified = verify_as(
        &mut world,
        AGENT_UNTRUSTED,
        "cap_untrusted",
        &handle,
        Optional::Absent,
        "req_control",
        "idem-control",
    );
    assert_eq!(
        verified.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        verified.envelope.error
    );
    match &verified.payload {
        Payload::EvidenceVerify(response) => {
            assert_eq!(response.status, EvidenceStatus::Validated);
            assert_eq!(response.evidence_kind, EvidenceKind::Certificate);
            assert_eq!(response.checker, CHECKER);
            assert_eq!(response.validation_basis, "checked-certificate");
        }
        other => panic!("expected an evidence.verify payload, got {other:?}"),
    }

    // The written history: `validated` is service-attributed, names its basis, and the
    // service it names is the daemon's verifier — the caller's identity appears nowhere.
    let written = node(&world, &handle);
    assert_eq!(written.status(), ClaimStatus::Validated);
    let last = written.history.last().expect("history is never empty");
    assert_eq!(last.service_identity.as_deref(), Some(CHECKER));
    assert_eq!(
        last.validation_basis,
        Some(ValidationBasis::CheckedCertificate)
    );

    // `validated` is now genuinely in the graph — and `proved` still is not, in the same
    // deployment that just proved the promotion machinery live.
    let (validated, _) = holding_statuses(
        &mut world,
        vec![EvidenceStatus::Validated],
        "req_control_validated",
    );
    assert_eq!(validated, vec![handle]);
    let (proved, _) = holding_statuses(
        &mut world,
        vec![EvidenceStatus::Proved],
        "req_control_proved",
    );
    assert_eq!(proved, Vec::<EvidenceHandle>::new());
}

// =========================================================================================
// P7 — replay of the authorized check transfers no authority
// =========================================================================================

/// The service records a real check of the proposal (`evidence.link`, the one authorized
/// edge append). Three replays follow, and none moves the proposal or mints authority:
///
/// 1. the attacker re-presents the identical body and idempotency key under its **own**
///    admitted identity — the ledger is scoped per actor, so there is no recorded outcome
///    to steal; the call executes fresh as the attacker and dies at the scheme;
/// 2. the attacker re-presents the identical body and key under the **service's** actor
///    with its own capability — admission (dispatch step 5) precedes the idempotency
///    ledger (step 7), so T4 refuses before the recorded outcome is even consulted;
/// 3. a byte-perfect replay as the service itself — indistinguishable in-process from the
///    service retrying — converges on the recorded outcome verbatim: same edge identity,
///    no second write, no new event (RFC 0038's write model; dx12 B5's convergence, cited).
///
/// And the authorized check edge itself moves no status: what an edge licenses is
/// `evidence.verify`'s separate decision, so even the *authorized* path's edge leaves the
/// proposal's record byte-identical.
#[test]
fn attack_replaying_the_authorized_check_grants_the_attacker_nothing() {
    let mut world = world();
    let mine = proposal(&mut world);
    let receipt = world.receipt.clone();
    let before = node_record_bytes(&mut world, &mine, "req_before");

    // The authorized check: the service records that it checked the untrusted claim.
    let authorized = link_as(
        &mut world,
        CHECKER,
        "cap_checker",
        &mine,
        &receipt,
        "req_authorized",
        "idem-authorized",
    );
    assert_eq!(
        authorized.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        authorized.envelope.error
    );
    let edge = linked_edge(&authorized);
    assert_eq!(world.daemon.state().evidence_edges().count(), 1);
    let events_after_authorized = world.daemon.state().evidence_events().len();

    // The authorized edge is not a promotion: the proposal's wire record is byte-identical
    // even after a real check was recorded over it.
    assert_eq!(
        node_record_bytes(&mut world, &mine, "req_after_edge"),
        before,
        "a CHECKED_BY edge records that a check ran; the status write is a separate, \
         service-only decision"
    );

    // Replay 1 — same body, same key, the attacker's own identity. Per-actor ledger: no
    // recorded outcome under (agent:untrusted, idem-authorized), so this executes fresh as
    // the attacker and the scheme refuses it.
    let replayed_as_self = link_as(
        &mut world,
        AGENT_UNTRUSTED,
        "cap_untrusted",
        &mine,
        &receipt,
        "req_replay_self",
        "idem-authorized",
    );
    assert_eq!(code(&replayed_as_self), ErrorCode::CapabilityDenied);
    assert_eq!(world.daemon.state().evidence_edges().count(), 1);

    // Replay 2 — the service's actor over the attacker's capability, same key. Admission
    // precedes the ledger, so the recorded outcome is unreachable behind T4's denial.
    let replayed_as_masquerade = link_as(
        &mut world,
        CHECKER,
        "cap_untrusted",
        &mine,
        &receipt,
        "req_replay_masquerade",
        "idem-authorized",
    );
    assert_eq!(code(&replayed_as_masquerade), ErrorCode::CapabilityDenied);
    assert_eq!(world.daemon.state().evidence_edges().count(), 1);

    // Replay 3 — byte-perfect, as the service (in-process, indistinguishable from the
    // service retrying; capability secrecy in transit is PR 5's layer). The ledger answers
    // the recorded outcome verbatim: the same edge identity, and nothing executed again.
    let replayed_verbatim = link_as(
        &mut world,
        CHECKER,
        "cap_checker",
        &mine,
        &receipt,
        "req_replay_verbatim",
        "idem-authorized",
    );
    assert_eq!(replayed_verbatim.envelope.status, ResultStatus::Ok);
    assert_eq!(linked_edge(&replayed_verbatim), edge);
    assert_eq!(world.daemon.state().evidence_edges().count(), 1);
    assert_eq!(
        world.daemon.state().evidence_events().len(),
        events_after_authorized,
        "a true replay returns the recorded outcome; it does not run and does not emit"
    );

    // After every replay: the proposal unmoved, and the exit's pair still empty.
    assert_eq!(node_record_bytes(&mut world, &mine, "req_after"), before);
    let (nodes, _) = holding_statuses(
        &mut world,
        vec![EvidenceStatus::Validated, EvidenceStatus::Proved],
        "req_replay_quantifier",
    );
    assert_eq!(nodes, Vec::<EvidenceHandle>::new());
}

// =========================================================================================
// Anti-vacuity — service authority flips exactly the gate the exit rests on, and no more
// =========================================================================================

/// The dx12 mutant precedent at exit grain. The one difference from the honest world is
/// the principal's actor scheme: `service:untrusted` instead of `agent:untrusted`, same
/// capability handle, same `execute` level, same profile. Under that grant:
///
/// - the **edge-mint that P3 refused now lands** — so P3's refusal was the authority gate
///   and nothing else, and a fail-open scheme check would have flipped the exit witness's
///   own assertion (the harness can fail);
/// - the identical mint over the principal's **own** production is *still* refused — the
///   refusal moves from the scheme (`CapabilityDenied`) to INV-004's producer gate
///   (`InsufficientEvidence`): authority elevation buys edge authority, never
///   self-certification;
/// - and the principal's claim *still* cannot reach `validated`/`proved` — the check
///   ceiling and the lost CAS are unmoved, because a status is what the daemon's checker
///   establishes, which no capability grant changes.
#[test]
fn mutant_service_authority_flips_the_edge_mint_but_never_self_certification() {
    let mut world = world_with(SERVICE_UNTRUSTED);
    let mine = proposal(&mut world);
    let other_claim = world.other_claim.clone();
    let theirs = ingested_handle(&ingest_as(
        &mut world,
        BYSTANDER,
        "cap_bystander",
        &other_claim,
        "req_bystander",
        "idem-bystander",
    ));
    let receipt = world.receipt.clone();

    // The identical call the exit witness refuses as P3, now from a `service:` scheme.
    let landed = link_as(
        &mut world,
        SERVICE_UNTRUSTED,
        "cap_untrusted",
        &theirs,
        &receipt,
        "req_mutant_p3",
        "idem-mutant-p3",
    );
    assert_eq!(
        landed.envelope.status,
        ResultStatus::Ok,
        "with service authority, the very call P3 refuses now lands — the verdict flips, \
         which is what makes P3 a real test: {:?}",
        landed.envelope.error
    );
    assert_eq!(world.daemon.state().evidence_edges().count(), 1);
    let edge = world
        .daemon
        .state()
        .evidence_edge(&linked_edge(&landed))
        .expect("the graph holds the landed edge")
        .clone();
    assert_eq!(edge.producer.as_str(), SERVICE_UNTRUSTED);

    // The identical call over its own production: still refused, now by INV-004 itself.
    let self_certified = link_as(
        &mut world,
        SERVICE_UNTRUSTED,
        "cap_untrusted",
        &mine,
        &receipt,
        "req_mutant_p4",
        "idem-mutant-p4",
    );
    assert_eq!(
        code(&self_certified),
        ErrorCode::InsufficientEvidence,
        "elevation changes the refusal's code, never its answer: a producer may not record \
         a check of its own claim"
    );
    assert_eq!(world.daemon.state().evidence_edges().count(), 1);

    // And status authority did not arrive with edge authority: the ceiling and the CAS are
    // exactly where the honest world left them.
    let forged = verify_as(
        &mut world,
        SERVICE_UNTRUSTED,
        "cap_untrusted",
        &mine,
        Optional::Present(EvidenceStatus::Validated),
        "req_mutant_forge",
        "idem-mutant-forge",
    );
    assert_eq!(code(&forged), ErrorCode::StatusConflict);
    assert_eq!(node(&world, &mine).status(), ClaimStatus::Proposed);
    let (nodes, _) = holding_statuses(
        &mut world,
        vec![EvidenceStatus::Validated, EvidenceStatus::Proved],
        "req_mutant_quantifier",
    );
    assert_eq!(
        nodes,
        Vec::<EvidenceHandle>::new(),
        "the mutant mints edges, never statuses"
    );
}

// =========================================================================================
// The cited evidence, pinned for freshness (the bn-1eqt cite-don't-re-run rule)
// =========================================================================================

/// The dx12 campaign and the two type-level boundaries this file cites are load-bearing
/// parts of the exit's case. Citations rot silently; these pins rot loudly.
#[test]
fn cited_type_level_and_campaign_evidence_still_stands() {
    // --- the campaign grain: dx12's five evidence-graph attacks and its mutants ---------
    const DX12: &str = include_str!("dx12_falsification.rs");
    for marker in [
        "attack_b1_a_non_service_principal_cannot_mint_a_check_edge",
        "attack_b2_a_producer_cannot_name_or_forge_a_promoted_status",
        "attack_b3_the_service_cannot_certify_its_own_production",
        "attack_b4_a_retargeted_or_misfiled_node_is_re_derived_and_refused",
        "attack_b5_replaying_an_authorized_edge_cannot_smuggle_different_content",
        "mutant_a_service_actor_lands_the_check_edge_the_agent_could_not",
        "Every unauthorized channel was refused by a typed authority check",
    ] {
        assert!(
            DX12.contains(marker),
            "the cited dx12 campaign no longer carries `{marker}`; re-argue this exit's \
             campaign citation before re-pinning"
        );
    }

    // --- the library type level: bn-2c8l's three compile_fail witnesses -----------------
    const AUTHORITY: &str = include_str!("../../continuum-evidence/src/authority.rs");
    assert_eq!(
        AUTHORITY.matches("```compile_fail").count(),
        3,
        "authority.rs holds exactly three compile_fail doc tests: a hand-built Promotion, \
         an externally minted StatusWrite, and a TrustedService over an ActorId"
    );
    for marker in [
        "pub const UNTRUSTED_CEILING",
        "StatusWrite::promoted(ClaimStatus::Validated",
        "TrustedService::new(agent",
    ] {
        assert!(
            AUTHORITY.contains(marker),
            "authority.rs no longer carries `{marker}`"
        );
    }
    // The ceiling's runtime half: the strongest status reachable without a trusted service
    // is the lattice's bottom — the very status "a proposal" names.
    assert_eq!(UNTRUSTED_CEILING, ClaimStatus::Proposed);
    assert_eq!(ClaimStatus::BOTTOM, ClaimStatus::Proposed);

    // --- the daemon type level: bn-24i's four places, and the scheme line ---------------
    const DAEMON_EVIDENCE: &str = include_str!("../src/daemon/evidence.rs");
    for marker in [
        "The enforcement is in four places",
        "pub struct Promotion",
        "no public constructor",
        "may not verify its own production",
    ] {
        assert!(
            DAEMON_EVIDENCE.contains(marker),
            "daemon/evidence.rs no longer carries `{marker}`"
        );
    }
    // The scheme line itself, live: a checker is a `service:` identity and nothing else,
    // decided by `continuum-evidence`'s own type — the daemon and the graph agree on what
    // a checker is by construction.
    assert!(ServiceIdentity::parse(CHECKER).is_ok());
    for refused in [AGENT_UNTRUSTED, BYSTANDER, "human:reviewer", "ci:pipeline"] {
        assert!(
            ServiceIdentity::parse(refused).is_err(),
            "`{refused}` must not parse as a service identity"
        );
    }
}
