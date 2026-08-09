//! The `observe` family: `ingest`, `classify`, `result` — the **producer's** side of the
//! evidence graph, and the half of INV-004 that must never be able to promote.
//!
//! # Why this is a separate module from [`evidence`](super::evidence)
//!
//! It is the enforcement, not the filing. `DaemonState::promote_evidence` takes a
//! [`Promotion`](super::evidence::Promotion), whose fields are private to
//! `daemon::evidence`; a module that is not `daemon::evidence` or one of its descendants
//! cannot build one. This module is neither. So "a producer may not promote its own claim"
//! is not a check anything here performs — it is a value this code cannot construct, and
//! moving the two families into one file would silently give the producer the ability back.
//!
//! The same separation is what makes `evidence.verify`'s self-certification refusal
//! meaningful rather than circular: the verification service compares its own identity
//! against `provenance.actor`, and `provenance.actor` is written *here*, from the admitted
//! capability rather than from anything the request said.
//!
//! # What `observe.ingest` appends, and at what status
//!
//! > Ingest a production trace. Additionally requires the production-trace capability
//! > (plan §18.2); capture-time contract in plan §18.4.
//! >
//! > — the IDL, `observe.ingest`
//!
//! The request declares `trace: Commitment` and `instrumentation_profile: String` and
//! **nothing else**. There is no status field, no confidence field, and no service-identity
//! field, so the strongest thing a producer can say about its own claim is which bytes it
//! is about. The appended node therefore lands at
//! [`ClaimStatus::BOTTOM`](continuum_evidence::claim_status::ClaimStatus::BOTTOM) —
//! `proposed`, the status the lattice documents as "where creation lands" — and there is no
//! request a client could send that would land it anywhere else. That is INV-004's producer
//! half, and it is a property of the wire's shape rather than of a validation this module
//! performs.
//!
//! The `production_trace` grant is not checked here either. It is
//! [`admission::required_grant`](super::admission::required_grant), decided from registry
//! data before any family runs, so an `execute` capability without the grant never reaches
//! this code (RFC 0027 R-4, `rule capability.profile_narrowing`).
//!
//! # Append-only, and what a second identical ingest does
//!
//! > The graph is append-only; nothing is edited in place. […] a replayed write returns the
//! > original node identity.
//! >
//! > — RFC 0038, "Write and concurrency model"
//!
//! An evidence identity here is derived from the trace commitment and the instrumentation
//! profile together — two captures of one trace under two profiles are two observations, so
//! one identity for both would collapse them — and
//! [`DaemonState::append_evidence`](super::state::DaemonState::append_evidence) returns the
//! held node rather than replacing it. So a re-ingest **converges**: the same handle comes
//! back, the stored node is untouched (including any status a verification service has
//! since written to it), and no second `node_published` delta is emitted. Nothing in this
//! module or in `DaemonState` can remove or edit a node; the append-only claim is a
//! property of the surface, and `tests/daemon_evidence.rs` holds it there.
//!
//! # The two operations this module refuses, and why refusing is the honest answer
//!
//! > Registered ahead of its producing subsystem: until the Phase C read-only observation
//! > lane ships, these operations MAY fail with `UnsupportedSemanticFeature`
//! > (`rule errors.unsupported_surface`).
//! >
//! > — the IDL, above the `observe` namespace
//!
//! `observe.classify` returns `classification: Opaque required` and the IDL names no schema
//! for it — unlike `intent.get`'s `record`, which cites
//! `intent-registry-record.schema.json` — so filling those bytes would be inventing wire
//! format, which is the same boundary [`daemon`](super) draws around the codec.
//! `observe.result` returns `VerificationResult`, whose `task` is `required`, and the task
//! service is PR 6. Both therefore return the typed refusal their `errors` clause declares
//! rather than a degraded answer.

use continuum_evidence::claim_status::ClaimStatus;
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::publication::{PublishRefusal, ReferenceStore};

use super::family::{Arguments, Call, Effect, Fault, OperationFamily, Payload, ScopeClaim};
use super::state::{DaemonState, EvidenceNode, StatusWrite};
use super::{Services, identity};
use crate::protocol::envelope::{ArtifactRef, StructuralVerdictValue, Verdict};
use crate::protocol::operations::observe::{ObserveIngestRequest, ObserveIngestResponse};
use crate::protocol::scalar::{ArtifactHandle, EvidenceHandle};
use crate::protocol::spec::{Nullable, Optional};
use crate::protocol::task::EvidenceEvent;
use crate::protocol::vocabulary::{
    ErrorCode, EvidenceEventKind, EvidenceKind, EvidenceNodeKind, StructuralOutcome,
};

