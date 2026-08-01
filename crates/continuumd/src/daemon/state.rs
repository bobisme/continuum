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
use continuum_intent::contract::IntentContract;
use continuum_value::assurance::ValidationBasis;
use continuum_workspace::components::WorkspaceDescriptor;
use continuum_workspace::lineage::{Fork, ForkName};
use continuum_workspace::snapshot::WorkspacePath;

use super::family::Arguments;
use super::{OperationOutcome, ServiceError};
use crate::protocol::envelope::{Redacted, RequestEnvelope};
use crate::protocol::handshake::CapabilityDescriptor;
use crate::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, EvidenceHandle, IntentHandle, Timestamp, WorkspaceHandle,
};
use crate::protocol::spec::Nullable;
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
    /// The assurance-result evidence class this node offers toward its claim.
    pub evidence_kind: EvidenceKind,
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
    /// The result the first execution produced, returned verbatim to a replay so that
    /// "the same task or artifact identity" is returned by construction.
    pub outcome: OperationOutcome,
}

/// The typed stand-in for `rule idempotency.replay`'s canonical request bytes.
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
    intents: BTreeMap<IntentHandle, IntentRecord>,
    evidence: BTreeMap<EvidenceHandle, EvidenceNode>,
    evidence_events: Vec<EvidenceEvent>,
    idempotency: BTreeMap<(String, String), Replay>,
    admissions: Vec<AdmissionRecord>,
    tasks: super::task::TaskTable,
    models: super::verification::ModelCatalog,
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
    /// See this module's sibling `workspace` for the wire gap this covers.
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

    /// Insert or replace a lineage.
    pub fn put_lineage(&mut self, fork: Fork) {
        self.lineages.insert(fork.name().clone(), fork);
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
