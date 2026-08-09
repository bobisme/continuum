//! The `evidence` family: `get`, `query`, `verify`, `subscribe`, `link` — the daemon's
//! verification service, which is the only thing in this workspace that may advance a
//! claim's status, and the one operation in the protocol that appends an evidence-graph
//! edge.
//!
//! # INV-004, and why it is a property of the module graph
//!
//! > INV-004 — no self-certification: a producer may not promote its own claim, and an
//! > untrusted client cannot reach validated or proved.
//! >
//! > — `crates/continuum-evidence/src/lib.rs`
//!
//! Three documents say the same thing in three vocabularies and none of them leaves room
//! for a caller-declared status:
//!
//! > Actor capabilities control node creation; status promotion is service-restricted. […]
//! > Only trusted services promote into `Validated` or `Proved`. […] Agent votes or
//! > confidence never change status.
//! >
//! > — RFC 0038, "Authority"
//!
//! > No capability at any level may alter an evidence-graph claim's status […] the
//! > evidence-status write […] is performed by a trusted service identity under RFC 0038's
//! > compare-and-set, not by the caller.
//! >
//! > — RFC 0027 A5, and again at H5 and P3
//!
//! > The daemon MUST NOT accept a client-declared status: status is what the checker
//! > establishes, and promotion into `validated` or `proved` is service-restricted.
//! >
//! > — the IDL, `evidence.verify`
//!
//! The enforcement is in four places, and each of them is a *type* rather than a check:
//!
//! 1. **No wire field carries a status into a write.** `evidence.verify`'s only status
//!    input is `expected_status`, which is the compare-and-set *guard* — the status the
//!    caller believes the claim already holds — and it is never the status written. The
//!    written status comes from what the daemon's own check established, computed inside
//!    `EvidenceFamily::check`. A request cannot raise it: it has no field that reaches it.
//! 2. **The producer's append cannot name a status at all.** `observe.ingest` — see
//!    [`observe`](super::observe) — declares `trace` and `instrumentation_profile` and
//!    nothing else, so an appended node lands at [`ClaimStatus::BOTTOM`] as a matter of
//!    the wire's shape.
//! 3. **The promotion write demands a witness only this module can build.**
//!    [`DaemonState::promote_evidence`] takes a [`Promotion`], whose fields are private to
//!    this module and which has no public constructor.
//!    `daemon::observe` — the producer's family — cannot name a value of that type, so the
//!    producer-side code physically cannot reach the status write. This is the same device
//!    [`Denied`](super::admission::Denied) uses for X1: the rule is what compiles.
//! 4. **A verification service may not verify its own production.** The three above stop a
//!    *client* from promoting. They do not stop a deployment that ingests traces under the
//!    same identity it verifies under, which is self-certification with the wire obeyed at
//!    every step. [`EvidenceFamily::service`] is compared against the node's
//!    `provenance.actor` and the promotion is refused when they are equal.
//!
//! # What "independently re-checks" actually means here, and what it does not
//!
//! `evidence.verify` is documented as "run the relevant independent checker over an
//! evidence node", and this daemon ships one: a **re-derivation of the whole reference
//! chain**, through the same [`ContentIdentifier`] seam that named each link.
//!
//! ```text
//! ev_… handle  ──derives from──▶  (artifact commitment, instrumentation profile)
//! artifact commitment  ──derives from──▶  (path, content bytes the daemon holds)
//! ```
//!
//! Both arrows are recomputed and compared, so a node filed under an identity it does not
//! derive, a node whose referent the daemon does not hold, and content that does not hash to
//! the commitment claimed for it are each caught. That is not a formality: it is the
//! difference between a graph that records what a producer *said* and one that records what
//! the daemon can still *confirm*, which is what "no trust-me responses" asks for. The
//! `handle → node` arrow is the one an attacker can actually bend — file a node under
//! someone else's identity, or retarget it at other content — and both attempts are refused
//! with `CertificateRejected` in `tests/daemon_evidence.rs`.
//!
//! The lattice chain is
//!
//! ```text
//! Proposed ⋖ Inconclusive ⋖ Observed ⋖ Sampled ⋖ Bounded ⋖ Validated ⋖ Proved ⋖ Refuted ⋖ Superseded
//! ```
//!
//! and a re-derived reference to a captured execution supports `Observed` — "one or more
//! concrete executions" — and no more. `Sampled` and `Bounded` name a producing engine
//! this daemon does not run and `Proved` needs the Lean kernel, which is out of this
//! process. Asked to verify a node whose class needs one of those, the family answers
//! [`ErrorCode::InsufficientEvidence`] rather than promoting on the strength of a check it
//! did not run. Refusing to overstate *is* INV-004 at the level below authority.
//!
//! # The certificate lane, and what makes `Validated` reachable
//!
//! `Validated` needs the independent certificate checker, and as of bn-dtg61 this daemon
//! reaches one: a node whose `evidence_kind` is `certificate` has its referenced bytes
//! handed to [`continuum_certificate::check_certificate`], which reads the leading
//! eight-byte magic, routes to the `continuum-kernel-*` crate that owns that wire contract,
//! and returns *that kernel's* verdict. Nothing here decodes a certificate, mints a
//! `Verified`, or has a second opinion to offer: `CheckedClaim::new` is `pub(crate)` in
//! each kernel, so this process could not fabricate one if it tried. That is INV-004
//! structurally — the promoting service and the checking code are different crates, and the
//! checking crate cannot link search (`RULE certificate-checker-not-search`).
//!
//! Four outcomes come back, and [`certificate_check`] keeps all four apart on the wire.
//! Their reasoning is written there, beside the `match` that performs it.
//!
//! # Redaction is reported beside the result, never instead of it
//!
//! > `evidence.verify` over a receipt whose referenced content is redacted MUST return the
//! > structural verification result *and* the redaction. It MUST NOT return a bare failure
//! > (the receipt is intact) and MUST NOT return a bare success (the claim is no longer
//! > fully supported).
//! >
//! > — RFC 0026, "Redacted values"
//!
//! So a redacted reference produces `status = ok`, the node's current status *unchanged*
//! (the daemon cannot re-derive what it cannot read, so it promotes nothing), a `semantic`
//! verdict of `inconclusive` with the typed INV-008 reason `InsufficientTelemetry`, the
//! `Redacted` stub in the response, an `omissions` entry with reason `redaction` (INV-007),
//! and every assurance dimension reading `Unsupported`. Each of the five is required by a
//! different sentence and none substitutes for another.
//!
//! [`ClaimStatus::BOTTOM`]: continuum_evidence::claim_status::ClaimStatus::BOTTOM
//! [`ContentIdentifier`]: continuum_workspace::publication::ContentIdentifier
//! [`DaemonState::promote_evidence`]: super::state::DaemonState::promote_evidence

use std::collections::BTreeMap;

use continuum_certificate::{
    Family as CertificateFamily, KernelVerdict, Outcome, RoutingFault, continuum_kernel_core,
    continuum_kernel_sat, continuum_kernel_smt, continuum_kernel_temporal,
};
use continuum_evidence::actor::ServiceIdentity;
use continuum_evidence::claim_status::{ClaimStatus, PromotionRejected, StatusTransition};
use continuum_evidence::edge::{CHECKED_BY_TARGET, CheckTargetRule, CheckerBinding, EdgeRelation};
use continuum_evidence::node::NodeKind;
use continuum_intent::canonical_json::Json;
use continuum_value::assurance::ValidationBasis;
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::publication::ReferenceStore;

use super::family::{
    Arguments, Call, Effect, ErrorData, Fault, OperationFamily, Payload, ScopeClaim,
};
use super::state::{DaemonState, EvidenceEdge, EvidenceNode, StatusWrite};
use super::{Services, identity};
use crate::protocol::envelope::{
    AssuranceEnvelope, CertificateRejection, EnvelopeDimension, Omission, ProducedDimension,
    Redacted, SemanticVerdictValue, UnsupportedDimension, Verdict,
};
use crate::protocol::operations::evidence::{
    EvidenceGetRequest, EvidenceGetResponse, EvidenceLinkRequest, EvidenceLinkResponse,
    EvidenceQueryRequest, EvidenceQueryResponse, EvidenceSubscribeRequest,
    EvidenceSubscribeResponse, EvidenceVerifyRequest, EvidenceVerifyResponse,
};
use crate::protocol::scalar::{ActorId, EvidenceHandle, Opaque};
use crate::protocol::shared::EvidenceQuery;
use crate::protocol::spec::{Nullable, Optional, ProtocolEnum};
use crate::protocol::task::EvidenceEvent;
use crate::protocol::vocabulary::{
    AssuranceClass, ErrorCode, EvidenceEdgeKind, EvidenceEventKind, EvidenceKind, EvidenceNodeKind,
    EvidenceStatus, InconclusiveReason, OmissionReason, SemanticVerdict, StructuralOutcome,
};

/// The `evidence` namespace's four operations, answering as one verification service.
///
/// The service identity is a field rather than a constant because it is *who is doing the
/// checking*, and INV-004 is a statement about that identity being different from the
/// producer's. A daemon whose checker identity were a constant could not express a
/// deployment that runs two of them, and a test could not construct the self-certification
/// case at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceFamily {
    service: ActorId,
}

/// The service identity a daemon verifies under when a deployment names none.
///
/// A `service:` actor, deliberately: an evidence-status write is "performed by a trusted
/// service identity" (RFC 0027 P3), and the wire's `ActorId` scheme already draws the line
/// between `agent:`, `human:`, `service:`, and `ci:`.
pub const DEFAULT_SERVICE: &str = "service:continuumd-verifier";

/// The right to advance a claim's status.
///
/// Its fields are private to this module and it has no public constructor, so the only code
/// in this workspace that can produce one is the verification service below. Every other
/// module — including [`observe`](super::observe), where a producer's append runs — can
/// *name* the type and can never build a value of it, which is what makes
/// "status promotion is service-restricted" (RFC 0038) a compile-time fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Promotion {
    status: ClaimStatus,
    service: String,
    validation_basis: Option<ValidationBasis>,
    inconclusive_reason: Option<InconclusiveReason>,
}

impl Promotion {
    /// The status this promotion would write.
    #[must_use]
    pub const fn status(&self) -> ClaimStatus {
        self.status
    }

    /// The service identity performing it.
    #[must_use]
    pub fn service(&self) -> &str {
        &self.service
    }

    /// The append-only record of this write, at the status the compare-and-set settled on.
    ///
    /// `settled` rather than [`status`](Promotion::status) because a reassertion settles at
    /// the claim's existing status, and the history must record what the claim *holds*
    /// rather than what a caller asked for.
    #[must_use]
    pub fn write(&self, settled: ClaimStatus) -> StatusWrite {
        StatusWrite {
            status: settled,
            // Only the four assurance-bearing statuses are obliged to name a service
            // (`evidence-graph-node.schema.json`), and naming one on `observed` would put a
            // field in the node the schema does not admit there.
            service_identity: SERVICE_ATTRIBUTED
                .contains(&settled)
                .then(|| self.service.clone()),
            validation_basis: self.validation_basis,
            inconclusive_reason: self.inconclusive_reason,
        }
    }
}

/// The statuses whose promotion the node schema requires to name a service identity.
const SERVICE_ATTRIBUTED: [ClaimStatus; 4] = [
    ClaimStatus::Sampled,
    ClaimStatus::Bounded,
    ClaimStatus::Validated,
    ClaimStatus::Proved,
];

