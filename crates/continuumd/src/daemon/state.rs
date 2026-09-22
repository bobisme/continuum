//! Everything the daemon knows, as one explicit value.
//!
//! # Why the state is a value and not an ambient
//!
//! > The core draws no randomness; a seed arrives as an explicit capability (INV-005,
//! > ADR-0003).
//!
//! The same rule applies to every other ambient a daemon is normally built out of. There is
//! no clock here, no filesystem, no socket, no task runtime and no `async` anywhere in this
//! module tree: a time reading arrives as [`Services::now`](super::Services::now) if the
//! deployment supplies one, content arrives through [`DaemonState::stage`], and capability
//! administration arrives through [`DaemonState::register_capability`] — the out-of-band
//! surface RFC 0026 deliberately keeps outside the operation registry:
//!
//! > Minting, scoping, delegating, and revoking capabilities is a separate administrative
//! > surface (plan §4.5) that is out of scope for this IDL version.
//! >
//! > — `schemas/continuumd-native-protocol.idl`, §7
//!
//! Consequently a dispatch is a pure function of (request, state) and two identical
//! requests against equal states produce equal results, which is `rule
//! ordering.deterministic` stated as a property of the type rather than as a discipline.
//!
//! # Collections
//!
//! Every map is a [`BTreeMap`]: iteration order is a function of the keys present and of
//! nothing else, which is what `rule ordering.deterministic` needs from the container layer
//! (docs/19 §7).

use std::collections::BTreeMap;

use continuum_evidence::claim_status::{
    ClaimStatus, PromotionRejected, StatusConflict, compare_and_set,
};
use continuum_evidence::edge::EdgeRelation;
use continuum_intent::contract::IntentContract;
use continuum_value::assurance::ValidationBasis;
use continuum_workspace::components::WorkspaceDescriptor;
use continuum_workspace::lineage::{Fork, ForkName};
use continuum_workspace::snapshot::WorkspacePath;

use super::family::Arguments;
use super::{OperationOutcome, ServiceError};
use crate::protocol::envelope::{Budget, OutputPolicy, Page, Redacted, RequestEnvelope};
use crate::protocol::handshake::CapabilityDescriptor;
use crate::protocol::scalar::{
    ActorId, ArtifactHandle, CapabilityHandle, Commitment, ContextHandle, EvidenceHandle,
    IntentHandle, Timestamp, WorkspaceHandle,
};
use crate::protocol::shared::SnapshotComponents;
use crate::protocol::spec::{Nullable, Optional};
use crate::protocol::task::EvidenceEvent;
use crate::protocol::vocabulary::{
    AuthorityLevel, EvidenceKind, EvidenceNodeKind, InconclusiveReason,
};

/// A registered capability: what it confers, and the capability it was delegated from.
///
/// The parent edge is what makes RFC 0027's D6/D7 a computation rather than a review
/// obligation — "a child's admission set MUST be a subset of its parent's" is checked by
/// walking this chain (see [`admission`](super::admission)). A capability with no parent is
/// a root minted out of band.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityGrant {
    /// What the capability confers, in the 3.1 wire form (level, scope, expiry, delegation
    /// depth, and the narrowing profile).
    pub descriptor: CapabilityDescriptor,
    /// The capability this one was delegated from, when it was delegated.
    pub parent: Option<CapabilityHandle>,
}

/// One admission decision, recorded whether it permitted or denied.
///
/// > every privileged call is audited with actor, capability, inputs, policy decision,
/// > outputs, and evidence identity, and so is every *denial* […] A block that leaves no
/// > audit record is indistinguishable from an attack that was never tried.
/// >
/// > — RFC 0027 P5
///
/// The capability is recorded as its handle, whose [`Debug`] elides the token: a `cap_*`
/// "never appears in a trace, an error, a `next_operations` argument, or a rendering"
/// (RFC 0027 S5), and an audit record is a trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionRecord {
    /// The wire operation name the request carried.
    pub operation: String,
    /// The actor the request claimed.
    pub actor: String,
    /// The capability presented.
    pub capability: CapabilityHandle,
    /// Whether the request was admitted.
    pub admitted: bool,
    /// The audit-correlation identity the result cites, so the record and the result name
    /// each other.
    pub audit: String,
}

/// Registry status of an Intent Contract
/// (`schemas/intent-registry-record.schema.json`, `status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryStatus {
    /// A proposal. It has no INV-001 protection until `intent.accept`.
    Proposed,
    /// Accepted into the registry, and the lineage head.
    Accepted,
    /// Superseded by a successor contract (RFC 0037 R6, ID3).
    Superseded,
}

impl RegistryStatus {
    /// The schema's `status` token.
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Accepted => "accepted",
            Self::Superseded => "superseded",
        }
    }
}

/// The acceptance record `intent.accept` wrote
/// (`intent-registry-record.schema.json`, `acceptance`).
///
/// `capability` is not a field: the schema fixes it to the constant `"revise-intent"`, and
/// storing a constant is how a constant comes to have two values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Acceptance {
    /// `accepted_by` — the principal that accepted.
    pub accepted_by: String,
    /// `signature` — the acceptance signature, verbatim as supplied.
    pub signature: String,
    /// `timestamp` — the caller-supplied acceptance time. There is no clock here.
    pub timestamp: String,
    /// `audit_record` — the audit-correlation identity of the accepting call. Written by
    /// the daemon, never taken from the caller: plan §5.4 makes this the record *the
    /// daemon* produced.
    pub audit_record: String,
}

