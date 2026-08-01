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

use continuum_intent::contract::IntentContract;
use continuum_workspace::components::WorkspaceDescriptor;
use continuum_workspace::lineage::{Fork, ForkName};
use continuum_workspace::snapshot::WorkspacePath;

use super::family::Arguments;
use super::{OperationOutcome, ServiceError};
use crate::protocol::envelope::RequestEnvelope;
use crate::protocol::handshake::CapabilityDescriptor;
use crate::protocol::scalar::{CapabilityHandle, Commitment, IntentHandle, WorkspaceHandle};
use crate::protocol::spec::Nullable;
use crate::protocol::vocabulary::AuthorityLevel;

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
    idempotency: BTreeMap<(String, String), Replay>,
    admissions: Vec<AdmissionRecord>,
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
        let mut preimage = Vec::new();
        let rendered = path.to_string();
        for part in [rendered.as_bytes(), content.as_slice()] {
            preimage.extend_from_slice(&(part.len() as u64).to_be_bytes());
            preimage.extend_from_slice(part);
        }
        let handle = identifier
            .identify(
                continuum_workspace::artifact_path::ArtifactClass::WorkspaceSnapshot,
                &preimage,
            )
            .map_err(|_| ServiceError::Identity)?;
        let commitment = Commitment::new(&handle.to_string());
        self.content
            .insert(commitment.clone(), StagedFile { path, content });
        Ok(commitment)
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