/// Every `(operation, code)` pair this family can answer with.
///
/// A data table rather than a comment, so `tests/daemon_evidence.rs` can hold every one of
/// them to `rule errors.common` ∪ the operation's `errors` clause instead of trusting that
/// the handlers stayed inside it.
pub const FAULTS: &[(&str, ErrorCode)] = &[
    ("evidence.get", ErrorCode::CapabilityDenied),
    ("evidence.query", ErrorCode::CapabilityDenied),
    ("evidence.verify", ErrorCode::CapabilityDenied),
    ("evidence.verify", ErrorCode::StatusConflict),
    ("evidence.verify", ErrorCode::CertificateRejected),
    ("evidence.verify", ErrorCode::InsufficientEvidence),
    // The routing failure of the certificate lane. Declared by the operation's own `errors`
    // clause since protocol 3.0 and unreachable until bn-dtg61 gave the daemon a certificate
    // checker to route to; see [`certificate_check`].
    ("evidence.verify", ErrorCode::EpochUnsupported),
    // `evidence.subscribe` names no row. It carried
    // `UnsupportedSemanticFeature` while the channel was unserved, and as of bn-3080b it is
    // served, so the only codes it can answer with are `rule errors.common`'s five — which
    // this table does not enumerate for any operation. Its `errors` clause keeps the code,
    // because a deployment without the transport half still owes that refusal
    // (`rule errors.unsupported_surface`), and a clause that names a code the operation does
    // not currently produce is a permission, not an obligation.
    ("evidence.link", ErrorCode::CapabilityDenied),
    ("evidence.link", ErrorCode::InsufficientEvidence),
    ("evidence.link", ErrorCode::UnsupportedSemanticFeature),
    ("evidence.link", ErrorCode::PublicationAborted),
];

/// The `$id` of the governing schema, with the `v<schema_epoch>/` segment removed
/// (`schemas/README.md`). Stable across schema epochs.
const NODE_SCHEMA_ID: &str = "https://continuum.dev/schema/evidence-graph-node.json";

/// The schema epoch this daemon writes evidence-graph nodes at.
const NODE_SCHEMA_EPOCH: i64 = 1;

/// The `$id` of the governing edge schema, with the `v<schema_epoch>/` segment removed.
const EDGE_SCHEMA_ID: &str = "https://continuum.dev/schema/evidence-graph-edge.json";

/// The schema epoch this daemon writes evidence-graph edges at.
const EDGE_SCHEMA_EPOCH: i64 = 1;

/// The domain tag that separates an edge preimage from a node preimage
/// (`rule evidence.edge_identity`).
///
/// It carries a `/`, which is outside the artifact-handle character class
/// `^[a-z][a-z0-9_]*_[A-Za-z0-9_-]+$` that every `Commitment` this daemon derives belongs
/// to. A node preimage's first part is such a commitment, so no node preimage can spell an
/// edge preimage's first part — which is what makes the two derivations disjoint rather
/// than merely unlikely to collide.
pub const EDGE_IDENTITY_DOMAIN: &str = "evidence-graph-edge/v1";

impl Default for EvidenceFamily {
    fn default() -> Self {
        Self::new()
    }
}

impl EvidenceFamily {
    /// The family verifying under [`DEFAULT_SERVICE`].
    ///
    /// # Panics
    ///
    /// Never: [`DEFAULT_SERVICE`] is a compile-time constant inside `ActorId`'s pattern,
    /// and `tests/daemon_evidence.rs` pins that it parses.
    #[must_use]
    pub fn new() -> Self {
        Self {
            service: ActorId::new(DEFAULT_SERVICE)
                .expect("the default service identity is a well-formed `service:` actor"),
        }
    }

    /// The family verifying under a deployment's own service identity.
    #[must_use]
    pub const fn verifying_as(service: ActorId) -> Self {
        Self { service }
    }

    /// The identity this service performs its status writes under (INV-004).
    #[must_use]
    pub const fn service(&self) -> &ActorId {
        &self.service
    }
}

impl OperationFamily for EvidenceFamily {
    fn namespace(&self) -> &'static str {
        "evidence"
    }

    fn scope(&self, arguments: &Arguments) -> ScopeClaim {
        let claim = |classes: Vec<&'static str>| ScopeClaim {
            snapshots: Vec::new(),
            intents: Vec::new(),
            classes,
        };
        let evidence = ArtifactClass::Evidence.token();
        match arguments {
            Arguments::EvidenceGet(_)
            | Arguments::EvidenceVerify(_)
            | Arguments::EvidenceLink(_) => claim(vec![evidence]),
            // A query names no instance it is authorized *against* — its `roots` are where
            // a traversal starts, not artifacts it is entitled to — so the class is the
            // whole scope claim, and every node it would return is filtered against the
            // descriptor again in `visible`.
            Arguments::EvidenceQuery(_) | Arguments::EvidenceSubscribe(_) => claim(vec![evidence]),
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
            Arguments::EvidenceGet(request) => get(request, state),
            Arguments::EvidenceQuery(request) => query(request, state),
            Arguments::EvidenceVerify(request) => self.verify(request, state, services),
            Arguments::EvidenceSubscribe(request) => subscribe(request, state),
            Arguments::EvidenceLink(request) => link(call, request, state, services, store),
            // Unreachable: the dispatcher checked shape agreement against the registry
            // before routing. A typed refusal rather than an `unreachable!`, because a
            // daemon does not abort on its own invariant.
            _ => Err(Fault::new(
                ErrorCode::MalformedRequest,
                "the request body is not the shape this operation declares",
            )),
        }
    }
}

// --- evidence.get -----------------------------------------------------------------------

fn get(request: &EvidenceGetRequest, state: &DaemonState) -> Result<Effect, Fault> {
    // One handle class names both halves of the graph (`^ev_[A-Za-z0-9_-]+$` is the pattern
    // of `node_id` *and* `edge_id`), so this operation resolves either. The edge arm is
    // reachable as of protocol 3.3, when `evidence.link` gave the graph its first edges.
    if let Some(edge) = state.evidence_edge(&request.evidence) {
        return Ok(Effect::new(
            Payload::EvidenceGet(EvidenceGetResponse {
                node: Nullable::Null,
                edge: Nullable::Value(Opaque::from_bytes(
                    edge_record(&request.evidence, edge).to_canonical_bytes(),
                )),
                // A redaction is a property of content a *node* references; an edge
                // references none.
                redacted: Optional::Absent,
            }),
            Nullable::Null,
        ));
    }

    // A node the graph does not hold is a denial, not a not-found: "a read of an artifact
    // the caller is not authorized for MUST return `CapabilityDenied` whether or not the
    // artifact exists; a distinct not-found *is* an existence oracle" (RFC 0027 X2).
    let node = state
        .evidence(&request.evidence)
        .ok_or_else(Fault::denied)?;

    // `inline` is "include bounded inline content", and this daemon has no bounded-content
    // channel in the response body — `node` and `edge` are the node record itself — so a
    // request asking for it gets the record plus a typed omission rather than silence.
    let inline = matches!(request.inline, Optional::Present(true));
    let mut omissions = Vec::new();
    if inline {
        omissions.push(Omission {
            reason: OmissionReason::Unsupported,
            subject: "evidence.inline_content".to_owned(),
            recoverable_by: Optional::Absent,
        });
    }

    // > A value the daemon cannot return in full MUST be returned as the typed `Redacted`
    // > stub […] Omitting the field, returning null, or returning an empty value MUST NOT
    // > be used to represent redaction — a client MUST be able to tell "withheld" from
    // > "absent" structurally, without inference.
    // >
    // > — RFC 0026, "Redacted values"
    //
    // The node *record* is not the redacted thing; the content it references is. So the
    // record is still returned and the stub travels beside it, and the INV-007 manifest
    // names the redaction as well ("Every redacted value MUST also appear in the result's
    // `omissions` manifest with reason `redaction`").
    let redacted = match &node.redaction {
        Some(redaction) => {
            omissions.push(Omission {
                reason: OmissionReason::Redaction,
                subject: node.claim_id.clone(),
                recoverable_by: Optional::Absent,
            });
            Optional::Present(redaction.clone())
        }
        None => Optional::Absent,
    };

    Ok(Effect::new(
        Payload::EvidenceGet(EvidenceGetResponse {
            node: Nullable::Value(Opaque::from_bytes(
                node_record(&request.evidence, node).to_canonical_bytes(),
            )),
            // The handle named a node, so it does not name an edge: `null` is the
            // declared reading of "this handle does not name one", and the two arms are
            // exclusive because one map is keyed by node handles and the other by edge
            // handles.
            edge: Nullable::Null,
            redacted,
        }),
        // `evidence.get` declares no `verdict` clause, so its result carries none.
        Nullable::Null,
    )
    .with_omissions(omissions))
}

/// One evidence node as `schemas/evidence-graph-node.schema.json` writes it.
///
/// Built here rather than stored, so the record a client reads is derived from the state
/// the daemon actually holds and cannot drift from it. The conditional members —
/// `service_identity`, `validation_basis`, `inconclusive_reason` — are emitted exactly when
/// the schema's three `if`/`then` clauses require them, and omitted otherwise:
/// `additionalProperties` is false, so a field written where the schema does not admit it
/// is as invalid as one missing where it does.
fn node_record(handle: &EvidenceHandle, node: &EvidenceNode) -> Json {
    let mut fields: BTreeMap<String, Json> = BTreeMap::new();
    let mut put = |key: &str, value: Json| {
        fields.insert(key.to_owned(), value);
    };
    put("schema_id", Json::String(NODE_SCHEMA_ID.to_owned()));
    put("schema_epoch", Json::Integer(NODE_SCHEMA_EPOCH));
    put("node_id", Json::String(handle.as_str().to_owned()));
    put("kind", Json::String(node_kind_token(node.kind).to_owned()));
    put("artifact", Json::String(node.artifact.as_str().to_owned()));
    put("status", Json::String(node.status().as_str().to_owned()));
    put("claim_id", Json::String(node.claim_id.clone()));
    put(
        "idempotency_key",
        Json::String(node.idempotency_key.clone()),
    );
    // `labels` is optional and a node with none has none: an empty array would be a
    // producer's assertion that it considered the question and had nothing to say.
    if !node.labels.is_empty() {
        put(
            "labels",
            Json::Array(
                node.labels
                    .iter()
                    .map(|label| Json::String(label.clone()))
                    .collect(),
            ),
        );
    }

    let mut provenance: BTreeMap<String, Json> = BTreeMap::new();
    provenance.insert(
        "actor".to_owned(),
        Json::String(node.producer.as_str().to_owned()),
    );
    provenance.insert(
        "created_at".to_owned(),
        Json::String(node.created_at.as_str().to_owned()),
    );
    provenance.insert(
        "inputs".to_owned(),
        Json::Array(
            node.inputs
                .iter()
                .map(|input| Json::String(input.clone()))
                .collect(),
        ),
    );
    // `provenance.tool` is optional in the schema, and a producer that named no tool has
    // no tool: an empty string would be a name. `whiteboard.compile` is the first producer
    // that can reach the absent case, because a note's `tool` is the one member of the
    // format that is optional rather than empty-able (RFC 0038 W1).
    if !node.tool.is_empty() {
        provenance.insert("tool".to_owned(), Json::String(node.tool.clone()));
    }
    put("provenance", Json::Object(provenance));

    if let Some(write) = node.history.last() {
        if let Some(service) = &write.service_identity {
            put("service_identity", Json::String(service.clone()));
        }
        if let Some(basis) = write.validation_basis {
            put("validation_basis", Json::String(basis.as_str().to_owned()));
        }
        if let Some(reason) = write.inconclusive_reason {
            put(
                "inconclusive_reason",
                Json::String(reason.as_wire().to_owned()),
            );
        }
    }
    if let Some(redaction) = &node.redaction {
        put(
            "redacted_references",
            Json::Array(vec![redacted_json(redaction)]),
        );
    }
    Json::Object(fields)
}