/// One Intent Contract as the registry holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentRecord {
    /// The contract itself, immutable by construction (`continuum-intent` has no
    /// `&mut self` method anywhere).
    pub contract: IntentContract,
    /// Protection status.
    pub status: RegistryStatus,
    /// The contract this one supersedes; the `SUPERSEDES` edge M1 walks.
    pub supersedes: Option<IntentHandle>,
    /// The contract that superseded this one; `None` while this record is the head.
    pub superseded_by: Option<IntentHandle>,
    /// The acceptance record, present exactly when `status` is `accepted`.
    pub acceptance: Option<Acceptance>,
}

/// One workspace as the daemon holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRecord {
    /// The file tree beside the components it was created with.
    pub descriptor: WorkspaceDescriptor,
    /// The governing Intent Contract, by identity only — never the contract itself
    /// (plan §4.2, INV-001).
    pub intent: IntentHandle,
    /// The lineage this workspace's source tree belongs to. `workspace.fork` advances it
    /// and `workspace.seal` checks against it, which is where `StaleSnapshot` comes from.
    pub lineage: ForkName,
    /// Whether the workspace has been sealed — published as immutable records.
    pub sealed: bool,
}

/// One staged file: the content a `Commitment` in a `SnapshotComponents` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedFile {
    /// Where in the workspace the content sits.
    pub path: WorkspacePath,
    /// The content.
    pub content: Vec<u8>,
}

/// One write to a claim's status, as the graph records it.
///
/// A status is never *edited*: every write appends one of these to
/// [`EvidenceNode::history`], and the claim's current status is the last one. That is plan
/// §11.7's "the evidence graph is append-only; nothing is edited in place" expressed as the
/// only shape this type has — there is no field to overwrite and no method that removes an
/// element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusWrite {
    /// The status after this write.
    pub status: ClaimStatus,
    /// The daemon service identity that performed the promotion, for the statuses whose
    /// schema conditional requires one.
    ///
    /// > Assurance-bearing status promotion is service-attributed: `sampled`, `bounded`,
    /// > `validated`, and `proved` statuses name the daemon service identity that performed
    /// > the promotion (plan §11.7 write model).
    /// >
    /// > — `schemas/evidence-graph-node.schema.json`
    ///
    /// [`None`] on the producer's own append, which is exactly the point: an entry a
    /// producer wrote names no service, and a status that requires one therefore cannot be
    /// reached by an append (INV-004).
    pub service_identity: Option<String>,
    /// `checked-certificate` or `trusted-solver`; required by the node schema when the
    /// status is `validated`.
    pub validation_basis: Option<ValidationBasis>,
    /// The typed INV-008 reason, required by the node schema when the status is
    /// `inconclusive`.
    pub inconclusive_reason: Option<InconclusiveReason>,
}

/// One evidence-graph node as the daemon holds it.
///
/// Every field but [`history`](EvidenceNode::history) is written once, at append, and the
/// state exposes no way to change any of them: [`DaemonState::append_evidence`] refuses to
/// replace an existing entry and there is no `evidence_mut`, no remove, and no clear. The
/// append-only write model is therefore a property of the surface rather than of a
/// convention a handler could forget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceNode {
    /// `kind` — the plan §11.2 node type.
    pub kind: EvidenceNodeKind,
    /// The assurance-result evidence class this node offers toward its claim, when it
    /// offers one.
    ///
    /// [`None`] is not "unknown": it is the honest reading for a node that offers no
    /// evidence at all. Every member of [`EvidenceKind`] is *a class of evidence
    /// offered toward a claim* (RFC 0010's thirteen), and a node a whiteboard note
    /// proposed offers none of them — it sits at `proposed` and asserts nothing, which
    /// is the whole point of RFC 0038 W5. Naming a class anyway would be the
    /// overstatement `evidence.verify` exists to refuse, so the field is absent and
    /// that operation answers `InsufficientEvidence` rather than routing a lane. The
    /// vocabulary's want of a fourteenth member is recorded as RFC 0026 F21, not
    /// papered over here: it is closed in a rank-1 schema and in
    /// `crates/continuum-value/src/assurance.rs` together, so widening it is not this
    /// slice's act.
    pub evidence_kind: Option<EvidenceKind>,
    /// `claim_id` — "Claim identity this node's status promotion is linearized against".
    pub claim_id: String,
    /// `artifact` — the artifact this node is *about*, by identity.
    pub artifact: Commitment,
    /// `provenance.actor` — the identity that appended this node, taken from the admitted
    /// capability rather than from the request body. A producer cannot name someone else as
    /// the author of its own append.
    pub producer: ActorId,
    /// `provenance.tool`.
    pub tool: String,
    /// `provenance.created_at`.
    pub created_at: Timestamp,
    /// `provenance.inputs`.
    pub inputs: Vec<String>,
    /// `idempotency_key` — "Idempotency key making agent retries safe (plan §11.7)".
    pub idempotency_key: String,
    /// `labels` — the node schema's own member for a producer's typed markers.
    ///
    /// Empty for a producer's append, which has nothing to mark. `whiteboard.compile`
    /// writes one, `whiteboard:<section key>` — `continuum-evidence`'s own spelling of
    /// which plan §11.5 section proposed the node — so a note's derivation is recoverable
    /// from the node itself and not only from the note the author holds.
    pub labels: Vec<String>,
    /// Every status this claim has held, in write order. Never empty: the append itself is
    /// the first element.
    pub history: Vec<StatusWrite>,
    /// Present when the content this node references has stopped being readable
    /// (summarized, purged, or lost — plan §4.5).
    pub redaction: Option<Redacted>,
}

impl EvidenceNode {
    /// The claim's current status: the last one written.
    ///
    /// # Panics
    ///
    /// Never: `history` is non-empty by construction — [`DaemonState::append_evidence`] is
    /// the only constructor path and it writes the appended status first.
    #[must_use]
    pub fn status(&self) -> ClaimStatus {
        self.history
            .last()
            .map_or(ClaimStatus::BOTTOM, |write| write.status)
    }