/// The `observe` namespace's three operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct ObserveFamily;

/// Every `(operation, code)` pair this family can answer with.
///
/// Held to `rule errors.common` ∪ each operation's `errors` clause by
/// `tests/daemon_evidence.rs`.
pub const FAULTS: &[(&str, ErrorCode)] = &[
    ("observe.ingest", ErrorCode::CapabilityDenied),
    ("observe.ingest", ErrorCode::UnsupportedSemanticFeature),
    ("observe.ingest", ErrorCode::PublicationAborted),
    ("observe.classify", ErrorCode::UnsupportedSemanticFeature),
    ("observe.result", ErrorCode::UnsupportedSemanticFeature),
];

impl OperationFamily for ObserveFamily {
    fn namespace(&self) -> &'static str {
        "observe"
    }

    fn scope(&self, arguments: &Arguments) -> ScopeClaim {
        let claim = || ScopeClaim {
            snapshots: Vec::new(),
            intents: Vec::new(),
            classes: vec![ArtifactClass::Evidence.token()],
        };
        match arguments {
            Arguments::ObserveIngest(_)
            | Arguments::ObserveClassify(_)
            | Arguments::ObserveResult(_) => claim(),
            _ => ScopeClaim::default(),
        }
    }

    fn handle(
        &self,
        call: &Call<'_>,
        state: &mut DaemonState,
        services: &Services,
        store: &ReferenceStore,
    ) -> Result<Effect, Fault> {
        match call.arguments {
            Arguments::ObserveIngest(request) => ingest(call, request, state, services, store),
            Arguments::ObserveClassify(_) => Err(Fault::new(
                ErrorCode::UnsupportedSemanticFeature,
                "the classification artifact this operation returns has no declared shape \
                 in this protocol version",
            )),
            Arguments::ObserveResult(_) => Err(Fault::new(
                ErrorCode::UnsupportedSemanticFeature,
                "the conformance task whose result this operation returns is not served by \
                 this daemon",
            )),
            // Unreachable: the dispatcher checked shape agreement before routing.
            _ => Err(Fault::new(
                ErrorCode::MalformedRequest,
                "the request body is not the shape this operation declares",
            )),
        }
    }
}