fn redacted_json(redaction: &Redacted) -> Json {
    let mut fields: BTreeMap<String, Json> = BTreeMap::new();
    fields.insert("redacted".to_owned(), Json::Bool(redaction.redacted));
    fields.insert(
        "reason".to_owned(),
        Json::String(redaction.reason.as_wire().to_owned()),
    );
    fields.insert(
        "commitment".to_owned(),
        Json::String(redaction.commitment.as_str().to_owned()),
    );
    fields.insert(
        "original_class".to_owned(),
        Json::String(redaction.original_class.clone()),
    );
    Json::Object(fields)
}

// --- evidence.query ---------------------------------------------------------------------

fn query(request: &EvidenceQueryRequest, state: &DaemonState) -> Result<Effect, Fault> {
    let (nodes, edges) = selected(&request.query, state);
    Ok(Effect::new(
        Payload::EvidenceQuery(EvidenceQueryResponse { nodes, edges }),
        Nullable::Null,
    ))
}

/// Every node and edge one [`EvidenceQuery`] selects, in the graph's own order.
///
/// Factored out because two operations answer with it and they must not drift:
/// `evidence.query` returns it as its two lists, and `evidence.subscribe` returns it as the
/// `frontier` its declared scope stands at. `rule subscription.delivery` makes that agreement
/// normative — "a delta is in scope exactly when the scope's `EvidenceQuery` selects the
/// artifact the delta's own kind names, by the same predicate `evidence.query` answers with"
/// — so a second spelling of the filter would be a second answer to one question.
fn selected(
    query: &EvidenceQuery,
    state: &DaemonState,
) -> (Vec<EvidenceHandle>, Vec<EvidenceHandle>) {
    let reach = Reach::of(query, state);
    let nodes = state
        .evidence_nodes()
        .filter(|(handle, node)| matches(query, &reach, handle, node))
        .map(|(handle, _)| handle.clone())
        .collect();
    let edges = state
        .evidence_edges()
        .filter(|(handle, edge)| matches_edge(query, &reach, handle, edge))
        .map(|(handle, _)| handle.clone())
        .collect();
    (nodes, edges)
}

/// The depth of every artifact an [`EvidenceQuery`]'s `roots` reach, or [`Reach::Whole`]
/// when the query names no root (`rule evidence.traversal`).
///
/// This is the traversal half of the query predicate, computed once per query and consulted
/// per artifact — a `roots` clause is a statement about the *graph*, and re-deriving it for
/// each candidate would be re-walking the graph once per node.
///
/// # Why an edge is walked in both directions
///
/// An edge's direction is what it **asserts**, not which way relevance runs. Every
/// `SUPPORTS`, `REFUTES`, and `COUNTEREXAMPLE_TO` edge names the claim as its `to`, so a
/// forward-only walk from a claim would reach *none* of the evidence offered to it — which
/// is the one question the graph exists to answer ("one counterexample refutes several
/// candidates; one invariant supports many properties", RFC 0038 "Why a graph"). Reverse-only
/// would fail the mirror case: `certificate CHECKED_BY receipt` points away from its subject.
/// Picking either arrow would be inventing an answer no source took, so an edge is an
/// adjacency (`rule evidence.traversal`, clause 1).
///
/// # Why an edge takes the greater of its endpoints' depths
///
/// So that an answer names no dangling edge. `depth(edge) = max(depth(from), depth(to)) <=
/// max_depth` implies both endpoints are within the bound too, which is "references must
/// resolve" — the graph's own structural refusal (RFC 0038's whiteboard compiler, docs/44) —
/// holding for a *query answer* and not only for a write. An edge named directly in `roots`
/// is at 0 with both its endpoints, for the same reason: half of an assertion is not an
/// assertion.
#[derive(Debug)]
enum Reach {
    /// `roots` is absent or empty: "the whole graph in scope", the field's declared
    /// sentence. There is no root to measure a depth from, so `max_depth` excludes nothing
    /// (`rule evidence.traversal`, clause 6).
    Whole,
    /// The depth of each reached artifact, keyed by handle. A handle absent from the map was
    /// not reached at all and is out of scope whatever `max_depth` says.
    From(BTreeMap<EvidenceHandle, u32>),
}

impl Reach {
    /// Walk the graph from `query`'s roots, breadth-first, recording each artifact's depth.
    fn of(query: &EvidenceQuery, state: &DaemonState) -> Self {
        let roots = match &query.roots {
            Optional::Present(roots) if !roots.is_empty() => roots,
            _ => return Self::Whole,
        };
        // `max_depth` absent is unbounded, and `u32::MAX` is that bound expressed in the
        // field's own type: the graph is finite and append-only, so a walk terminates on
        // exhausting it long before the counter could saturate.
        let bound = query.max_depth.value().copied().unwrap_or(u32::MAX);

        // Adjacency is built once, from the edge set, and is symmetric by construction:
        // clause 1 is a property of this map rather than a branch in the loop below.
        let mut incident: BTreeMap<&EvidenceHandle, Vec<&EvidenceHandle>> = BTreeMap::new();
        for (_, edge) in state.evidence_edges() {
            incident.entry(&edge.from).or_default().push(&edge.to);
            incident.entry(&edge.to).or_default().push(&edge.from);
        }

        let mut depth: BTreeMap<EvidenceHandle, u32> = BTreeMap::new();
        let mut frontier: Vec<EvidenceHandle> = Vec::new();
        for root in roots {
            // A root naming an edge seeds that edge *and both its endpoints* at 0. A root
            // naming a node, or a handle the graph does not hold, seeds only itself — an
            // unheld handle then reaches nothing, which is a refusal to invent a
            // neighborhood around an artifact that does not exist.
            if let Some(edge) = state.evidence_edge(root) {
                for endpoint in [&edge.from, &edge.to] {
                    if depth.insert(endpoint.clone(), 0).is_none() {
                        frontier.push(endpoint.clone());
                    }
                }
            }
            if depth.insert(root.clone(), 0).is_none() && state.evidence_edge(root).is_none() {
                frontier.push(root.clone());
            }
        }

        // Breadth-first, so the first depth recorded for a node is the least one — the
        // shortest undirected path from any root, which is what clause 2 defines.
        let mut current = 0_u32;
        while !frontier.is_empty() && current < bound {
            let mut next = Vec::new();
            for handle in &frontier {
                for other in incident.get(handle).into_iter().flatten() {
                    if !depth.contains_key(*other) {
                        depth.insert((*other).clone(), current + 1);
                        next.push((*other).clone());
                    }
                }
            }
            frontier = next;
            current += 1;
        }

        // An edge's own depth is the greater of its endpoints', assigned after the node walk
        // so that both are final. An edge already at 0 by being a root keeps that: it is the
        // lesser of the two derivations, and a root is a root.
        let edge_depths: Vec<(EvidenceHandle, u32)> = state
            .evidence_edges()
            .filter_map(|(handle, edge)| {
                let from = *depth.get(&edge.from)?;
                let to = *depth.get(&edge.to)?;
                Some((handle.clone(), from.max(to)))
            })
            .collect();
        for (handle, at) in edge_depths {
            let entry = depth.entry(handle).or_insert(at);
            *entry = (*entry).min(at);
        }
        Self::From(depth)
    }

    /// Whether one artifact is within the traversal's bound.
    ///
    /// The bound is re-applied here rather than trusted from the walk because the walk stops
    /// expanding at `max_depth` but still assigns edge depths afterwards, and an edge across
    /// the frontier can land one past it.
    fn holds(&self, query: &EvidenceQuery, handle: &EvidenceHandle) -> bool {
        match self {
            Self::Whole => true,
            Self::From(depth) => {
                let bound = query.max_depth.value().copied().unwrap_or(u32::MAX);
                depth.get(handle).is_some_and(|at| *at <= bound)
            }
        }
    }
}

/// Whether one edge satisfies an [`EvidenceQuery`].
///
/// The clauses that name *node* properties — `node_kinds`, `statuses`, `claim_id` — select
/// no edge when they are present, because an edge has none of them and answering "matches"
/// would be answering a question that was not asked. `edge_kinds` and `roots` are the two
/// that apply, and `roots` is the traversal of `rule evidence.traversal` rather than handle
/// membership: an edge is in scope when its depth — the greater of its two endpoints' — is
/// within `max_depth`.
fn matches_edge(
    query: &EvidenceQuery,
    reach: &Reach,
    handle: &EvidenceHandle,
    edge: &EvidenceEdge,
) -> bool {
    for absent in [
        query.node_kinds.value().map(|kinds| kinds.is_empty()),
        query.statuses.value().map(|statuses| statuses.is_empty()),
    ] {
        if absent == Some(false) {
            return false;
        }
    }
    if query.claim_id.value().is_some() {
        return false;
    }
    if let Optional::Present(kinds) = &query.edge_kinds {
        if !kinds.is_empty() && !kinds.contains(&wire_edge_kind(edge.relation.kind())) {
            return false;
        }
    }
    reach.holds(query, handle)
}

/// Whether one node satisfies an [`EvidenceQuery`].
///
/// Every clause is a conjunction and an absent clause filters nothing, which is the reading
/// the IDL fixes by declaring each field `optional` and by documenting `roots` as "empty
/// means the whole graph in scope". `roots` and `max_depth` are the traversal
/// `rule evidence.traversal` states — an undirected walk from the roots, bounded in edges —
/// and it is computed over the **whole** graph by [`Reach::of`], not over the sub-graph the
/// other clauses admit: a walk that could only pass through nodes matching `statuses` would
/// mean "paths through `validated` nodes", which is one clause changing another's meaning
/// rather than a conjunction (clause 5).
///
/// Until bn-35l6g this read `roots` as exact handle membership and ignored `max_depth`
/// outright, on the recorded ground that "this graph has no edges to traverse". That was
/// true when it was written and stopped being true at protocol 3.3, when `evidence.link`
/// gave the graph its first edges — so the bound was silently dropped rather than served,
/// which is the "degrading, guessing" `rule errors.unsupported_surface` forbids. The
/// membership reading is still expressible, and is now `max_depth = 0`.
fn matches(
    query: &EvidenceQuery,
    reach: &Reach,
    handle: &EvidenceHandle,
    node: &EvidenceNode,
) -> bool {
    if let Optional::Present(kinds) = &query.node_kinds {
        if !kinds.is_empty() && !kinds.contains(&node.kind) {
            return false;
        }
    }
    if let Optional::Present(statuses) = &query.statuses {
        let status = wire_status(node.status());
        if !statuses.is_empty() && !statuses.contains(&status) {
            return false;
        }
    }
    if let Optional::Present(claim) = &query.claim_id {
        if claim != &node.claim_id {
            return false;
        }
    }
    reach.holds(query, handle)
}