    /// The statuses this claim has held, in write order — the input
    /// [`verify_promotion_history`](continuum_evidence::claim_status::verify_promotion_history)
    /// checks.
    #[must_use]
    pub fn status_history(&self) -> Vec<ClaimStatus> {
        self.history.iter().map(|write| write.status).collect()
    }

    /// The service identity that performed the last status write, if any.
    #[must_use]
    pub fn service_identity(&self) -> Option<&str> {
        self.history
            .last()
            .and_then(|write| write.service_identity.as_deref())
    }
}

/// One evidence-graph edge as the daemon holds it.
///
/// The relation is [`EdgeRelation`] — `continuum-evidence`'s closed thirteen-kind
/// vocabulary — rather than a local enum or a string, so the schema's one conditional
/// arrives with the type: `EdgeRelation::CheckedBy` *carries* its checker, there is no
/// `Option` on the path to it, and a check edge with no checker is a value this daemon
/// cannot construct. That is `evidence-graph-edge.schema.json`'s `if`/`then` and plan §2
/// SD-11's "`evidence-graph-edge` requires `checker` on `CHECKED_BY`" (INV-004) as a
/// property of what compiles.
///
/// Only `CHECKED_BY` edges exist here today, because `evidence.link` is the only operation
/// that appends one; the field is the whole vocabulary anyway, so the next verb needs no
/// new type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceEdge {
    /// `kind`, and — for `CHECKED_BY` — the `checker` the schema requires with it.
    pub relation: EdgeRelation,
    /// `from` — the checked subject.
    pub from: EvidenceHandle,
    /// `to` — the receipt recording the check (RFC 0038 D1).
    pub to: EvidenceHandle,
    /// `provenance.actor` — the identity that appended the edge, from the admitted
    /// capability rather than from the request.
    pub producer: ActorId,
    /// `provenance.tool`.
    pub tool: String,
    /// `provenance.created_at`.
    pub created_at: Timestamp,
    /// `provenance.inputs`.
    pub inputs: Vec<String>,
    /// The key the ledger filed this append under.
    pub idempotency_key: String,
}

/// A replayed mutation: the request it was produced for, and the result it produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    /// The operation name, the arguments, and the two envelope handles the request pinned.
    /// `rule idempotency.replay` compares "a byte-identical canonical request"; this layer
    /// has no canonical bytes to compare (the codec is the transport half's, and the IDL
    /// fixes neither canonical field order nor union tagging), so the comparison is over
    /// the typed request instead. The two agree by construction once a codec exists: equal
    /// canonical bytes decode to equal typed requests.
    pub request: ReplayKey,
    /// The result the first execution produced. A replay returns it with the payload and
    /// every identity-bearing field unchanged, so "the same task or artifact identity" is
    /// returned by construction. Only the per-attempt `request_id` echo and the `audit`
    /// correlation derived from it are re-addressed to the retry (see `Daemon::dispatch`,
    /// step 7).
    pub outcome: OperationOutcome,
}

/// The typed stand-in for `rule idempotency.replay`'s canonical request bytes.
///
/// # What the canonical request is, field by field
///
/// > A mutation replayed with the same `idempotency_key` and a byte-identical canonical
/// > request MUST return the same task or artifact identity. The same key with a different
/// > request MUST be rejected with `IdempotencyKeyReused`.
/// >
/// > — `rule idempotency.replay`
///
/// The rule names the *request*, so this type has to be the whole of it and nothing besides.
/// [`RequestEnvelope`]'s thirteen fields, audited one at a time (bn-h1zqz, bn-1kp6's
/// DEFECT 3) — the five carried here, and the eight that are deliberately not:
///
/// | field | in the key? | why |
/// |---|---|---|
/// | `operation` | **yes** | two operations are two requests, whatever else agrees |
/// | `snapshot` | **yes** | the artifact the request is *about* |
/// | `intent` | **yes** | likewise |
/// | `arguments` | **yes** | the operation's own request struct, decoded |
/// | `budget` | **yes** | see below — it is load-bearing, not decoration |
/// | `output_policy` | **yes** | it bounds what the daemon returns, so two policies are two answers to compare |
/// | `page` | **yes** | a page request selects *which* elements come back; one key must not answer two pages |
/// | `request_id` | no | the per-*attempt* identity. A retry carries a fresh one by construction, so including it would make every genuine replay a conflict — it is what the rule exists to let differ |
/// | `idempotency_key` | no | the key the ledger files this under, not part of what is compared under it |
/// | `actor` | no | already the other half of the ledger's map key ("keys are scoped per actor"), so it cannot differ between a record and a hit |
/// | `capability` | no | RFC 0027 S5 keeps a `cap_*` out of every derived value, and the actor it is bound to (T4) is already the scope. Two delegations of one actor's authority are one caller retrying, not two requests |
/// | `protocol_version` | no | a request naming a version other than the negotiated one is refused at step 2 of the dispatch, so every request that reaches the ledger carries the same value and it can only add a constant |
/// | `trace` | no | W3C propagation metadata, per-attempt like `request_id`: a retry carries a new `traceparent`, and a replay that conflicted on it would be unusable by any traced client |
///
/// # Why `budget`, `output_policy`, and `page` are here
///
/// They were omitted until bn-h1zqz, and the omission was reachable: `budget` is
/// *semantically load-bearing* as well as declared — `verification::start_handle` writes
/// `budget_preimage` into the `task_*` content identity, so **two budgets are two tasks** —
/// and a `verification.start` replayed under one key with a larger budget was neither
/// refused with `IdempotencyKeyReused` nor honoured. It returned the first budget's task
/// verbatim, so a caller that asked for a 64-state campaign was answered with a 4-state one
/// and could not tell. `output_policy` and `page` are the same shape of omission: each is a
/// declared field of the request that changes the answer, so a key that ignores it answers
/// two different questions with one recorded reply.
///
/// The direction of the repair is the rule's own: a *different* canonical request under a
/// used key is refused, never silently served. A true replay — every field above equal — is
/// unaffected and still returns the recorded outcome, re-addressed only in its per-attempt
/// `request_id` and `audit` fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayKey {
    /// `RequestEnvelope.operation`.
    pub operation: String,
    /// `RequestEnvelope.snapshot`.
    pub snapshot: Nullable<WorkspaceHandle>,
    /// `RequestEnvelope.intent`.
    pub intent: Nullable<IntentHandle>,
    /// The operation's decoded request struct.
    pub arguments: Arguments,
    /// `RequestEnvelope.budget` — REQUIRED for `@task_starting` operations, and in the
    /// `task_*` preimage, so two budgets are two tasks.
    pub budget: Optional<Budget>,
    /// `RequestEnvelope.output_policy` — the bound on what the daemon returns.
    pub output_policy: Optional<OutputPolicy>,
    /// `RequestEnvelope.page` — which elements a `@paginated` answer carries.
    pub page: Optional<Page>,
}