fn ingest(
    call: &Call<'_>,
    request: &ObserveIngestRequest,
    state: &mut DaemonState,
    services: &Services,
    store: &ReferenceStore,
) -> Result<Effect, Fault> {
    // Content the daemon does not hold is a denial, never a not-found (RFC 0027 X2). This
    // is also RFC 0038's "rejects nonexistent references", one layer down from the
    // whiteboard compiler that names it.
    let staged = state
        .staged(&request.trace)
        .ok_or_else(Fault::denied)?
        .clone();

    // `provenance.created_at` is required by `evidence-graph-node.schema.json` and time is
    // an explicit effect, never ambient (INV-005, ADR-0003). A daemon built with no clock
    // capability cannot write a conforming provenance record and says so, rather than
    // stamping a node with a time it invented.
    let created_at = services.now().cloned().ok_or_else(|| {
        Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "this daemon holds no time reading, and a provenance record names a capture time",
        )
    })?;

    // The evidence identity: the trace and the profile together, canonically framed so no
    // pair of inputs can produce another pair's preimage by concatenation.
    let handle = evidence_identity(request, services)?;

    // The trace is published under the caller's own capability, so the store decides and
    // audits the write against the identity the wire presented — "authorization separate
    // from handle possession" (ADR-0037), and the appender's identity bound to the append
    // by the store's own record rather than by this function's say-so.
    let token =
        identity::capability_to_store(&call.envelope.capability).map_err(|_| Fault::denied())?;
    store
        .publish(ArtifactClass::Evidence, staged.content.clone(), &token)
        .map_err(|refusal| match refusal {
            PublishRefusal::CapabilityDenied(_) => Fault::denied(),
            PublishRefusal::Aborted(_) => Fault::new(
                ErrorCode::PublicationAborted,
                "the publication aborted; nothing was published and nothing was truncated",
            ),
        })?;

    let node = EvidenceNode {
        // A captured execution is a `run` (plan §11.2), not a certificate and not a proof.
        kind: EvidenceNodeKind::Run,
        evidence_kind: Some(EvidenceKind::ProductionObservation),
        // The claim a production observation is about is the trace's own identity: two
        // ingests of one trace are two statements about one claim, which is what makes the
        // per-claim compare-and-set linearization meaningful.
        claim_id: request.trace.as_str().to_owned(),
        artifact: request.trace.clone(),
        // `provenance.actor` — from the *admitted grant*, not from the request. T4 already
        // bound the envelope's actor to the capability, so this is the identity the daemon
        // authorized, and a producer cannot append under someone else's name.
        producer: call.grant.actor.clone(),
        tool: request.instrumentation_profile.clone(),
        created_at: created_at.clone(),
        inputs: vec![request.trace.as_str().to_owned()],
        // `@mutation` operations carry a non-empty key (`rule idempotency.replay`, enforced
        // in `obligation`), so this is present by the time the family runs.
        idempotency_key: call
            .envelope
            .idempotency_key
            .value()
            .cloned()
            .unwrap_or_default(),
        // A capture carries no producer marker: `labels` is the node schema's optional
        // member and an ingest has nothing to put in it.
        labels: Vec::new(),
        history: vec![StatusWrite {
            // The unpromoted entry status. No request field reaches it.
            status: ClaimStatus::BOTTOM,
            // A producer's append names no service identity, which is exactly why the four
            // assurance-bearing statuses are unreachable from here: their schema
            // conditional requires one.
            service_identity: None,
            validation_basis: None,
            inconclusive_reason: None,
        }],
        redaction: None,
    };

    let (_, appended) = state.append_evidence(handle.clone(), node);
    if appended {
        state.record_evidence_event(EvidenceEvent {
            at: created_at,
            kind: EvidenceEventKind::NodePublished,
            node: Optional::Present(handle.clone()),
            edge: Optional::Absent,
            status: Optional::Absent,
            claim_id: Optional::Present(request.trace.as_str().to_owned()),
        });
    }

    Ok(Effect::new(
        Payload::ObserveIngest(ObserveIngestResponse {
            // "task: TaskHandle optional" — and there is no task: the conformance campaign
            // an ingest would start belongs to the task service (PR 6). Absent is the
            // truthful reading of "not available", never a handle to nothing.
            task: Optional::Absent,
            evidence: vec![handle.clone()],
        }),
        Nullable::Value(Verdict::Structural(StructuralVerdictValue {
            outcome: StructuralOutcome::Created,
        })),
    )
    .with_artifacts(vec![artifact(&handle)?]))
}

/// The identity of the node an ingest appends.
///
/// Derived rather than minted: two identical ingests must converge on one node ("a replayed
/// write returns the original node identity"), and a minted identity would make convergence
/// depend on the idempotency ledger rather than on the content.
///
/// The derivation itself is [`evidence::node_identity`](super::evidence::node_identity),
/// not a local copy of it: `evidence.verify` re-derives the same value as its independent
/// check, and two spellings would let an append and its verification agree while both
/// disagreed with the protocol.
fn evidence_identity(
    request: &ObserveIngestRequest,
    services: &Services,
) -> Result<EvidenceHandle, Fault> {
    super::evidence::node_identity(services, &request.trace, &request.instrumentation_profile)
        .map_err(|_| {
            Fault::new(
                ErrorCode::PublicationAborted,
                "no well-formed content identity could be derived for the evidence node",
            )
        })
}

fn artifact(handle: &EvidenceHandle) -> Result<ArtifactRef, Fault> {
    Ok(ArtifactRef {
        kind: ArtifactClass::Evidence.token().to_owned(),
        handle: ArtifactHandle::new(handle.as_str()).map_err(|_| {
            Fault::new(
                ErrorCode::PublicationAborted,
                "the derived evidence identity is not a well-formed artifact handle",
            )
        })?,
        commitment: Optional::Present(handle_commitment(handle)),
        redacted: Optional::Absent,
    })
}

fn handle_commitment(handle: &EvidenceHandle) -> crate::protocol::scalar::Commitment {
    crate::protocol::scalar::Commitment::new(handle.as_str())
}