// --- evidence.subscribe -----------------------------------------------------------------

/// `evidence.subscribe` — the scope's frontier now; the deltas are the committed writes
/// that follow.
///
/// > `evidence.subscribe` streams typed evidence-graph deltas for a declared scope; the
/// > graph itself remains authoritative on reconnect. A dropped subscription changes
/// > nothing (INV-002); a client MUST be able to recover the same state by re-reading.
/// >
/// > — `rule subscription.hints_only`
///
/// # Why this was a refusal until bn-3080b, and what changed
///
/// It answered `UnsupportedSemanticFeature`, and that was the right answer at the time: the
/// *operation* is a stream, `events EvidenceEvent` is where the deltas go, and nothing on
/// the wire delivered one. A frontier returned by a subscription whose deltas no client
/// could read is the "empty success" `rule errors.unsupported_surface` forbids — worse than
/// a refusal, because a client would believe it was subscribed.
///
/// What was missing was never daemon-side state: every committed delta has been recorded in
/// [`DaemonState::evidence_events`](super::state::DaemonState::evidence_events) since the
/// graph landed. It was the *spelling* of the channel — what an event frame is, how a client
/// tells one from a `ResultEnvelope`, which deltas a scope selects — and no normative source
/// fixed it. `rule subscription.delivery` (IDL 1.7) does, so this operation is now served,
/// and `UnsupportedSemanticFeature` remains the answer of a deployment that does not carry
/// the transport half: "whether a lane has shipped is a property of a deployment, not of an
/// operation" (`rule errors.common`, as relaxed at 3.2).
///
/// # What this layer answers, and what it deliberately does not
///
/// The IDL's response body is exactly one field — "the scope's current frontier at
/// subscription time" — and that is what this answers, through [`selected`]: the same
/// predicate `evidence.query` answers with, so the frontier and a query over the identical
/// scope cannot disagree. The frames that follow are the transport's, because a *stream* is
/// a transport object and this layer emits no wire bytes (see [`daemon`](super)'s "no
/// codec"); [`in_scope`] is the predicate it filters the delta log with, exported here so
/// the rule lives beside the query it is defined in terms of rather than being restated at
/// the byte boundary.
///
/// The subscription's cursor is not held here either, and that is INV-002 rather than an
/// omission: a cursor is per-connection, the log it points into is the daemon's, and a
/// daemon that remembered which client had read how far would be holding exactly the session
/// state RFC 0026 says it holds none of. Dropping a connection loses the cursor and no
/// artifact.
fn subscribe(request: &EvidenceSubscribeRequest, state: &DaemonState) -> Result<Effect, Fault> {
    // One list on the wire: `frontier: list<EvidenceHandle>`, and the handle class names
    // both halves of the graph. Nodes before edges, each in the graph's own order, so two
    // calls over one scope answer identically.
    let (mut frontier, edges) = selected(&request.scope, state);
    frontier.extend(edges);
    Ok(Effect::new(
        Payload::EvidenceSubscribe(EvidenceSubscribeResponse { frontier }),
        // `evidence.subscribe` declares no `verdict` clause, so its result carries none.
        Nullable::Null,
    ))
}

/// Whether one committed delta is in a subscription's declared scope
/// (`rule subscription.delivery`, clause 3).
///
/// The delta's `kind` decides which member names its artifact, and nothing else does:
/// `node_published`, `status_transition`, and `conflict_materialized` name the `node`,
/// `edge_published` names the `edge`. Reading whichever member happens to be present would
/// let a mislabeled event reach a scope by filling the other one, and the vocabulary already
/// says which artifact each kind is about — so the vocabulary decides, and an event whose
/// kind's member is absent names no artifact and is in no scope.
///
/// The selection itself is [`matches`]/[`matches_edge`] — `evidence.query`'s own predicate,
/// applied to the graph **as it stands now** rather than as it stood when the delta was
/// committed. That is the rule's own choice and it is the honest one: the graph is
/// authoritative and a delta is a pointer into it, never a copy of it
/// (`rule subscription.hints_only`), so a scope naming `statuses: [validated]` follows the
/// claim rather than freezing a status the claim has since left behind.
///
/// "Now" is the delivery pass, and a caller asks *once per delta*: the transport's cursor
/// moves past a delta whether or not this answered `true` for it, which is what makes a
/// subscription a cursor rather than a filter re-run over history. The consequence is a real
/// limit and the rule states it rather than hiding it — a delta a later write would have
/// brought inside a scope may simply not be delivered, and the client recovers it by
/// re-reading. A stream that promised otherwise would be promising to be the graph.
///
/// A delta naming an artifact the graph does not hold is in no scope. The graph is
/// append-only, so that is unreachable today; it is written as a refusal rather than an
/// assumption because "unreachable" is a property of the current write set and not of this
/// function.
///
/// The traversal of `rule evidence.traversal` is part of that predicate and so is re-walked
/// here, per delta, against the graph as it stands. That is the same "evaluated ONCE per
/// delta per subscription, at the first delivery after that delta was committed" the rule
/// already states, read for a scope whose selection depends on the *edge set*: an edge
/// committed after a delta can bring a later delta inside a scope, and does not retroactively
/// deliver an earlier one. A subscription is a cursor, and re-reading is what recovers the
/// difference.
#[must_use]
pub fn in_scope(scope: &EvidenceQuery, event: &EvidenceEvent, state: &DaemonState) -> bool {
    let reach = Reach::of(scope, state);
    match event.kind {
        EvidenceEventKind::NodePublished
        | EvidenceEventKind::StatusTransition
        | EvidenceEventKind::ConflictMaterialized => match &event.node {
            Optional::Present(handle) => state
                .evidence(handle)
                .is_some_and(|node| matches(scope, &reach, handle, node)),
            Optional::Absent => false,
        },
        EvidenceEventKind::EdgePublished => match &event.edge {
            Optional::Present(handle) => state
                .evidence_edge(handle)
                .is_some_and(|edge| matches_edge(scope, &reach, handle, edge)),
            Optional::Absent => false,
        },
    }
}

// --- evidence.verify --------------------------------------------------------------------

/// What the daemon's independent check established about one node.
enum Checked {
    /// The reference re-derived, and the check supports this status.
    ///
    /// The third member names the trusted checker that answered, when one did. `None` is the
    /// observation lane, where the daemon's own re-derivation is the whole check; `Some` is
    /// the certificate lane, and the `proof_status` dimension of the assurance envelope
    /// names that kernel rather than this service.
    Establishes(ClaimStatus, ValidationBasis, Option<CertificateFamily>),
    /// The referenced content is redacted: the receipt is intact and the claim is no longer
    /// fully supported, so nothing is promoted and the redaction is reported beside the
    /// structural result.
    Redacted(Redacted),
    /// The routed kernel read the artifact and named a feature it does not implement.
    ///
    /// A verdict, not a failure: the checker ran and answered. Nothing is promoted, and the
    /// answer carries INV-008's typed `Unsupported` reason rather than a rejection — see
    /// [`certificate_check`].
    Unsupported(CertificateFamily),
}

impl EvidenceFamily {
    fn verify(
        &self,
        request: &EvidenceVerifyRequest,
        state: &mut DaemonState,
        services: &Services,
    ) -> Result<Effect, Fault> {
        let node = state
            .evidence(&request.evidence)
            .ok_or_else(Fault::denied)?
            .clone();

        // INV-004, dimension four. Checked before the content is even looked at, because a
        // verification service that is the claim's own producer has nothing to establish
        // whatever the bytes say, and because a refusal that depended on the artifact would
        // be a refusal that leaked something about it.
        if node.producer == self.service {
            return Err(Fault::new(
                ErrorCode::InsufficientEvidence,
                "a producer may not promote its own claim: this daemon's verification \
                 service is the identity that appended this node",
            ));
        }

        // A node that offers no evidence class has no checker to run. This is the case a
        // whiteboard proposal lands in (`whiteboard.compile`): it sits at `proposed` and
        // asserts nothing, so there is nothing for an independent checker to re-derive and
        // no member of `EvidenceKind` that would be true of it. Refusing is the same act
        // the lane match below performs for the ten classes this daemon cannot check —
        // stated here because it is a fact about the *node*, not about which checker this
        // build ships, and because the response's `evidence_kind` is `required` and there
        // is no honest value for it. RFC 0026 F21 records the vocabulary gap.
        let Some(evidence_kind) = node.evidence_kind else {
            return Err(Fault::new(
                ErrorCode::InsufficientEvidence,
                "this node offers no class of evidence toward its claim, so no independent \
                 checker applies to it; nothing is promoted on a check that cannot be run",
            ));
        };

        let checked = self.check(&request.evidence, &node, state, services)?;

        let expected = match &request.expected_status {
            // The caller stated what it believes the claim holds. This is the CAS guard and
            // is never the status written — `rule` and RFC alike forbid a client-declared
            // status, and the value below comes from `checked`.
            Optional::Present(status) => lattice_status(*status),
            // No guard: compare against what the claim holds now, which still refuses a
            // regression because `compare_and_set` checks the lattice as well as the guard.
            Optional::Absent => node.status(),
        };

        let (status, basis, verdict, assurance, redacted, omissions) = match checked {
            Checked::Establishes(establishes, basis, checker) => {
                let promotion = Promotion {
                    status: establishes,
                    service: self.service.as_str().to_owned(),
                    validation_basis: (establishes == ClaimStatus::Validated).then_some(basis),
                    inconclusive_reason: None,
                };
                let settled = state
                    .promote_evidence(&request.evidence, expected, &promotion)
                    .map_err(status_fault)?;
                if let Some(at) = services.now() {
                    state.record_evidence_event(crate::protocol::task::EvidenceEvent {
                        at: at.clone(),
                        kind: crate::protocol::vocabulary::EvidenceEventKind::StatusTransition,
                        node: Optional::Present(request.evidence.clone()),
                        edge: Optional::Absent,
                        status: Optional::Present(wire_status(settled)),
                        claim_id: Optional::Present(node.claim_id.clone()),
                    });
                }
                (
                    settled,
                    basis,
                    semantic(SemanticVerdict::Established, None, assurance_class(settled)),
                    verify_envelope(
                        self.service.as_str(),
                        // A promotion that landed where the check pointed says what the check
                        // did; a reassertion that settled at a higher status the claim
                        // already held says that instead, because the re-derivation is not
                        // what the status now rests on.
                        match settled {
                            ClaimStatus::Observed | ClaimStatus::Validated => {
                                "reference re-derived from held content"
                            }
                            _ => "status settled by compare-and-set",
                        },
                        checked_proof_status(checker),
                    ),
                    Optional::Absent,
                    Vec::new(),
                )
            }
            // The routed kernel answered `Unsupported`. Nothing is promoted — an artifact
            // naming a contract its own checker does not implement supports no status — and
            // the answer is a *result* rather than an error, because `verdict` is null on
            // `status = error` (`ResultEnvelope`) and `SemanticVerdictValue`'s
            // `inconclusive_reason` is the only channel INV-008's typed reason has. An error
            // here would carry the fact that something was inconclusive and lose which of the
            // six reasons it was.
            //
            // Promoting to `Inconclusive` instead was considered and declined: the lattice
            // puts `Inconclusive` below `Observed`, so an honest "I cannot answer" over a node
            // that already holds anything higher would come back as `StatusConflict` — a
            // failure to write, reported in place of the verdict the checker actually gave.
            Checked::Unsupported(family) => (
                node.status(),
                // No certificate was checked, so the stronger of the two bases would be a
                // claim nothing backs (plan §11.4: "the two never render identically").
                ValidationBasis::TrustedSolver,
                semantic(
                    SemanticVerdict::Inconclusive,
                    Some(InconclusiveReason::Unsupported),
                    // The class the node's evidence is supported at, which this answer did
                    // not raise.
                    AssuranceClass::Observed,
                ),
                verify_envelope(
                    self.service.as_str(),
                    "reference re-derived from held content",
                    unsupported_dimension(unsupported_feature(family)),
                ),
                Optional::Absent,
                Vec::new(),
            ),
            Checked::Redacted(redaction) => (
                // Nothing is promoted: the daemon cannot re-derive what it cannot read.
                node.status(),
                // No check ran, so the weaker of the two bases the wire admits is the only
                // honest one: "no independently checked certificate exists". Reporting
                // `checked-certificate` over content nobody could read would be the
                // full-strength claim RFC 0026 forbids over a redacted input.
                ValidationBasis::TrustedSolver,
                semantic(
                    SemanticVerdict::Inconclusive,
                    Some(InconclusiveReason::InsufficientTelemetry),
                    // "A verdict MUST NOT be reported at full strength over a redacted
                    // input on the grounds that the input 'would have' supported it."
                    AssuranceClass::Observed,
                ),
                withheld_envelope(),
                Optional::Present(redaction),
                vec![Omission {
                    reason: OmissionReason::Redaction,
                    subject: node.claim_id.clone(),
                    recoverable_by: Optional::Absent,
                }],
            ),
        };

        Ok(Effect::new(
            Payload::EvidenceVerify(EvidenceVerifyResponse {
                evidence: request.evidence.clone(),
                status: wire_status(status),
                evidence_kind,
                // "The checker's service identity (INV-004)." The daemon's, never the
                // caller's: `call.grant.actor` is who *asked*, and who asked is not who
                // checked.
                checker: self.service.as_str().to_owned(),
                validation_basis: basis.as_str().to_owned(),
                redacted,
            }),
            verdict,
        )
        .with_assurance(assurance)
        .with_omissions(omissions))
    }