impl ReplayKey {
    /// The key a request presents.
    #[must_use]
    pub fn of(envelope: &RequestEnvelope, arguments: &Arguments) -> Self {
        Self {
            operation: envelope.operation.as_str().to_owned(),
            snapshot: envelope.snapshot.clone(),
            intent: envelope.intent.clone(),
            arguments: arguments.clone(),
            budget: envelope.budget.clone(),
            output_policy: envelope.output_policy.clone(),
            page: envelope.page.clone(),
        }
    }
}

/// Everything the daemon knows.
#[derive(Debug, Default)]
pub struct DaemonState {
    capabilities: BTreeMap<CapabilityHandle, CapabilityGrant>,
    workspaces: BTreeMap<WorkspaceHandle, WorkspaceRecord>,
    lineages: BTreeMap<ForkName, Fork>,
    content: BTreeMap<Commitment, StagedFile>,
    components: BTreeMap<Commitment, SnapshotComponents>,
    intents: BTreeMap<IntentHandle, IntentRecord>,
    evidence: BTreeMap<EvidenceHandle, EvidenceNode>,
    edges: BTreeMap<EvidenceHandle, EvidenceEdge>,
    evidence_events: Vec<EvidenceEvent>,
    idempotency: BTreeMap<(String, String), Replay>,
    admissions: Vec<AdmissionRecord>,
    tasks: super::task::TaskTable,
    regions: super::region::TaskRegions,
    models: super::verification::ModelCatalog,
    context_packs: BTreeMap<ContextHandle, super::context::ContextPackRecord>,
    compile_sources: BTreeMap<String, super::context::ContextCompileSource>,
}

impl DaemonState {
    /// The empty state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // --- capabilities (out-of-band administration) ---------------------------------

    /// Register a capability, optionally as a delegation of `parent`.
    ///
    /// Capability administration is not an operation in this protocol version, so this is
    /// the surface a deployment provisions through. It performs no narrowing check: D6/D7
    /// are enforced at admission time, on the chain, so a mis-provisioned child is denied
    /// rather than silently trusted.
    pub fn register_capability(
        &mut self,
        descriptor: CapabilityDescriptor,
        parent: Option<CapabilityHandle>,
    ) {
        self.capabilities.insert(
            descriptor.capability.clone(),
            CapabilityGrant { descriptor, parent },
        );
    }

    /// Revoke a capability.
    ///
    /// > Revocation, not purge, is how a capability stops being usable.
    /// >
    /// > — docs/35, as cited by `continuum_workspace::publication::ReferenceStore::revoke`
    ///
    /// Revoking a capability that was never registered is a no-op and is not reported:
    /// probing the registry through this surface reveals nothing, and a revoked token is
    /// indistinguishable from an unregistered one at admission (RFC 0027 X1).
    pub fn revoke_capability(&mut self, capability: &CapabilityHandle) {
        self.capabilities.remove(capability);
    }

    /// What `capability` confers, or [`None`] when it is unregistered or revoked.
    #[must_use]
    pub fn grant(&self, capability: &CapabilityHandle) -> Option<&CapabilityGrant> {
        self.capabilities.get(capability)
    }

    // --- audit ---------------------------------------------------------------------

    /// Record one admission decision (RFC 0027 P5).
    pub fn record_admission(&mut self, record: AdmissionRecord) {
        self.admissions.push(record);
    }

    /// Every admission decision so far, in decision order.
    #[must_use]
    pub fn admissions(&self) -> &[AdmissionRecord] {
        &self.admissions
    }

    // --- staged content ------------------------------------------------------------

    /// Stage one file's content and return the commitment that names it.
    ///
    /// The commitment is derived from a canonical record of the *path and the content*
    /// together, not the content alone, because a snapshot component has to be placeable:
    /// two files with identical bytes at two paths are two components, and one commitment
    /// for both would make a snapshot unreconstructable from `SnapshotComponents.files`.
    ///
    /// Until protocol 3.2 that derivation was also the *only* place a file's path existed:
    /// `files` was a bare commitment list, so the path had to be smuggled through the
    /// preimage and recovered from daemon-held state, which is the workaround bn-3gi
    /// documented and bn-i4aem paid. `SnapshotComponents.file_components` now declares the
    /// placement on the wire (`rule snapshot.file_components`), and the preimage is no
    /// longer carrying that weight — a request says where its files go, and
    /// `daemon::workspace` checks the two statements against each other instead of
    /// deriving one from the other.
    ///
    /// The preimage itself is unchanged, deliberately. It binds path to content, which is
    /// what makes the agreement check mean something, and it is the same derivation
    /// `evidence.verify` re-runs for INV-004 ([`commit_of`](DaemonState::commit_of)):
    /// changing it here would move an identity two subsystems agree on for the sake of a
    /// field that is now checked rather than needed.
    ///
    /// # Errors
    ///
    /// [`ServiceError::Identity`] when the identity seam cannot name the record.
    pub fn stage(
        &mut self,
        identifier: &dyn continuum_workspace::publication::ContentIdentifier,
        path: WorkspacePath,
        content: Vec<u8>,
    ) -> Result<Commitment, ServiceError> {
        let commitment = Self::commit_of(identifier, &path, &content)?;
        self.content
            .insert(commitment.clone(), StagedFile { path, content });
        Ok(commitment)
    }

    /// The commitment a `(path, content)` record has, without staging it.
    ///
    /// Factored out of [`stage`](DaemonState::stage) rather than duplicated because the
    /// daemon derives this value twice for two different reasons — once to *name* staged
    /// content and once to *re-derive* it, which is the independent check
    /// `evidence.verify` runs (INV-004) — and a second spelling of the preimage would make
    /// the check pass against its own copy of the rule rather than against the rule.
    ///
    /// # Errors
    ///
    /// [`ServiceError::Identity`] when the identity seam cannot name the record.
    pub fn commit_of(
        identifier: &dyn continuum_workspace::publication::ContentIdentifier,
        path: &WorkspacePath,
        content: &[u8],
    ) -> Result<Commitment, ServiceError> {
        let mut preimage = Vec::new();
        let rendered = path.to_string();
        for part in [rendered.as_bytes(), content] {
            preimage.extend_from_slice(&(part.len() as u64).to_be_bytes());
            preimage.extend_from_slice(part);
        }
        let handle = identifier
            .identify(
                continuum_workspace::artifact_path::ArtifactClass::WorkspaceSnapshot,
                &preimage,
            )
            .map_err(|_| ServiceError::Identity)?;
        Ok(Commitment::new(&handle.to_string()))
    }

    /// The file a commitment names, or [`None`] when nothing is staged under it.
    #[must_use]
    pub fn staged(&self, commitment: &Commitment) -> Option<&StagedFile> {
        self.content.get(commitment)
    }

    // --- component sets (`rule snapshot.by_reference`, protocol 3.6) ----------------

    /// Register a `SnapshotComponents` value under its content identity, and return that
    /// identity.
    ///
    /// Two callers, one surface — `put_context_pack`'s shape, for the same reason. A
    /// deployment registers the component sets it already holds here, out of band through
    /// [`Daemon::state_mut`](super::Daemon::state_mut) (IDL §7), exactly as it stages the
    /// content those sets name; and `workspace.create` writes through the same method on
    /// every accepted inline create, so **the inline form is what registers what the
    /// by-reference form can then name**. That is the whole of "whatever registration verb
    /// RFC 0019's manifest needs": there is no verb, because a components identity is
    /// derivable from the components and content already reaches a daemon out of band in
    /// any case (`rule snapshot.file_components`).
    ///
    /// The identity is over the value's `canonical_json` encoding and not the connection's
    /// negotiation, which is `rule snapshot.by_reference`'s own clause: an identity that
    /// moved with the negotiated encoding would give one component set two names on two
    /// connections. It goes through the deployment's [`ContentIdentifier`] seam, as every
    /// other identity in this daemon does.
    ///
    /// [`ContentIdentifier`]: continuum_workspace::publication::ContentIdentifier
    ///
    /// # Errors
    ///
    /// [`ServiceError::Identity`] when the value cannot be canonically encoded or the
    /// identity seam cannot name it.
    pub fn register_components(
        &mut self,
        identifier: &dyn continuum_workspace::publication::ContentIdentifier,
        components: SnapshotComponents,
    ) -> Result<Commitment, ServiceError> {
        let commitment = Self::components_identity(identifier, &components)?;
        self.components.insert(commitment.clone(), components);
        Ok(commitment)
    }

    /// The content identity a `SnapshotComponents` value has, without registering it.
    ///
    /// Factored out for [`stage`](DaemonState::stage)'s reason: a client derives this
    /// value to *name* a set and the daemon derives it to *resolve* one, and two spellings
    /// of the preimage would make the resolution agree with its own copy of the rule
    /// rather than with the rule. The preimage is length-prefixed exactly as
    /// [`commit_of`](DaemonState::commit_of)'s is, over a domain tag and the canonical
    /// bytes, so no other record can collide with it by construction.
    ///
    /// # Errors
    ///
    /// [`ServiceError::Identity`] when the value cannot be canonically encoded or the
    /// identity seam cannot name it.
    pub fn components_identity(
        identifier: &dyn continuum_workspace::publication::ContentIdentifier,
        components: &SnapshotComponents,
    ) -> Result<Commitment, ServiceError> {
        let canonical = crate::codec::to_bytes(components).map_err(|_| ServiceError::Identity)?;
        let mut preimage = Vec::new();
        for part in [b"snapshot-components".as_slice(), canonical.as_slice()] {
            preimage.extend_from_slice(&(part.len() as u64).to_be_bytes());
            preimage.extend_from_slice(part);
        }
        let handle = identifier
            .identify(
                continuum_workspace::artifact_path::ArtifactClass::WorkspaceSnapshot,
                &preimage,
            )
            .map_err(|_| ServiceError::Identity)?;
        Ok(Commitment::new(&handle.to_string()))
    }