    /// Run the independent check this daemon ships, in four steps.
    ///
    /// The steps close a chain, and each link is *re-derived* rather than read back:
    ///
    /// | # | Link | Refusal |
    /// |---|---|---|
    /// | 1 | the class of checker this evidence needs is one this daemon can reach | `InsufficientEvidence` |
    /// | 2 | the reference resolves — the daemon holds content under the identity the node names | `InsufficientEvidence` |
    /// | 3 | the bytes it holds re-derive to that identity, and the node re-derives to the identity it is *filed under* | `CertificateRejected` |
    /// | 4 | for the certificate lane only: the trusted checking base's answer on those bytes | [`certificate_check`] |
    ///
    /// Steps 2 and 3 run in **both** lanes, and deliberately run *before* the kernel. They
    /// are what stops a certificate node from being filed under someone else's identity or
    /// retargeted at other content — an attack the kernel cannot see, because a
    /// well-formed certificate is well-formed wherever it is filed. So a caller that
    /// reaches step 4 at all has already had its reference confirmed, and the `values`
    /// dimension of the assurance envelope is produced on the certificate lane for exactly
    /// that reason.
    ///
    /// Step 3's second half is the one that makes this a check rather than a formality. A
    /// node's identity is a function of what it is about — the referenced commitment and the
    /// instrumentation profile — so a node filed in the graph under an identity it does not
    /// derive is a node claiming to be about something other than what it is. Confirming
    /// that is the difference between a graph that records what a producer *said* and one
    /// that records what the daemon can still *confirm*, which is what "no trust-me
    /// responses" asks for. Its first half — that the held bytes hash to the commitment —
    /// is unreachable through the staging surface today, because
    /// [`DaemonState::stage`](super::state::DaemonState::stage) files content *under* the
    /// identity it derives. It is kept because the chain is only closed if every link is
    /// re-derived, and a link that cannot break today is not a link that can be dropped.
    fn check(
        &self,
        handle: &EvidenceHandle,
        node: &EvidenceNode,
        state: &DaemonState,
        services: &Services,
    ) -> Result<Checked, Fault> {
        if let Some(redaction) = &node.redaction {
            return Ok(Checked::Redacted(redaction.clone()));
        }

        // 1. The class of check this node's evidence would need, and which of the two lanes
        //    can supply it. Everything outside them needs a checker this process cannot
        //    reach, and saying so is not a limitation to hide: promoting to `validated` on
        //    the strength of a re-derived reference would be precisely the
        //    self-certification INV-004 forbids.
        let lane = match node.evidence_kind {
            Some(EvidenceKind::ProductionObservation | EvidenceKind::Example) => Lane::Observation,
            Some(EvidenceKind::Certificate) => Lane::Certificate,
            // `None` is unreachable here — `verify` refuses a node with no evidence class
            // before it calls this function — and is folded into the same arm rather than
            // given an `unreachable!`: a daemon does not abort on its own invariant.
            _ => {
                return Err(Fault::new(
                    ErrorCode::InsufficientEvidence,
                    "the independent checker this evidence class requires is not served by \
                     this daemon; no status is promoted on a check that did not run",
                ));
            }
        };

        // 2. The reference resolves. RFC 0038's whiteboard compiler "rejects nonexistent
        //    references"; this is that rule one layer down, where the graph is written.
        let staged = state
            .staged(&node.artifact)
            .ok_or_else(|| {
                Fault::new(
                    ErrorCode::InsufficientEvidence,
                    "the daemon holds nothing under the identity this node references",
                )
            })?
            .clone();

        // 3a. The held bytes re-derive to the identity the node names.
        let referent = super::state::DaemonState::commit_of(
            services.identifier(),
            &staged.path,
            &staged.content,
        )
        .map_err(|_| identity_rejected())?;
        if referent != node.artifact {
            return Err(identity_rejected());
        }

        // 3b. The node re-derives to the identity it is filed under. Computed through the
        //     same function `observe.ingest` names its appends with, so the check runs
        //     against the rule rather than against a second copy of it.
        let rederived =
            node_identity(services, &node.artifact, &node.tool).map_err(|_| identity_rejected())?;
        if &rederived != handle {
            return Err(identity_rejected());
        }

        match lane {
            Lane::Observation => Ok(Checked::Establishes(
                ClaimStatus::Observed,
                // The re-derivation was performed by the daemon's own identity kernel, not by
                // a solver whose verdict is taken on trust. `checked-certificate` is the
                // nearer of the two members the wire admits; the vocabulary has no member for
                // "reference re-derivation", and inventing one is a schema change, not a
                // string.
                ValidationBasis::CheckedCertificate,
                None,
            )),
            // 4. The bytes go to the trusted checking base exactly as they are held. The
            //    daemon reads none of them: not the magic, not a length, not an epoch.
            Lane::Certificate => certificate_check(&staged.content),
        }
    }
}

/// Which of the two checks `evidence.verify` can run this node needs.
///
/// A closed pair rather than a `bool`, because a third lane is a thing this daemon may gain
/// (`sampled` and `bounded` name producing engines; `proved` names Lean) and a boolean would
/// have to be rewritten rather than extended when it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lane {
    /// A captured execution, re-derived through the [`ContentIdentifier`] seam.
    ///
    /// [`ContentIdentifier`]: continuum_workspace::publication::ContentIdentifier
    Observation,
    /// A certificate an untrusted producer wrote, checked by the kernel that owns its
    /// family.
    Certificate,
}

// --- the certificate lane ------------------------------------------------------------------

/// Route one certificate-class artifact through `continuum-certificate`, and keep the four
/// outcomes it can answer with apart on the wire.
///
/// > A caller that wants a boolean has to write the collapse itself, where a reader can see
/// > it.
/// >
/// > — `continuum_certificate::verdict::Outcome`
///
/// This is that place, and there is no collapse in it. The four outcomes, and the four wire
/// answers they get:
///
/// | outcome | wire | why this one |
/// |---|---|---|
/// | `Checked(Verified)` | `ok`, promoted to `validated`, `verdict = established` | the kernel re-derived every obligation from the bytes |
/// | `Checked(Rejected)` | `error CertificateRejected` | "An independent checker rejected a certificate or proof artifact" — the code's own sentence, and this is the only place in the daemon that can raise it truthfully |
/// | `Checked(Unsupported)` | `ok`, nothing promoted, `verdict = inconclusive`, `inconclusive_reason = Unsupported` | the checker ran and answered; INV-008's typed reason has no channel on an error, because `verdict` is null there |
/// | `Unroutable` | `error EpochUnsupported` | "An artifact […] declares a schema or semantic epoch unknown or incompatible with this daemon. Typed rejection, never best-effort decoding (docs/09 T13)" |
///
/// The fourth row is the one worth defending, because it is the row a shortcut would ruin.
/// A byte string whose leading magic names no family could be a corrupted `CONTCERT`, a
/// family a later kernel will own, or a JPEG, and *nothing decoded it*: answering
/// `CertificateRejected` would call a possibly-valid artifact invalid, and answering
/// `UnsupportedSemanticFeature` — the code for a lane this deployment has not shipped —
/// would say the daemon lacks a checker it may well have. `EpochUnsupported` is the code for
/// an artifact that declares a contract this daemon cannot read, it is declared by
/// `evidence.verify`'s own `errors` clause, and docs/09 T13's control is *"no 'best effort'
/// decode for evidence"*, which is precisely why `continuum-certificate` has no decoder to
/// guess with.
///
/// The `Verified` row carries one more distinction that a composition must not flatten:
/// `continuum-kernel-smt`'s claim reports its own [`AssuranceClass`], because a refutation
/// leaning on unchecked theory lemmas is `TRUSTED_SOLVER` and not `CHECKED_CERTIFICATE`
/// (ADR-0017, docs/03 §6.2). The wire's `validation_basis` has exactly those two members, so
/// it is read from the claim rather than assumed — the one arm below that does not hard-code
/// `checked-certificate`.
///
/// [`AssuranceClass`]: continuum_kernel_smt::verdict::AssuranceClass
fn certificate_check(bytes: &[u8]) -> Result<Checked, Fault> {
    let verdict = match continuum_certificate::check_certificate(bytes) {
        Outcome::Unroutable(fault) => return Err(unroutable(fault)),
        Outcome::Checked(verdict) => verdict,
    };
    let family = verdict.family();
    // Twelve arms rather than a helper that reduces the four kernels to one three-valued
    // enum: the reduction would be a fifth vocabulary, and the crate that composes these
    // checkers declines to define one for the same reason ("there is no unified claim
    // accessor"). Written out, the match is total by the compiler's own reading, and a fifth
    // kernel or a fourth verdict arm is a compile error here rather than a silent default.
    let checked = |basis| {
        Ok(Checked::Establishes(
            ClaimStatus::Validated,
            basis,
            Some(family),
        ))
    };
    match &verdict {
        KernelVerdict::Core(verdict) => match verdict {
            continuum_kernel_core::Verdict::Verified(_) => {
                checked(ValidationBasis::CheckedCertificate)
            }
            continuum_kernel_core::Verdict::Rejected(rejection) => Err(certificate_rejected(
                family,
                rejection.reason(),
                rejection
                    .field()
                    .map(continuum_kernel_core::verdict::Field::as_str),
            )),
            continuum_kernel_core::Verdict::Unsupported(_) => Ok(Checked::Unsupported(family)),
        },
        KernelVerdict::Sat(verdict) => match verdict {
            continuum_kernel_sat::Verdict::Verified(_) => {
                checked(ValidationBasis::CheckedCertificate)
            }
            continuum_kernel_sat::Verdict::Rejected(rejection) => Err(certificate_rejected(
                family,
                rejection.reason(),
                rejection
                    .field()
                    .map(continuum_kernel_sat::verdict::Field::as_str),
            )),
            continuum_kernel_sat::Verdict::Unsupported(_) => Ok(Checked::Unsupported(family)),
        },
        KernelVerdict::Smt(verdict) => match verdict {
            continuum_kernel_smt::Verdict::Verified(claim) => checked(smt_basis(claim)),
            continuum_kernel_smt::Verdict::Rejected(rejection) => Err(certificate_rejected(
                family,
                rejection.reason(),
                rejection
                    .field()
                    .map(continuum_kernel_smt::verdict::Field::as_str),
            )),
            continuum_kernel_smt::Verdict::Unsupported(_) => Ok(Checked::Unsupported(family)),
        },
        KernelVerdict::Temporal(verdict) => match verdict {
            continuum_kernel_temporal::Verdict::Verified(_) => {
                checked(ValidationBasis::CheckedCertificate)
            }
            continuum_kernel_temporal::Verdict::Rejected(rejection) => Err(certificate_rejected(
                family,
                rejection.reason(),
                rejection
                    .field()
                    .map(continuum_kernel_temporal::verdict::Field::as_str),
            )),
            continuum_kernel_temporal::Verdict::Unsupported(_) => Ok(Checked::Unsupported(family)),
        },
    }
}