    /// The component set a commitment names, or [`None`] when this daemon holds none.
    ///
    /// [`None`] is answered as `CapabilityDenied` on the wire, byte identical with every
    /// other denial: there is no `UnknownComponents` code and inventing a distinguishable
    /// not-found is the existence oracle RFC 0027 X2 forbids.
    #[must_use]
    pub fn components(&self, commitment: &Commitment) -> Option<&SnapshotComponents> {
        self.components.get(commitment)
    }

    // --- Context Packs (out-of-band administration) ---------------------------------

    /// Register a published Context Pack this daemon can expand.
    ///
    /// Two callers, one surface. A deployment registers the packs it already holds here, out of
    /// band through [`Daemon::state_mut`](super::Daemon::state_mut) (IDL §7), exactly as it
    /// stages content and provisions capabilities; and since bn-1y4qc `context.compile` writes
    /// through this same method, so a compiled pack is navigable the moment its `ctx_*` reaches
    /// the wire. The record's own constructor is what refuses a pack this daemon could not
    /// navigate, so nothing unexpandable can be registered by either route.
    pub fn put_context_pack(
        &mut self,
        handle: ContextHandle,
        record: super::context::ContextPackRecord,
    ) {
        self.context_packs.insert(handle, record);
    }

    /// Register the projection one `ev_*` evidence root compiles from.
    ///
    /// The out-of-band half of `context.compile`. Stage 2's declared input is a CIR causal
    /// order and `continuum-cir` is a PR-17 scaffold, so nothing here derives a candidate order
    /// from an evidence graph; a deployment registers the one it holds and the landed pipeline
    /// does the rest. See [`context::ContextCompileSource`](super::context::ContextCompileSource)
    /// for what is registered and what is emphatically not.
    pub fn put_compile_source(
        &mut self,
        root: &ArtifactHandle,
        source: super::context::ContextCompileSource,
    ) {
        self.compile_sources
            .insert(root.as_str().to_owned(), source);
    }

    /// The compile projection `root` names, or [`None`].
    #[must_use]
    pub fn compile_source(
        &self,
        root: &ArtifactHandle,
    ) -> Option<&super::context::ContextCompileSource> {
        self.compile_sources.get(root.as_str())
    }

    /// The Context Pack `handle` names, or [`None`].
    #[must_use]
    pub fn context_pack(
        &self,
        handle: &ContextHandle,
    ) -> Option<&super::context::ContextPackRecord> {
        self.context_packs.get(handle)
    }

    // --- intents -------------------------------------------------------------------

    /// Insert or replace an intent record.
    pub fn put_intent(&mut self, handle: IntentHandle, record: IntentRecord) {
        self.intents.insert(handle, record);
    }

    /// The intent record `handle` names, or [`None`].
    #[must_use]
    pub fn intent(&self, handle: &IntentHandle) -> Option<&IntentRecord> {
        self.intents.get(handle)
    }

    /// The intent record `handle` names, mutably.
    pub fn intent_mut(&mut self, handle: &IntentHandle) -> Option<&mut IntentRecord> {
        self.intents.get_mut(handle)
    }

    /// Every Intent Contract this daemon holds, in handle order.
    ///
    /// The sibling of [`evidence_nodes`](Self::evidence_nodes), and deterministic for the
    /// same reason: a [`BTreeMap`]'s iteration order is a function of the keys present and
    /// of nothing else.
    ///
    /// No wire operation is served from this — [`intent`](Self::intent) is what the family
    /// reads, one handle at a time, because "a caller learns nothing about any artifact" is
    /// what RFC 0027 X2's existence-oracle rule protects and an enumeration would be exactly
    /// such an oracle if a request could reach it. It exists for the *out-of-band* reader
    /// [`Daemon::state`](super::Daemon::state) already is: the G2 prompt-injection corpus's
    /// evidence has to assert "no intent's registry status moved" over the whole registry,
    /// and a check that could only look at the handles the test already knew would be blind
    /// to a status alteration that minted a record.
    pub fn intents(&self) -> impl Iterator<Item = (&IntentHandle, &IntentRecord)> {
        self.intents.iter()
    }

    /// Drop an intent record. `intent.reject` is the only caller: a rejected proposal never
    /// became a registry record, and the schema's `status` vocabulary has no `rejected`
    /// member to record it as.
    pub fn drop_intent(&mut self, handle: &IntentHandle) -> Option<IntentRecord> {
        self.intents.remove(handle)
    }

    // --- the evidence graph ----------------------------------------------------------

    /// Append one node to the evidence graph, or return the node already filed under
    /// `handle`.
    ///
    /// > The graph is append-only; nothing is edited in place. […] Idempotency keys
    /// > (RFC 0026) make agent retries safe: a replayed write returns the original node
    /// > identity.
    /// >
    /// > — RFC 0038, "Write and concurrency model"
    ///
    /// So a second append under an identity the graph already holds is a *convergence*, not
    /// an overwrite and not an error: the stored node is returned untouched, which is what
    /// makes "returns the original node identity" true even when the two appends disagree
    /// about everything else. The returned flag says which of the two happened, for the
    /// caller that must decide whether to emit a `node_published` event.
    pub fn append_evidence(
        &mut self,
        handle: EvidenceHandle,
        node: EvidenceNode,
    ) -> (&EvidenceNode, bool) {
        use std::collections::btree_map::Entry;
        match self.evidence.entry(handle) {
            Entry::Occupied(occupied) => (occupied.into_mut(), false),
            Entry::Vacant(vacant) => (vacant.insert(node), true),
        }
    }