/// The wire's `validation_basis` for an SMT refutation, read from the kernel's own claim.
///
/// > `Validated` records whether solver evidence is `CHECKED_CERTIFICATE` or
/// > `TRUSTED_SOLVER`; the two never render identically.
/// >
/// > — plan §11.4
///
/// The kernel decides which it is — `CheckedCertificate` exactly when the refutation used no
/// theory lemma — and this function transcribes that answer into the wire's two-member
/// vocabulary. It does not consult the theories itself: a second rule for "was anything
/// trusted" is a second answer to the question ADR-0017 exists to keep single.
fn smt_basis(claim: &continuum_kernel_smt::verdict::CheckedClaim) -> ValidationBasis {
    match claim.assurance_class() {
        continuum_kernel_smt::verdict::AssuranceClass::CheckedCertificate => {
            ValidationBasis::CheckedCertificate
        }
        continuum_kernel_smt::verdict::AssuranceClass::TrustedSolver => {
            ValidationBasis::TrustedSolver
        }
    }
}

/// The `CertificateRejected` a routed kernel's own `Rejected` arm produces.
///
/// One detail per family, so the answer names the trusted checker that spoke rather than
/// leaving the caller to infer it. As of protocol 3.4 the kernel's *reason within*
/// `Rejected` travels too, on the typed channel built for exactly this: `Error.data`
/// carries the declared `CertificateRejection` shape (RFC 0026 F19, bn-3jrtz), whose
/// `reason` and `field` are the owning kernel's own stable diagnostic tokens
/// (`Rejection::reason()`, `Field::as_str()`), relayed verbatim. `detail` stays a
/// non-interpolated `&'static str` by `rule envelope.no_prose`, and no vocabulary is
/// transcribed here: the tokens are produced by the crate that owns them, which is what
/// keeps this daemon from becoming the second authority bn-dtg61 declined to create. The
/// rejection's numeric specifics — offsets, counts, indices — still do not travel;
/// recovering them is a matter of running that kernel over the same bytes, which any
/// holder of the artifact can do and this daemon's own tests do.
fn certificate_rejected(
    family: CertificateFamily,
    reason: &'static str,
    field: Option<&'static str>,
) -> Fault {
    Fault::new(
        ErrorCode::CertificateRejected,
        match family {
            CertificateFamily::Core => {
                "continuum-kernel-core rejected this certificate: the verdict is that \
                 kernel's own, over the bytes the daemon holds"
            }
            CertificateFamily::Sat => {
                "continuum-kernel-sat rejected this refutation: the verdict is that \
                 kernel's own, over the bytes the daemon holds"
            }
            CertificateFamily::Smt => {
                "continuum-kernel-smt rejected this refutation: the verdict is that \
                 kernel's own, over the bytes the daemon holds"
            }
            CertificateFamily::Temporal => {
                "continuum-kernel-temporal rejected this witness: the verdict is that \
                 kernel's own, over the bytes the daemon holds"
            }
        },
    )
    .with_data(ErrorData::CertificateRejection(CertificateRejection {
        checker: family.checker_crate().to_owned(),
        reason: reason.to_owned(),
        field: match field {
            Some(token) => Optional::Present(token.to_owned()),
            None => Optional::Absent,
        },
    }))
}

/// The `EpochUnsupported` a routing failure produces, one detail per [`RoutingFault`].
///
/// Two details under one code, because the two faults are one fact to a caller — no member
/// of the trusted checking base owns these bytes — while still saying which way the bytes
/// failed to name one. Neither is a rejection and neither is an unsupported feature: no
/// checker ran, so no checker is in a position to say why.
const fn unroutable(fault: RoutingFault) -> Fault {
    Fault::new(
        ErrorCode::EpochUnsupported,
        match fault {
            RoutingFault::TooShortForMagic { .. } => {
                "the artifact this node references is too short to carry a certificate \
                 family magic, so it declares no wire contract this daemon's trusted \
                 checking base implements"
            }
            RoutingFault::UnknownFamily { .. } => {
                "the artifact this node references declares a certificate family no checker \
                 in this build owns; nothing decoded it, so nothing is in a position to \
                 call it invalid either"
            }
        },
    )
}

/// The `proof_status` reason when the routed kernel declined the artifact's feature.
///
/// A static token per family rather than one shared token: "unsupported" is a fact about a
/// particular checker's implemented surface, and a client deciding whether to re-check later
/// needs to know whose surface it was.
const fn unsupported_feature(family: CertificateFamily) -> &'static str {
    match family {
        CertificateFamily::Core => "feature-unsupported-by-continuum-kernel-core",
        CertificateFamily::Sat => "feature-unsupported-by-continuum-kernel-sat",
        CertificateFamily::Smt => "feature-unsupported-by-continuum-kernel-smt",
        CertificateFamily::Temporal => "feature-unsupported-by-continuum-kernel-temporal",
    }
}

// --- evidence.link ----------------------------------------------------------------------

/// Record that an independent checker checked an evidence node (RFC 0038 D1–D3).
///
/// # Why this appends two artifacts and not one
///
/// RFC 0026 F14's decisive objection was that a `CHECKED_BY` edge "would have nothing to
/// point at". RFC 0038 D1 answered which kind it points at — a `receipt` — and an operation
/// that could only *reference* a receipt would have re-created the objection one level
/// down, because no verb creates one. So the receipt node arrives with the edge, and that is
/// the stronger design rather than the convenient one: a `receipt` reachable by any other
/// producer would be a forgeable check artifact, and here the node and the edge naming its
/// checker are appended together or not at all.
///
/// # The order of the refusals, and why it is this order
///
/// | # | Refusal | Code | Source |
/// |---|---|---|---|
/// | 1 | the caller is not a `service:` actor | `CapabilityDenied` | RFC 0038 "Authority": a checker is a service identity (RFC 0027 P3) |
/// | 2 | the graph does not hold the subject | `CapabilityDenied` | RFC 0027 X2 — never a distinguishable not-found |
/// | 3 | the checker is the subject's own producer | `InsufficientEvidence` | INV-004, no self-certification |
/// | 4 | the daemon holds no clock | `UnsupportedSemanticFeature` | INV-005: a provenance record names a time, and none is invented |
/// | 5 | the daemon holds no receipt content | `CapabilityDenied` | RFC 0038's "rejects nonexistent references", at the write layer |
/// | 6 | the receipt identity already names a non-`receipt` node | `InsufficientEvidence` | RFC 0038 D1, through `CheckTargetRule` |
/// | 7 | the edge would relate an artifact to itself | `InsufficientEvidence` | INV-004: an artifact that checks itself asserts nothing |
///
/// Three is checked before four, five, six, and seven deliberately: a service that is the
/// claim's own producer has established nothing whatever the receipt says, and a refusal
/// that depended on the receipt would be a refusal that leaked something about it. That is
/// the order `evidence.verify` uses, for the same reason.
///
/// The checker is **never** a request field. It is `call.grant.actor` — the identity T4
/// bound to the admitted capability — so a caller cannot name someone else as the checker,
/// exactly as `observe.ingest`'s producer cannot name someone else as the author of its
/// append. `ServiceIdentity::parse` is what enforces the scheme, and it is
/// `continuum-evidence`'s type rather than a local check, so the daemon and the graph agree
/// on what a checker is by construction.
fn link(
    call: &Call<'_>,
    request: &EvidenceLinkRequest,
    state: &mut DaemonState,
    services: &Services,
    store: &ReferenceStore,
) -> Result<Effect, Fault> {
    // 1. The checker is the admitted actor, and it must be a service. `ServiceIdentity`
    //    refuses `agent:`, `human:`, and `ci:`, so an agent capability cannot append a
    //    check edge at any authority level — the gate is the scheme, not the ladder.
    let checker = ServiceIdentity::parse(call.grant.actor.as_str())
        .map(CheckerBinding::new)
        .map_err(|_| Fault::denied())?;

    // 2. A subject the graph does not hold is a denial (RFC 0027 X2).
    let subject = state
        .evidence(&request.subject)
        .ok_or_else(Fault::denied)?
        .clone();

    // 3. INV-004 at the edge: the thing being checked cannot mint its own checked-by.
    if subject.producer.as_str() == checker.checker().as_str() {
        return Err(Fault::new(
            ErrorCode::InsufficientEvidence,
            "a checker may not record a check of its own production: the appending service \
             is this node's producer",
        ));
    }

    // 4. Time is an explicit effect (INV-005, ADR-0003); a provenance record names one.
    let created_at = services.now().cloned().ok_or_else(|| {
        Fault::new(
            ErrorCode::UnsupportedSemanticFeature,
            "this daemon holds no time reading, and a provenance record names a check time",
        )
    })?;

    // 5. The receipt's content must be held. Same rule as `observe.ingest`: a reference the
    //    daemon cannot resolve is a denial, not a node it invents.
    let staged = state
        .staged(&request.receipt)
        .ok_or_else(Fault::denied)?
        .clone();

    // 6. Both identities, through the one seam `rule evidence.edge_identity` names. The
    //    receipt node is derived by the same function `observe.ingest` names its appends
    //    with, so a check recorded twice converges rather than forking the graph.
    let receipt_handle = node_identity(services, &request.receipt, &request.checker_profile)
        .map_err(|_| identity_unavailable())?;
    if let Some(held) = state.evidence(&receipt_handle) {
        // The identity is a function of (content, profile) and nothing else, so it can
        // already name a node of another kind. RFC 0038 D1 says what a check edge may point
        // at, and the rule that decides is `continuum-evidence`'s, consulted rather than
        // restated.
        let kind = library_kind(held.kind);
        if kind != Some(CHECKED_BY_TARGET) || !CheckTargetRule::default().admits(CHECKED_BY_TARGET)
        {
            return Err(Fault::new(
                ErrorCode::InsufficientEvidence,
                "a CHECKED_BY edge names a receipt, and this identity already names an \
                 evidence node of another kind",
            ));
        }
    }
    let relation = EdgeRelation::CheckedBy(checker.clone());
    if receipt_handle == request.subject {
        return Err(Fault::new(
            ErrorCode::InsufficientEvidence,
            "a check edge relates two artifacts; the receipt and the subject are one \
             identity",
        ));
    }
    let edge_handle = edge_identity(services, &relation, &request.subject, &receipt_handle)
        .map_err(|_| identity_unavailable())?;

    // 7. Publish the receipt under the caller's own capability, so the store decides and
    //    audits the write against the identity the wire presented (ADR-0037).
    let token =
        identity::capability_to_store(&call.envelope.capability).map_err(|_| Fault::denied())?;
    store
        .publish(ArtifactClass::Evidence, staged.content.clone(), &token)
        .map_err(|refusal| match refusal {
            continuum_workspace::publication::PublishRefusal::CapabilityDenied(_) => {
                Fault::denied()
            }
            continuum_workspace::publication::PublishRefusal::Aborted(_) => Fault::new(
                ErrorCode::PublicationAborted,
                "the publication aborted; nothing was published and nothing was truncated",
            ),
        })?;

    let key = call
        .envelope
        .idempotency_key
        .value()
        .cloned()
        .unwrap_or_default();

    // 8. The receipt node. It lands at the lattice's bottom like every other append: this
    //    operation writes no status, mints no `Promotion`, and cannot reach the
    //    compare-and-set. An edge is evidence that a check ran; what it licenses is
    //    `evidence.verify`'s separate decision (RFC 0038 "Authority").
    let receipt_node = EvidenceNode {
        kind: EvidenceNodeKind::Receipt,
        evidence_kind: subject.evidence_kind,
        claim_id: subject.claim_id.clone(),
        artifact: request.receipt.clone(),
        producer: call.grant.actor.clone(),
        tool: request.checker_profile.clone(),
        created_at: created_at.clone(),
        inputs: vec![request.receipt.as_str().to_owned()],
        idempotency_key: key.clone(),
        // A receipt records that a check ran; it carries no producer marker.
        labels: Vec::new(),
        history: vec![StatusWrite {
            status: ClaimStatus::BOTTOM,
            service_identity: None,
            validation_basis: None,
            inconclusive_reason: None,
        }],
        redaction: None,
    };
    let (_, node_appended) = state.append_evidence(receipt_handle.clone(), receipt_node);
    if node_appended {
        state.record_evidence_event(crate::protocol::task::EvidenceEvent {
            at: created_at.clone(),
            kind: EvidenceEventKind::NodePublished,
            node: Optional::Present(receipt_handle.clone()),
            edge: Optional::Absent,
            status: Optional::Absent,
            claim_id: Optional::Present(subject.claim_id.clone()),
        });
    }

    // 9. The edge.
    let edge = EvidenceEdge {
        relation,
        from: request.subject.clone(),
        to: receipt_handle.clone(),
        producer: call.grant.actor.clone(),
        tool: request.checker_profile.clone(),
        created_at: created_at.clone(),
        inputs: vec![
            request.subject.as_str().to_owned(),
            receipt_handle.as_str().to_owned(),
        ],
        idempotency_key: key,
    };
    let (_, edge_appended) = state.append_edge(edge_handle.clone(), edge);
    if edge_appended {
        state.record_evidence_event(crate::protocol::task::EvidenceEvent {
            at: created_at,
            kind: EvidenceEventKind::EdgePublished,
            node: Optional::Absent,
            edge: Optional::Present(edge_handle.clone()),
            status: Optional::Absent,
            claim_id: Optional::Present(subject.claim_id.clone()),
        });
    }

    Ok(Effect::new(
        Payload::EvidenceLink(EvidenceLinkResponse {
            edge: edge_handle.clone(),
            receipt: receipt_handle.clone(),
            // The checker is the daemon-admitted actor, reported back so a client can see
            // what was recorded rather than what it asked for.
            checker: checker.checker().as_str().to_owned(),
        }),
        Nullable::Value(Verdict::Structural(
            crate::protocol::envelope::StructuralVerdictValue {
                outcome: StructuralOutcome::Created,
            },
        )),
    )
    .with_artifacts(vec![
        edge_artifact(&receipt_handle)?,
        edge_artifact(&edge_handle)?,
    ]))
}

/// One evidence edge as `schemas/evidence-graph-edge.schema.json` writes it.
///
/// `checker` is emitted exactly for `CHECKED_BY` — the schema's one `if`/`then` — and here
/// that is not a branch anyone had to remember: [`EdgeRelation::checker`] is [`Some`] for
/// that variant and [`None`] for the other twelve, by the shape of the type.
fn edge_record(handle: &EvidenceHandle, edge: &EvidenceEdge) -> Json {
    let mut fields: BTreeMap<String, Json> = BTreeMap::new();
    fields.insert(
        "schema_id".to_owned(),
        Json::String(EDGE_SCHEMA_ID.to_owned()),
    );
    fields.insert("schema_epoch".to_owned(), Json::Integer(EDGE_SCHEMA_EPOCH));
    fields.insert(
        "edge_id".to_owned(),
        Json::String(handle.as_str().to_owned()),
    );
    fields.insert(
        "kind".to_owned(),
        Json::String(edge.relation.kind().as_str().to_owned()),
    );
    fields.insert(
        "from".to_owned(),
        Json::String(edge.from.as_str().to_owned()),
    );
    fields.insert("to".to_owned(), Json::String(edge.to.as_str().to_owned()));
    if let Some(service) = edge.relation.checker() {
        fields.insert(
            "checker".to_owned(),
            Json::String(service.as_str().to_owned()),
        );
    }
    let mut provenance: BTreeMap<String, Json> = BTreeMap::new();
    provenance.insert(
        "actor".to_owned(),
        Json::String(edge.producer.as_str().to_owned()),
    );
    provenance.insert(
        "created_at".to_owned(),
        Json::String(edge.created_at.as_str().to_owned()),
    );
    provenance.insert(
        "inputs".to_owned(),
        Json::Array(
            edge.inputs
                .iter()
                .map(|input| Json::String(input.clone()))
                .collect(),
        ),
    );
    // Optional, and omitted when the producer named none — `node_record`'s rule, for the
    // same reason and over the same schema object.
    if !edge.tool.is_empty() {
        provenance.insert("tool".to_owned(), Json::String(edge.tool.clone()));
    }
    fields.insert("provenance".to_owned(), Json::Object(provenance));
    Json::Object(fields)
}

fn edge_artifact(handle: &EvidenceHandle) -> Result<crate::protocol::envelope::ArtifactRef, Fault> {
    Ok(crate::protocol::envelope::ArtifactRef {
        kind: ArtifactClass::Evidence.token().to_owned(),
        handle: crate::protocol::scalar::ArtifactHandle::new(handle.as_str()).map_err(|_| {
            Fault::new(
                ErrorCode::PublicationAborted,
                "the derived evidence identity is not a well-formed artifact handle",
            )
        })?,
        commitment: Optional::Present(crate::protocol::scalar::Commitment::new(handle.as_str())),
        redacted: Optional::Absent,
    })
}

/// The identity of an evidence edge: a function of what the edge *asserts*
/// (`rule evidence.edge_identity`, RFC 0038 D2).
///
/// Five length-framed parts through the same [`ContentIdentifier`] seam and the same
/// artifact class that name a node, so one identity kernel names every evidence artifact
/// and a deployment cannot end up with two. The parts are the domain tag, the edge kind,
/// both endpoint handles, and the checker — everything the edge says and nothing else.
///
/// `provenance` is deliberately outside it, which is the same decision RFC 0038 D4 takes
/// for a node and for the same reason: a checker's retry must converge on one edge, and an
/// identity carrying a clock cannot. Note this differs from `continuum-evidence`'s *edge*
/// content key, which keeps provenance inside so that two agents asserting one relation
/// keep two credits — that key is a within-graph key and never reaches the wire (RFC 0038
/// D4), and the wire's rule is convergence.
///
/// # Errors
///
/// [`ServiceError::Identity`](super::ServiceError::Identity) when the identity seam cannot
/// name the preimage, or when the derived token is not a well-formed `ev_` handle.
///
/// [`ContentIdentifier`]: continuum_workspace::publication::ContentIdentifier
pub fn edge_identity(
    services: &Services,
    relation: &EdgeRelation,
    from: &EvidenceHandle,
    to: &EvidenceHandle,
) -> Result<EvidenceHandle, super::ServiceError> {
    let checker = relation.checker().map_or("", ServiceIdentity::as_str);
    let mut preimage = Vec::new();
    for part in [
        EDGE_IDENTITY_DOMAIN.as_bytes(),
        relation.kind().as_str().as_bytes(),
        from.as_str().as_bytes(),
        to.as_str().as_bytes(),
        checker.as_bytes(),
    ] {
        preimage.extend_from_slice(&(part.len() as u64).to_be_bytes());
        preimage.extend_from_slice(part);
    }
    let stored = services
        .identifier()
        .identify(ArtifactClass::Evidence, &preimage)
        .map_err(|_| super::ServiceError::Identity)?;
    EvidenceHandle::new(&stored.to_string()).map_err(|_| super::ServiceError::Handle)
}

/// The `continuum-evidence` node kind a wire node kind names.
///
/// Both vocabularies transcribe `evidence-graph-node.schema.json`'s `kind` enum, token for
/// token, so the bridge is the token itself rather than a second twenty-row table that
/// could disagree with either side.
#[must_use]
pub fn library_kind(kind: EvidenceNodeKind) -> Option<NodeKind> {
    NodeKind::from_token(kind.as_wire())
}

/// The wire node kind a `continuum-evidence` node kind names — [`library_kind`]'s inverse.
///
/// Both vocabularies transcribe `evidence-graph-node.schema.json`'s `kind` enum token for
/// token, so the bridge is the token itself rather than a second twenty-row table that
/// could disagree with either side. [`None`] is unreachable while the two agree, and is
/// returned rather than panicked on for the reason the dispatcher returns a typed refusal
/// on an unroutable shape: a daemon does not abort on its own invariant.
#[must_use]
pub fn wire_node_kind(kind: NodeKind) -> Option<EvidenceNodeKind> {
    EvidenceNodeKind::ALL
        .iter()
        .copied()
        .find(|member| member.as_wire() == kind.as_str())
}

/// The wire edge kind a `continuum-evidence` edge kind names, by the same bridge.
#[must_use]
pub fn wire_edge_kind(kind: continuum_evidence::edge::EdgeKind) -> EvidenceEdgeKind {
    EvidenceEdgeKind::ALL
        .iter()
        .copied()
        .find(|member| member.as_wire() == kind.as_str())
        .unwrap_or(EvidenceEdgeKind::CheckedBy)
}

/// The one answer a failed identity derivation gets on the append path.
fn identity_unavailable() -> Fault {
    Fault::new(
        ErrorCode::PublicationAborted,
        "no well-formed content identity could be derived for the evidence artifact",
    )
}