    /// The evidence node `handle` names, or [`None`].
    #[must_use]
    pub fn evidence(&self, handle: &EvidenceHandle) -> Option<&EvidenceNode> {
        self.evidence.get(handle)
    }

    /// Every node in the graph, in handle order.
    ///
    /// Deterministic by the container: a [`BTreeMap`]'s iteration order is a function of
    /// the keys present and of nothing else, which is what `rule pagination.deterministic`
    /// needs from a query that returns a page of it.
    pub fn evidence_nodes(&self) -> impl Iterator<Item = (&EvidenceHandle, &EvidenceNode)> {
        self.evidence.iter()
    }

    /// Append one edge to the evidence graph, or return the edge already filed under
    /// `handle`.
    ///
    /// Put-if-absent, for the same sentence [`append_evidence`](Self::append_evidence)
    /// implements it for: the graph is append-only and a replayed write returns the
    /// original identity (RFC 0038). A checker that retries its own `evidence.link`
    /// converges on one edge, because `rule evidence.edge_identity` keeps `provenance`
    /// outside an edge's identity.
    pub fn append_edge(
        &mut self,
        handle: EvidenceHandle,
        edge: EvidenceEdge,
    ) -> (&EvidenceEdge, bool) {
        use std::collections::btree_map::Entry;
        match self.edges.entry(handle) {
            Entry::Occupied(occupied) => (occupied.into_mut(), false),
            Entry::Vacant(vacant) => (vacant.insert(edge), true),
        }
    }

    /// The edge `handle` names, or [`None`].
    #[must_use]
    pub fn evidence_edge(&self, handle: &EvidenceHandle) -> Option<&EvidenceEdge> {
        self.edges.get(handle)
    }

    /// Every edge in the graph, in handle order.
    pub fn evidence_edges(&self) -> impl Iterator<Item = (&EvidenceHandle, &EvidenceEdge)> {
        self.edges.iter()
    }

    /// Record that a node's referenced content has stopped being readable.
    ///
    /// The out-of-band half of plan §4.5: content is summarized after promotion, purged by
    /// key shred, or lost across a restore, and none of those three is an operation in this
    /// protocol version. It does not edit the node's claim — the redaction is *about the
    /// referenced content* — and it cannot invent one: a handle the graph does not hold is
    /// a no-op, so probing the graph through this surface reveals nothing.
    pub fn redact_evidence(&mut self, handle: &EvidenceHandle, redaction: Redacted) {
        if let Some(node) = self.evidence.get_mut(handle) {
            node.redaction = Some(redaction);
        }
    }

    /// Advance a claim's status. **The service-restricted write** (RFC 0038, INV-004).
    ///
    /// The `promotion` argument is not decoration and is not a parameter a caller chooses:
    /// [`Promotion`](super::evidence::Promotion) has private fields and no public
    /// constructor, so it can be built only inside `daemon::evidence` — the daemon's
    /// verification service. `daemon::observe`, which is where a *producer's* append runs,
    /// cannot name a value of this type, so "a producer may not promote its own claim" is
    /// enforced by what compiles rather than by a check a handler could omit.
    ///
    /// The advance itself is plan §11.7's compare-and-set, decided by
    /// [`continuum_evidence::claim_status::compare_and_set`] rather than restated here, so
    /// the daemon and any independent linearizability checker (plan §24.5) share one
    /// implementation of "would this write lower the claim?".
    ///
    /// # Errors
    ///
    /// [`PromotionRejected`] when the expected status is not the current one, or when the
    /// write would regress the lattice. A node the graph does not hold reports a lost
    /// compare-and-set against the lattice's bottom rather than a not-found, because a
    /// distinguishable not-found is the existence oracle RFC 0027 X2 forbids.
    pub fn promote_evidence(
        &mut self,
        handle: &EvidenceHandle,
        expected: ClaimStatus,
        promotion: &super::evidence::Promotion,
    ) -> Result<ClaimStatus, PromotionRejected> {
        let Some(node) = self.evidence.get_mut(handle) else {
            return Err(PromotionRejected::Conflict(StatusConflict {
                expected,
                actual: ClaimStatus::BOTTOM,
            }));
        };
        let settled = compare_and_set(node.status(), expected, promotion.status())?;
        node.history.push(promotion.write(settled));
        Ok(settled)
    }

    /// Record one committed evidence-graph delta.
    ///
    /// > `evidence.subscribe` streams typed evidence-graph deltas for a declared scope; the
    /// > graph itself remains authoritative on reconnect.
    /// >
    /// > — `rule subscription.hints_only`
    ///
    /// The log is the daemon-side half of that stream: a delta is recorded when it is
    /// *committed*, so a transport that drains this log can never deliver an event for a
    /// write that did not land.
    pub fn record_evidence_event(&mut self, event: EvidenceEvent) {
        self.evidence_events.push(event);
    }

    /// Every committed evidence-graph delta so far, in commit order.
    ///
    /// Append-only and never truncated, which is what lets a subscription be a *cursor*
    /// rather than a queue: an index into this slice means the same thing across the life of
    /// the daemon, so a connection can hold one and the daemon can hold no per-connection
    /// state at all (`rule subscription.delivery`, and INV-002 behind it). The transport
    /// reads it through [`Server::deliver`](crate::transport::Server::deliver).
    #[must_use]
    pub fn evidence_events(&self) -> &[EvidenceEvent] {
        &self.evidence_events
    }

    // --- workspaces and lineages ---------------------------------------------------

    /// Insert or replace a workspace record.
    pub fn put_workspace(&mut self, handle: WorkspaceHandle, record: WorkspaceRecord) {
        self.workspaces.insert(handle, record);
    }

    /// The workspace record `handle` names, or [`None`].
    #[must_use]
    pub fn workspace(&self, handle: &WorkspaceHandle) -> Option<&WorkspaceRecord> {
        self.workspaces.get(handle)
    }

    /// The workspace record `handle` names, mutably.
    pub fn workspace_mut(&mut self, handle: &WorkspaceHandle) -> Option<&mut WorkspaceRecord> {
        self.workspaces.get_mut(handle)
    }

    /// Replace a lineage with an advanced one — **the advance, not the opening**.
    ///
    /// `workspace.fork` is the only caller: it computes the next `Fork` from the current one
    /// through [`advance_current`](continuum_workspace::staleness::advance_current), which is
    /// a compare-and-set against the remembered head, so the value written here is always the
    /// value already held plus one step. That is why an unconditional insert is the right
    /// shape *here* — the guard is upstream, in the compare-and-set that produced the
    /// argument.
    ///
    /// [`open_lineage`](DaemonState::open_lineage) is the other half, and the distinction is
    /// bn-n1xou: `workspace.create` used this method, whose insert is unconditional, on a
    /// lineage it named from the *content identity* of the snapshot it built. So a second
    /// create of identical components rewound the live lineage to its origin. An opening is
    /// not an advance and no longer spells itself as one.
    pub fn put_lineage(&mut self, fork: Fork) {
        self.lineages.insert(fork.name().clone(), fork);
    }

    /// Open a lineage — **put-if-absent**: a name this daemon already holds keeps the
    /// lineage it has, advances and all.
    ///
    /// The convergent half of the pair above, and `workspace.create`'s only door to the
    /// lineage map. A `ForkName` here is derived from a content-addressed `ws_*` handle, so
    /// two creates naming the same components name the same lineage *by construction*; the
    /// question is only what the second one does to the first one's history, and the answer
    /// is nothing. That is the G0-DX-13 disposition for identical creation — "one semantic
    /// identity; no lost receipts" — read at the lineage: identical creation converges on the
    /// identity that exists, and the state it accumulated survives.
    pub fn open_lineage(&mut self, fork: Fork) {
        let name = fork.name().clone();
        self.lineages.entry(name).or_insert(fork);
    }

    /// The lineage `name` labels, or [`None`].
    #[must_use]
    pub fn lineage(&self, name: &ForkName) -> Option<&Fork> {
        self.lineages.get(name)
    }

    // --- tasks and the models they run over -----------------------------------------

    /// Every task this daemon holds, and every continuation that names one.
    #[must_use]
    pub const fn tasks(&self) -> &super::task::TaskTable {
        &self.tasks
    }

    /// The task table, mutably.
    pub const fn tasks_mut(&mut self) -> &mut super::task::TaskTable {
        &mut self.tasks
    }

    /// Every region this daemon has opened for task work, and what each teardown left
    /// behind.
    ///
    /// The daemon's structured-concurrency scope, and the evidence that it is one: PR 6
    /// puts every unit of task work inside a region that is opened and finalized within one
    /// dispatch, and [`TaskRegions::is_total`](super::region::TaskRegions::is_total) is that
    /// sentence as a value a caller — or a test — can read without re-deriving it.
    #[must_use]
    pub const fn regions(&self) -> &super::region::TaskRegions {
        &self.regions
    }

    /// The region ledger, mutably.
    ///
    /// The step surface: a unit of work says what it just did and the region layer decides
    /// whether that was legal. Opening and tearing down a scope is *not* reachable here —
    /// both are private to [`region`](super::region), and
    /// [`region::scoped`](super::region::scoped) is the only bracket that does either.
    pub const fn regions_mut(&mut self) -> &mut super::region::TaskRegions {
        &mut self.regions
    }

    /// The models this daemon can construct.
    ///
    /// Registered out of band, like capabilities and staged content, and for the same
    /// reason: elaborating CML source is not an operation in this protocol version, and
    /// there is no CML front end to do it with (see [`verification`](super::verification)).
    #[must_use]
    pub const fn models(&self) -> &super::verification::ModelCatalog {
        &self.models
    }

    /// The model catalog, mutably — the registration surface itself.
    pub const fn models_mut(&mut self) -> &mut super::verification::ModelCatalog {
        &mut self.models
    }

    // --- idempotency ----------------------------------------------------------------

    /// The replay recorded for `actor` under `key`, if any.
    #[must_use]
    pub fn replay(&self, actor: &str, key: &str) -> Option<&Replay> {
        self.idempotency.get(&(actor.to_owned(), key.to_owned()))
    }

    /// Record a mutation's result under `actor`'s `key`.
    ///
    /// Keys are "scoped per actor" (`rule idempotency.replay`), and RFC 0027 T4 is what
    /// makes that scoping meaningful: the actor is bound to the capability, so one
    /// principal cannot replay under another's key.
    pub fn record_replay(&mut self, actor: &str, key: &str, replay: Replay) {
        self.idempotency
            .insert((actor.to_owned(), key.to_owned()), replay);
    }
}

/// The authority level a wire descriptor confers, as the store spells it.
///
/// The two enums are the same five levels in the same order — RFC 0027's "Landed
/// vocabulary" clause requires it — and `tests/daemon_admission.rs` asserts the token-level
/// agreement rather than trusting this function.
#[must_use]
pub const fn store_level(
    level: AuthorityLevel,
) -> continuum_workspace::publication::AuthorityLevel {
    use continuum_workspace::publication::AuthorityLevel as Store;
    match level {
        AuthorityLevel::Read => Store::Read,
        AuthorityLevel::Propose => Store::Propose,
        AuthorityLevel::Execute => Store::Execute,
        AuthorityLevel::ReviseIntent => Store::ReviseIntent,
        AuthorityLevel::Promote => Store::Promote,
    }
}