/// The identity of an evidence node: a function of what the node is *about*.
///
/// The preimage is the referenced commitment and the instrumentation profile, each framed
/// by its length so no pair of inputs can produce another pair's preimage by concatenation.
/// Two captures of one trace under two profiles are two observations, so the profile is in
/// the preimage; two ingests of one trace under one profile are one observation, which is
/// what makes a re-ingest converge on the original node identity (RFC 0038: "a replayed
/// write returns the original node identity").
///
/// Called from exactly two places — [`observe::ingest`](super::observe) when a node is
/// appended, and this module's own check when it is verified — so the check runs against
/// the rule rather than against a second spelling of it. A second copy would let a node and
/// its verification agree while both disagreed with the protocol.
///
/// # Errors
///
/// [`ServiceError::Identity`](super::ServiceError::Identity) when the identity seam cannot
/// name the preimage, or when the derived token is not a well-formed `ev_` handle.
pub fn node_identity(
    services: &Services,
    artifact: &crate::protocol::scalar::Commitment,
    tool: &str,
) -> Result<EvidenceHandle, super::ServiceError> {
    let mut preimage = Vec::new();
    for part in [artifact.as_str().as_bytes(), tool.as_bytes()] {
        preimage.extend_from_slice(&(part.len() as u64).to_be_bytes());
        preimage.extend_from_slice(part);
    }
    let stored = services
        .identifier()
        .identify(ArtifactClass::Evidence, &preimage)
        .map_err(|_| super::ServiceError::Identity)?;
    EvidenceHandle::new(&stored.to_string()).map_err(|_| super::ServiceError::Handle)
}

/// The one answer every failed re-derivation gets.
///
/// One constant rather than three, because the three disagreements — unnameable content,
/// a referent whose bytes do not hash to the identity claimed for them, and a node filed
/// under an identity it does not derive — are one fact to the caller: the daemon could not
/// confirm what the node says about itself. Distinguishing them would say which link of the
/// chain broke, which is a detail about the *store's* interior.
fn identity_rejected() -> Fault {
    Fault::new(
        ErrorCode::CertificateRejected,
        "an independent re-derivation disagrees with the identity this node claims",
    )
}

/// The two ways a compare-and-set can fail, both reported as the one code the IDL declares.
///
/// > An evidence-graph status promotion lost its compare-and-set against the claim's
/// > current status (plan §11.7, RFC 0038). Re-read and retry; the lattice never regresses.
/// >
/// > — `ErrorCode::StatusConflict`
///
/// The sentence covers both halves — a lost guard and a would-be regression — so both map
/// here, and the `detail` does not distinguish them because the recovery is identical.
fn status_fault(rejected: PromotionRejected) -> Fault {
    match rejected {
        PromotionRejected::Conflict(_) | PromotionRejected::Regression(_) => Fault::new(
            ErrorCode::StatusConflict,
            "the promotion lost its compare-and-set against the claim's current status",
        ),
    }
}

fn semantic(
    verdict: SemanticVerdict,
    reason: Option<InconclusiveReason>,
    class: AssuranceClass,
) -> Nullable<Verdict> {
    Nullable::Value(Verdict::Semantic(SemanticVerdictValue {
        verdict,
        // "REQUIRED when `verdict = inconclusive` (INV-008)", and absent otherwise.
        inconclusive_reason: match reason {
            Some(reason) => Optional::Present(reason),
            None => Optional::Absent,
        },
        assurance_class: class,
    }))
}

/// The assurance class a settled status is supported at.
///
/// `AssuranceClass` has five members and the lattice has nine, so the four with no class —
/// `proposed`, `refuted`, `inconclusive`, `superseded` — report `observed`, the weakest
/// class, rather than a stronger one they do not hold.
const fn assurance_class(status: ClaimStatus) -> AssuranceClass {
    match status {
        ClaimStatus::Sampled => AssuranceClass::Sampled,
        ClaimStatus::Bounded => AssuranceClass::Bounded,
        ClaimStatus::Validated => AssuranceClass::Validated,
        ClaimStatus::Proved => AssuranceClass::Proved,
        _ => AssuranceClass::Observed,
    }
}

/// The nine dimensions of an `evidence.verify` answer over content the daemon could read.
///
/// Two of them move and seven never do. `values` is produced in every answer this builder
/// serves, because the reference re-derivation is the one check that runs on both lanes and
/// has already completed by the time an envelope exists; `proof_status` is the certificate
/// lane's dimension and is passed in. The other seven carry a typed `Unsupported(reason)`,
/// which is the honest shape: "every dimension MUST name a producing engine or carry a typed
/// `Unsupported(reason)` […] Hiding uncertainty to save tokens is prohibited"
/// (`rule envelope.assurance_required`).
fn verify_envelope(
    service: &str,
    values: &'static str,
    proof_status: EnvelopeDimension,
) -> AssuranceEnvelope {
    let produced = EnvelopeDimension::Produced(ProducedDimension {
        engine: service.to_owned(),
        summary: values.to_owned(),
    });
    AssuranceEnvelope {
        bounds: unsupported_dimension("no-exploration-engine"),
        faults: unsupported_dimension("no-fault-model"),
        fairness: unsupported_dimension("no-fairness-obligation"),
        // The dimension every answer here establishes: the values the node references are
        // exactly the ones the daemon re-derived.
        values: produced,
        schedules: unsupported_dimension("no-schedule-exploration"),
        memory_model: unsupported_dimension("sequential-consistency-only"),
        // Reconciled at bn-1y4qc, and the token is unchanged because the reading behind it
        // was always the narrow one. A dimension is a statement about *the answer that
        // carries it*, never about the daemon: this envelope describes an `evidence.verify`
        // re-derivation, which projects no observer and has no observer set to project over
        // — the node it re-derives names a commitment and an instrumentation profile, not an
        // intent. Since `context.compile` landed, this daemon *does* run RFC 0028's stage 6
        // over a registered observer projection, and the pack it publishes names
        // `continuum-context::observer` in its own envelope's `observer` dimension
        // (`daemon::context::observer_dimension`) — with this same token on a compile that
        // configured no projection. One token, two answers, and neither of them the claim
        // "this daemon has no observer projection".
        observer: unsupported_dimension("no-observer-projection"),
        proof_status,
        unknowns: unsupported_dimension("not-enumerated"),
    }
}

/// One dimension nothing established, with the typed reason it did not.
fn unsupported_dimension(reason: &str) -> EnvelopeDimension {
    EnvelopeDimension::Unsupported(UnsupportedDimension {
        reason: reason.to_owned(),
    })
}

/// What `proof_status` reads when a check succeeded.
///
/// The observation lane discharges no proof obligation and says so. The certificate lane
/// does discharge one, and the engine that discharged it is **the kernel, not this service**
/// — naming `service:continuumd-verifier` there would credit the daemon with a check it
/// routed rather than ran, which is the one thing INV-004 is about.
fn checked_proof_status(checker: Option<CertificateFamily>) -> EnvelopeDimension {
    match checker {
        None => unsupported_dimension("no-proof-obligation-discharged"),
        Some(family) => EnvelopeDimension::Produced(ProducedDimension {
            engine: family.checker_crate().to_owned(),
            summary: "certificate re-checked from its wire form by the kernel that owns its \
                      family"
                .to_owned(),
        }),
    }
}

/// The nine dimensions over a redacted input: none of them, typed.
///
/// > any claim that required the hidden data MUST downgrade in the `assurance` envelope per
/// > plan §18.4.
/// >
/// > — RFC 0026, "Redacted values"
///
/// A downgrade from "one dimension produced" is "no dimension produced", and every reason
/// says why in one word the client can branch on.
fn withheld_envelope() -> AssuranceEnvelope {
    let withheld = || unsupported_dimension("referenced-content-redacted");
    AssuranceEnvelope {
        bounds: withheld(),
        faults: withheld(),
        fairness: withheld(),
        values: withheld(),
        schedules: withheld(),
        memory_model: withheld(),
        observer: withheld(),
        proof_status: withheld(),
        unknowns: withheld(),
    }
}

// --- the two spellings of one lattice ----------------------------------------------------

/// The wire status a lattice status is.
///
/// The two enums are the same nine members in the same order — the IDL's `EvidenceStatus`
/// and `continuum-evidence`'s `ClaimStatus` both transcribe plan §11.4 — and
/// `tests/daemon_evidence.rs` asserts the token-for-token agreement rather than trusting
/// this function. The *order* is not shared: the wire enum is a vocabulary with no meaning
/// attached, and the lattice's chain lives in the crate that owns it.
pub const fn wire_status(status: ClaimStatus) -> EvidenceStatus {
    match status {
        ClaimStatus::Proposed => EvidenceStatus::Proposed,
        ClaimStatus::Observed => EvidenceStatus::Observed,
        ClaimStatus::Sampled => EvidenceStatus::Sampled,
        ClaimStatus::Bounded => EvidenceStatus::Bounded,
        ClaimStatus::Validated => EvidenceStatus::Validated,
        ClaimStatus::Proved => EvidenceStatus::Proved,
        ClaimStatus::Refuted => EvidenceStatus::Refuted,
        ClaimStatus::Inconclusive => EvidenceStatus::Inconclusive,
        ClaimStatus::Superseded => EvidenceStatus::Superseded,
    }
}

/// The lattice status a wire status is.
#[must_use]
pub const fn lattice_status(status: EvidenceStatus) -> ClaimStatus {
    match status {
        EvidenceStatus::Proposed => ClaimStatus::Proposed,
        EvidenceStatus::Observed => ClaimStatus::Observed,
        EvidenceStatus::Sampled => ClaimStatus::Sampled,
        EvidenceStatus::Bounded => ClaimStatus::Bounded,
        EvidenceStatus::Validated => ClaimStatus::Validated,
        EvidenceStatus::Proved => ClaimStatus::Proved,
        EvidenceStatus::Refuted => ClaimStatus::Refuted,
        EvidenceStatus::Inconclusive => ClaimStatus::Inconclusive,
        EvidenceStatus::Superseded => ClaimStatus::Superseded,
    }
}

/// Whether advancing from `from` to `to` is a promotion, a reassertion, or a regression.
///
/// Re-exported through this module so a caller reasoning about the daemon's write model
/// reads one name for it; the decision is `continuum-evidence`'s.
#[must_use]
pub const fn transition(from: ClaimStatus, to: ClaimStatus) -> StatusTransition {
    from.transition_to(to)
}

/// The schema token for a node kind.
///
/// The wire enum's own tokens are the schema's, so this reads them off the vocabulary
/// rather than restating twenty strings.
fn node_kind_token(kind: EvidenceNodeKind) -> &'static str {
    kind.as_wire()
}

/// The store handle a wire evidence handle names.
///
/// # Errors
///
/// [`HandleMismatch`](super::identity::HandleMismatch) when the identity half is empty or
/// carries a character outside `[A-Za-z0-9_-]`.
pub fn evidence_to_store(
    handle: &EvidenceHandle,
) -> Result<continuum_workspace::artifact_path::ArtifactHandle, identity::HandleMismatch> {
    let identity = handle
        .as_str()
        .strip_prefix(ArtifactClass::Evidence.prefix())
        .ok_or(identity::HandleMismatch {
            class: ArtifactClass::Evidence,
        })?;
    continuum_workspace::artifact_path::ArtifactHandle::new(ArtifactClass::Evidence, identity)
        .map_err(|_| identity::HandleMismatch {
            class: ArtifactClass::Evidence,
        })
}
