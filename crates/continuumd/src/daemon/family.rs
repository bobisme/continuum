//! The handler-registration seam: what an operation family is, and what the dispatcher
//! guarantees before one runs.
//!
//! # The seam, in four steps
//!
//! A family is one module implementing [`OperationFamily`]. Adding the next one —
//! `task`, `evidence`, `verification` — is these four edits and no others:
//!
//! 1. **Declare its request and response bodies as variants.** [`Arguments`] gains one
//!    variant per operation carrying the IDL's request struct, and [`Payload`] one per
//!    operation carrying the response struct. Both are closed enums on purpose: an
//!    operation whose arguments cannot be named here is an operation the dispatcher cannot
//!    route, which is the fail-closed reading of an unknown shape.
//! 2. **Name each variant's operation.** [`Arguments::operation`] and
//!    [`Payload::operation`] return the wire name, and the dispatcher rejects a request
//!    whose envelope names a different operation than its arguments carry — the "wrong
//!    shape" half of `MalformedRequest`.
//! 3. **Implement [`OperationFamily`]**: a namespace, a pure [`OperationFamily::scope`]
//!    reading the handles the arguments name, and [`OperationFamily::handle`].
//! 4. **Register it** with `DaemonBuilder::family`.
//!
//! Nothing else changes. In particular a family never writes admission, obligation,
//! version, audit-correlation, or idempotency code: those run in
//! [`Daemon::dispatch`](super::Daemon::dispatch) *from registry data*, uniformly, for every
//! operation — including operations whose families have not landed, which is why an
//! unregistered namespace is a typed `UnsupportedSemanticFeature` and not a panic.
//!
//! # What a family may assume when `handle` is called
//!
//! - the request named this connection's negotiated protocol version;
//! - the operation is one of the registry's 83 and `Call::spec` is its entry;
//! - the arguments are the shape that operation declares;
//! - admission T1–T4 passed for this actor, this capability, and every handle
//!   [`OperationFamily::scope`] reported — so no family re-checks authority, and none may
//!   weaken it;
//! - every annotation obligation the registry states for the operation is satisfied;
//! - for a `@mutation`, no earlier call under this actor's idempotency key had a different
//!   request.
//!
//! # What a family must not do
//!
//! Return an [`ErrorCode`] outside the union `rule errors.common` and the operation's own
//! `errors` clause allow. The dispatcher checks it — [`Fault`] is checked against
//! [`Call::spec`] on the way out — so a family that tries fails a debug assertion in test
//! builds rather than putting an undeclared code on the wire.

use continuum_workspace::publication::ReferenceStore;

use super::state::DaemonState;
use super::{Services, errors};
use crate::protocol::envelope::{
    ArtifactRef, AssuranceEnvelope, CertificateRejection, Omission, RequestEnvelope, Verdict,
    Warning,
};
use crate::protocol::handshake::CapabilityDescriptor;
use crate::protocol::operations::context::{
    ContextCompileRequest, ContextCompileResponse, ContextExpandRequest, ContextExpandResponse,
};
use crate::protocol::operations::evidence::{
    EvidenceGetRequest, EvidenceGetResponse, EvidenceLinkRequest, EvidenceLinkResponse,
    EvidenceQueryRequest, EvidenceQueryResponse, EvidenceSubscribeRequest,
    EvidenceSubscribeResponse, EvidenceVerifyRequest, EvidenceVerifyResponse,
};
use crate::protocol::operations::intent::{
    IntentAcceptRequest, IntentAcceptResponse, IntentDiffRequest, IntentDiffResponse,
    IntentExportBundleRequest, IntentExportBundleResponse, IntentGetRequest, IntentGetResponse,
    IntentImportBundleRequest, IntentImportBundleResponse, IntentLockRequest, IntentLockResponse,
    IntentProposeRevisionRequest, IntentProposeRevisionResponse, IntentRejectRequest,
    IntentRejectResponse,
};
use crate::protocol::operations::observe::{
    ObserveClassifyRequest, ObserveClassifyResponse, ObserveIngestRequest, ObserveIngestResponse,
    ObserveResultRequest,
};
use crate::protocol::operations::signing::{
    SigningMintRequest, SigningMintResponse, SigningRegistryRequest, SigningRegistryResponse,
    SigningRevokeRequest, SigningRevokeResponse, SigningRotateRequest, SigningRotateResponse,
    SigningSignPackRequest, SigningSignPackResponse, SigningVerifyRequest, SigningVerifyResponse,
};
use crate::protocol::operations::task::{
    TaskCancelRequest, TaskCancelResponse, TaskResumeRequest, TaskResumeResponse,
    TaskStatusRequest, TaskSubscribeRequest, TaskSubscribeResponse, TaskUpdateBudgetRequest,
    TaskUpdateBudgetResponse,
};
use crate::protocol::operations::verification::{
    VerificationAwaitRequest, VerificationResultRequest, VerificationStartRequest,
    VerificationStartResponse,
};
use crate::protocol::operations::whiteboard::{
    WhiteboardCompileRequest, WhiteboardCompileResponse,
};
use crate::protocol::operations::workspace::{
    WorkspaceCreateByReferenceRequest, WorkspaceCreateByReferenceResponse, WorkspaceCreateRequest,
    WorkspaceCreateResponse, WorkspaceDiffRequest, WorkspaceDiffResponse, WorkspaceForkRequest,
    WorkspaceForkResponse, WorkspaceSealRequest, WorkspaceSealResponse,
};
use crate::protocol::scalar::{
    AuditCorrelationId, ContinuationHandle, IntentHandle, TaskHandle, WorkspaceHandle,
};
use crate::protocol::shared::VerificationResult;
use crate::protocol::spec::{Nullable, OperationSpec, Optional};
use crate::protocol::task::TaskRecord;
use crate::protocol::vocabulary::{ErrorCode, ResultStatus};

/// A decoded operation request body.
///
/// The envelope's `arguments` field is declared `Opaque`, and this layer deliberately does
/// not decode it: the operation layer takes the request *already decoded*, and
/// [`crate::codec::operations::decode_arguments`] is what produces this value from the
/// envelope's bytes (`rule encoding.opaque_payloads`, protocol 3.2). Everything the wire
/// fixes — the field names, types, and three-valued presence of each body — is enforced
/// either way, because each variant carries the IDL's own struct.
///
/// This enum is therefore one half of a pair: a new operation is a variant here and an arm
/// in that table, and a mismatch between them is a compile error rather than a decode that
/// silently produces the wrong shape.
// `workspace.create`'s request body carries `SnapshotComponents`, which is ten commitment
// lists, and is several times the size of the smallest body here. Boxing it would make the
// seam asymmetric — "add one variant carrying the IDL's request struct" would become "add
// one variant carrying the IDL's request struct, boxed if it is large" — and would buy
// nothing on the path that matters, because the ledger clones the *contents* either way.
// The uniform shape is worth more than the moved bytes.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arguments {
    /// `workspace.create`.
    WorkspaceCreate(WorkspaceCreateRequest),
    /// `workspace.fork`.
    WorkspaceFork(WorkspaceForkRequest),
    /// `workspace.diff`.
    WorkspaceDiff(WorkspaceDiffRequest),
    /// `workspace.seal`.
    WorkspaceSeal(WorkspaceSealRequest),
    /// `intent.get`.
    IntentGet(IntentGetRequest),
    /// `intent.diff`.
    IntentDiff(IntentDiffRequest),
    /// `intent.propose_revision`.
    IntentProposeRevision(IntentProposeRevisionRequest),
    /// `intent.accept`.
    IntentAccept(IntentAcceptRequest),
    /// `intent.reject`.
    IntentReject(IntentRejectRequest),
    /// `intent.lock`.
    IntentLock(IntentLockRequest),
    /// `evidence.get`.
    EvidenceGet(EvidenceGetRequest),
    /// `evidence.query`.
    EvidenceQuery(EvidenceQueryRequest),
    /// `evidence.verify`.
    EvidenceVerify(EvidenceVerifyRequest),
    /// `evidence.subscribe`.
    EvidenceSubscribe(EvidenceSubscribeRequest),
    /// `evidence.link`.
    EvidenceLink(EvidenceLinkRequest),
    /// `observe.ingest`.
    ObserveIngest(ObserveIngestRequest),
    /// `observe.classify`.
    ObserveClassify(ObserveClassifyRequest),
    /// `observe.result`.
    ObserveResult(ObserveResultRequest),
    /// `verification.start`.
    VerificationStart(VerificationStartRequest),
    /// `verification.result`.
    VerificationResult(VerificationResultRequest),
    /// `verification.await`.
    VerificationAwait(VerificationAwaitRequest),
    /// `task.status`.
    TaskStatus(TaskStatusRequest),
    /// `task.cancel`.
    TaskCancel(TaskCancelRequest),
    /// `task.resume`.
    TaskResume(TaskResumeRequest),
    /// `task.subscribe`.
    TaskSubscribe(TaskSubscribeRequest),
    /// `task.update_budget`.
    TaskUpdateBudget(TaskUpdateBudgetRequest),
    /// `context.compile`.
    ContextCompile(ContextCompileRequest),
    /// `context.expand`.
    ContextExpand(ContextExpandRequest),
    /// `whiteboard.compile`.
    WhiteboardCompile(WhiteboardCompileRequest),
    /// `workspace.create_by_reference`.
    WorkspaceCreateByReference(WorkspaceCreateByReferenceRequest),
    /// `intent.export_bundle` (3.8).
    IntentExportBundle(IntentExportBundleRequest),
    /// `intent.import_bundle` (3.8).
    IntentImportBundle(IntentImportBundleRequest),
    /// `signing.mint` (3.8).
    SigningMint(SigningMintRequest),
    /// `signing.rotate` (3.8).
    SigningRotate(SigningRotateRequest),
    /// `signing.revoke` (3.8).
    SigningRevoke(SigningRevokeRequest),
    /// `signing.registry` (3.8).
    SigningRegistry(SigningRegistryRequest),
    /// `signing.verify` (3.8).
    SigningVerify(SigningVerifyRequest),
    /// `signing.sign_pack` (3.8).
    SigningSignPack(SigningSignPackRequest),
}

impl Arguments {
    /// The wire operation whose `request` body this is.
    #[must_use]
    pub const fn operation(&self) -> &'static str {
        match self {
            Self::WorkspaceCreate(_) => "workspace.create",
            Self::WorkspaceFork(_) => "workspace.fork",
            Self::WorkspaceDiff(_) => "workspace.diff",
            Self::WorkspaceSeal(_) => "workspace.seal",
            Self::IntentGet(_) => "intent.get",
            Self::IntentDiff(_) => "intent.diff",
            Self::IntentProposeRevision(_) => "intent.propose_revision",
            Self::IntentAccept(_) => "intent.accept",
            Self::IntentReject(_) => "intent.reject",
            Self::IntentLock(_) => "intent.lock",
            Self::EvidenceGet(_) => "evidence.get",
            Self::EvidenceQuery(_) => "evidence.query",
            Self::EvidenceVerify(_) => "evidence.verify",
            Self::EvidenceSubscribe(_) => "evidence.subscribe",
            Self::EvidenceLink(_) => "evidence.link",
            Self::ObserveIngest(_) => "observe.ingest",
            Self::ObserveClassify(_) => "observe.classify",
            Self::ObserveResult(_) => "observe.result",
            Self::VerificationStart(_) => "verification.start",
            Self::VerificationResult(_) => "verification.result",
            Self::VerificationAwait(_) => "verification.await",
            Self::TaskStatus(_) => "task.status",
            Self::TaskCancel(_) => "task.cancel",
            Self::TaskResume(_) => "task.resume",
            Self::TaskSubscribe(_) => "task.subscribe",
            Self::TaskUpdateBudget(_) => "task.update_budget",
            Self::ContextCompile(_) => "context.compile",
            Self::ContextExpand(_) => "context.expand",
            Self::WhiteboardCompile(_) => "whiteboard.compile",
            Self::WorkspaceCreateByReference(_) => "workspace.create_by_reference",
            Self::IntentExportBundle(_) => "intent.export_bundle",
            Self::IntentImportBundle(_) => "intent.import_bundle",
            Self::SigningMint(_) => "signing.mint",
            Self::SigningRotate(_) => "signing.rotate",
            Self::SigningRevoke(_) => "signing.revoke",
            Self::SigningRegistry(_) => "signing.registry",
            Self::SigningVerify(_) => "signing.verify",
            Self::SigningSignPack(_) => "signing.sign_pack",
        }
    }
}

/// A typed operation response body.
///
/// Carried beside the [`ResultEnvelope`](crate::protocol::envelope::ResultEnvelope) rather
/// than inside its `payload` field for the reason [`Arguments`] gives: this layer emits no
/// bytes, so the envelope's `payload` reads `null` here and
/// [`crate::transport::Server::answer`] is what encodes this value into it before the
/// frame is written. `tests/daemon_operations.rs` pins that the two agree, and
/// `tests/transport_local.rs` pins that the field carries real bytes at the boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Payload {
    /// No response body: the result is an error, and `payload` is null on `status = error`.
    None,
    /// `workspace.create`.
    WorkspaceCreate(WorkspaceCreateResponse),
    /// `workspace.fork`.
    WorkspaceFork(WorkspaceForkResponse),
    /// `workspace.diff`.
    WorkspaceDiff(WorkspaceDiffResponse),
    /// `workspace.seal`.
    WorkspaceSeal(WorkspaceSealResponse),
    /// `intent.get`.
    IntentGet(IntentGetResponse),
    /// `intent.diff`.
    IntentDiff(IntentDiffResponse),
    /// `intent.propose_revision`.
    IntentProposeRevision(IntentProposeRevisionResponse),
    /// `intent.accept`.
    IntentAccept(IntentAcceptResponse),
    /// `intent.reject`.
    IntentReject(IntentRejectResponse),
    /// `intent.lock`.
    IntentLock(IntentLockResponse),
    /// `evidence.get`.
    EvidenceGet(EvidenceGetResponse),
    /// `evidence.query`.
    EvidenceQuery(EvidenceQueryResponse),
    /// `evidence.verify`.
    EvidenceVerify(EvidenceVerifyResponse),
    /// `evidence.subscribe`.
    EvidenceSubscribe(EvidenceSubscribeResponse),
    /// `evidence.link`.
    EvidenceLink(EvidenceLinkResponse),
    /// `observe.ingest`.
    ObserveIngest(ObserveIngestResponse),
    /// `observe.classify`.
    ObserveClassify(ObserveClassifyResponse),
    /// `observe.result`. The IDL declares the *named* body `VerificationResult` here rather
    /// than an anonymous one, so this variant carries the shared type.
    ObserveResult(VerificationResult),
    /// `verification.start`.
    VerificationStart(VerificationStartResponse),
    /// `verification.result`. The IDL declares a *named* response body here
    /// (`response VerificationResult;`), shared with five other operations, so the variant
    /// carries the shared struct rather than an anonymous one.
    VerificationResult(VerificationResult),
    /// `verification.await`, whose named response body is the same shared struct.
    VerificationAwait(VerificationResult),
    /// `task.status`, whose named response body is [`TaskRecord`].
    TaskStatus(TaskRecord),
    /// `task.cancel`.
    TaskCancel(TaskCancelResponse),
    /// `task.resume`.
    TaskResume(TaskResumeResponse),
    /// `task.subscribe`.
    TaskSubscribe(TaskSubscribeResponse),
    /// `task.update_budget`.
    TaskUpdateBudget(TaskUpdateBudgetResponse),
    /// `context.compile`. Declared beside its request because the codec's two tables and
    /// these two enums are one seam — see this module's four-step list — even though the
    /// `context` family answers this operation with a typed refusal rather than a body.
    ContextCompile(ContextCompileResponse),
    /// `context.expand`.
    ContextExpand(ContextExpandResponse),
    /// `whiteboard.compile`.
    WhiteboardCompile(WhiteboardCompileResponse),
    /// `workspace.create_by_reference`.
    WorkspaceCreateByReference(WorkspaceCreateByReferenceResponse),
    /// `intent.export_bundle` (3.8).
    IntentExportBundle(IntentExportBundleResponse),
    /// `intent.import_bundle` (3.8).
    IntentImportBundle(IntentImportBundleResponse),
    /// `signing.mint` (3.8).
    SigningMint(SigningMintResponse),
    /// `signing.rotate` (3.8).
    SigningRotate(SigningRotateResponse),
    /// `signing.revoke` (3.8).
    SigningRevoke(SigningRevokeResponse),
    /// `signing.registry` (3.8).
    SigningRegistry(SigningRegistryResponse),
    /// `signing.verify` (3.8).
    SigningVerify(SigningVerifyResponse),
    /// `signing.sign_pack` (3.8).
    SigningSignPack(SigningSignPackResponse),
}

impl Payload {
    /// The wire operation whose `response` body this is, or [`None`] for
    /// [`Payload::None`].
    #[must_use]
    pub const fn operation(&self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::WorkspaceCreate(_) => Some("workspace.create"),
            Self::WorkspaceFork(_) => Some("workspace.fork"),
            Self::WorkspaceDiff(_) => Some("workspace.diff"),
            Self::WorkspaceSeal(_) => Some("workspace.seal"),
            Self::IntentGet(_) => Some("intent.get"),
            Self::IntentDiff(_) => Some("intent.diff"),
            Self::IntentProposeRevision(_) => Some("intent.propose_revision"),
            Self::IntentAccept(_) => Some("intent.accept"),
            Self::IntentReject(_) => Some("intent.reject"),
            Self::IntentLock(_) => Some("intent.lock"),
            Self::EvidenceGet(_) => Some("evidence.get"),
            Self::EvidenceQuery(_) => Some("evidence.query"),
            Self::EvidenceVerify(_) => Some("evidence.verify"),
            Self::EvidenceSubscribe(_) => Some("evidence.subscribe"),
            Self::EvidenceLink(_) => Some("evidence.link"),
            Self::ObserveIngest(_) => Some("observe.ingest"),
            Self::ObserveClassify(_) => Some("observe.classify"),
            Self::ObserveResult(_) => Some("observe.result"),
            Self::VerificationStart(_) => Some("verification.start"),
            Self::VerificationResult(_) => Some("verification.result"),
            Self::VerificationAwait(_) => Some("verification.await"),
            Self::TaskStatus(_) => Some("task.status"),
            Self::TaskCancel(_) => Some("task.cancel"),
            Self::TaskResume(_) => Some("task.resume"),
            Self::TaskSubscribe(_) => Some("task.subscribe"),
            Self::TaskUpdateBudget(_) => Some("task.update_budget"),
            Self::ContextCompile(_) => Some("context.compile"),
            Self::ContextExpand(_) => Some("context.expand"),
            Self::WhiteboardCompile(_) => Some("whiteboard.compile"),
            Self::WorkspaceCreateByReference(_) => Some("workspace.create_by_reference"),
            Self::IntentExportBundle(_) => Some("intent.export_bundle"),
            Self::IntentImportBundle(_) => Some("intent.import_bundle"),
            Self::SigningMint(_) => Some("signing.mint"),
            Self::SigningRotate(_) => Some("signing.rotate"),
            Self::SigningRevoke(_) => Some("signing.revoke"),
            Self::SigningRegistry(_) => Some("signing.registry"),
            Self::SigningVerify(_) => Some("signing.verify"),
            Self::SigningSignPack(_) => Some("signing.sign_pack"),
        }
    }
}

/// The instances and classes a request names, read before any handler runs.
///
/// RFC 0027 T2 scopes admission by "every snapshot, intent, and artifact class the request
/// names", and a request names them in two places: the envelope's `snapshot` and `intent`
/// fields, and the operation's own arguments (`workspace.fork.base`, `intent.diff.after`,
/// …). The dispatcher reads the first; a family declares the second here. The function is
/// pure in the arguments and is given no state, so producing the scope claim cannot itself
/// be the store lookup RFC 0027 X3 forbids before admission.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScopeClaim {
    /// Snapshot instances the arguments name.
    pub snapshots: Vec<WorkspaceHandle>,
    /// Intent instances the arguments name.
    pub intents: Vec<IntentHandle>,
    /// Artifact classes the request touches, as plan §4.4 class tokens
    /// (`rule artifact_class.spelling`).
    pub classes: Vec<&'static str>,
    /// Every other instance the arguments name, as handle spellings: the `ev_`, `task_`,
    /// `cont_`, `ctx_`, and other handles T2 decides against `CapabilityDescriptor.instances`
    /// (`rule capability.instance_scope`). Each handle's class is its prefix. A handle a
    /// request *returns* or *derives* is not named here, and neither is one a handler would
    /// resolve from state: this list is pure in the arguments.
    pub instances: Vec<String>,
}

/// Everything the dispatcher established before the family ran.
#[derive(Debug, Clone, Copy)]
pub struct Call<'a> {
    /// The registry entry for the operation: its authority, annotations, bodies, and the
    /// error codes it may add to `rule errors.common`.
    pub spec: &'static OperationSpec,
    /// The request envelope, exactly as it arrived.
    pub envelope: &'a RequestEnvelope,
    /// The decoded request body, already checked to be this operation's shape.
    pub arguments: &'a Arguments,
    /// What the presented capability confers, after admission accepted it. A family reads
    /// it to name the actor and the store token a publication runs under; it never
    /// re-decides authority, and cannot widen it — every test has already run.
    pub grant: &'a CapabilityDescriptor,
    /// The audit-correlation identity of this call, derived from the request identity
    /// alone (`rule audit.correlation`). A family that writes an audit record cites this
    /// value rather than minting one.
    pub audit: &'a AuditCorrelationId,
    /// Every derived handle this call admitted through [`Call::admits`].
    /// The dispatcher files it with the replay record, so a replay re-decides each one
    /// against the presenting grant (cr-3lrkq3). A set: a traversal that decides one
    /// handle twice records it once, and insertion is logarithmic. A shared reference, so
    /// a copy of the call records into the same log and no decision escapes it.
    pub(crate) consulted:
        &'a std::cell::RefCell<std::collections::BTreeSet<super::admission::Consulted>>,
}

impl Call<'_> {
    /// Refuse, with the one `CapabilityDenied`, a handle this call reached from one it
    /// named when the grant does not admit it ([`admits_derived`]).
    ///
    /// Called by a handler after the lookup that found the derived handle and before it
    /// reads, writes, or reports anything about it, so an out-of-scope derived instance
    /// leaves no trace: nothing published, nothing answered (cr-3hcpn4).
    ///
    /// # Errors
    ///
    /// [`Fault::denied`] when the derived handle is out of scope.
    ///
    /// [`admits_derived`]: super::admission::admits_derived
    pub fn derived(&self, derived: super::admission::Derived<'_>) -> Result<(), Fault> {
        if self.admits(derived) {
            Ok(())
        } else {
            let mut denied = Fault::denied();
            denied.derived_denial = true;
            Err(denied)
        }
    }

    /// Whether the grant admits a handle this call reached from one it named
    /// ([`admits_derived`]), recording an admitted handle for the replay ledger.
    ///
    /// Only an admitted handle is recorded. A refused one either ends the call in the one
    /// denial, which names nothing, or was filtered out, and the outcome says nothing about
    /// it; recording it would make a legitimate replay under the identical grant fail.
    ///
    /// Every derived-handle decision a handler makes goes through here, directly or through
    /// [`Call::derived`]. That is what lets a replay, which does not run the handler,
    /// re-decide exactly the handles the original call decided (cr-3lrkq3).
    ///
    /// [`admits_derived`]: super::admission::admits_derived
    #[must_use]
    pub fn admits(&self, derived: super::admission::Derived<'_>) -> bool {
        let admitted = super::admission::admits_derived(self.grant, derived);
        if admitted {
            self.consulted
                .borrow_mut()
                .insert(super::admission::Consulted::of(derived));
        }
        admitted
    }
}

/// Where a family's result lands on the result envelope's `status` lane, and which
/// handles the envelope names because of it.
///
/// > `task` — present when `status` is `task_started` or `task_suspended`, and on results
/// > of task-observing operations.
/// > `continuation` — present when `status = task_suspended`, and on a resumable failure.
/// >
/// > — `ResultEnvelope`, IDL §6
///
/// The three lanes are a closed enum rather than three independent fields because the
/// presence rules above are a *joint* condition: `task_suspended` without a continuation,
/// or `task_started` without a task, is not a result envelope the IDL admits, and three
/// fields set independently can spell both. Here they cannot be spelled at all.
///
/// [`Completion::Answered`] carries an optional task for the second half of the `task`
/// rule — a task-observing operation at `status = ok`, such as `task.status` or
/// `verification.result`, names the task its answer is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Completion {
    /// `status = ok`: the operation answered.
    Answered {
        /// The task this answer is about, for a task-observing operation.
        task: Optional<TaskHandle>,
    },
    /// `status = task_started`: work was started and the handle is the answer.
    TaskStarted {
        /// The started task.
        task: TaskHandle,
    },
    /// `status = task_suspended`: work parked with committed partial evidence plus a
    /// valid continuation (`rule task.cancel_correct`, `rule task.update_budget`).
    TaskSuspended {
        /// The parked task.
        task: TaskHandle,
        /// The continuation that resumes it. Never absent on this lane.
        continuation: ContinuationHandle,
    },
}

impl Completion {
    /// The envelope status this lane reports.
    #[must_use]
    pub const fn status(&self) -> ResultStatus {
        match self {
            Self::Answered { .. } => ResultStatus::Ok,
            Self::TaskStarted { .. } => ResultStatus::TaskStarted,
            Self::TaskSuspended { .. } => ResultStatus::TaskSuspended,
        }
    }

    /// The task the envelope names, if any.
    #[must_use]
    pub fn task(&self) -> Optional<TaskHandle> {
        match self {
            Self::Answered { task } => task.clone(),
            Self::TaskStarted { task } | Self::TaskSuspended { task, .. } => {
                Optional::Present(task.clone())
            }
        }
    }

    /// The continuation the envelope names, if any.
    #[must_use]
    pub fn continuation(&self) -> Optional<ContinuationHandle> {
        match self {
            Self::Answered { .. } | Self::TaskStarted { .. } => Optional::Absent,
            Self::TaskSuspended { continuation, .. } => Optional::Present(continuation.clone()),
        }
    }
}

/// What a family produced on success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effect {
    /// The operation's response body.
    pub payload: Payload,
    /// The envelope status lane this result lands on.
    ///
    /// Until bn-i4aem item 9 there was no such field and
    /// [`result::success`](super::result::success) hard-coded `status = ok` with `task`
    /// and `continuation` absent, so the `task_started`/`task_suspended` lane the IDL
    /// declares was unreachable however the operation behaved. A parked campaign reported
    /// `ok` and left the caller to find the continuation in a typed payload — true, but
    /// not what the envelope says.
    pub completion: Completion,
    /// The operation's typed verdict. `Null` exactly when the operation declares no
    /// `verdict` clause; the dispatcher checks the agreement against
    /// [`OperationSpec::verdict`].
    pub verdict: Nullable<Verdict>,
    /// The nine-dimension assurance envelope.
    ///
    /// > A result whose `verdict` is `semantic` or `evaluation` MUST carry the
    /// > nine-dimension `assurance` envelope, and every dimension MUST name a producing
    /// > engine or carry a typed `Unsupported(reason)` (plan B11). Hiding uncertainty to
    /// > save tokens is prohibited.
    /// >
    /// > — `rule envelope.assurance_required`
    ///
    /// Absent for a `structural` or `policy` verdict, and for an operation with no verdict
    /// clause at all: the envelope is a statement about a *semantic* claim, and attaching
    /// nine dimensions to a snapshot creation would be nine claims nothing established.
    /// > engine or carry a typed `Unsupported(reason)` (plan B11).
    /// >
    /// > — `rule envelope.assurance_required`
    ///
    /// It sits on the family's own output rather than being filled in by [`result`]
    /// (super::result) because it is a statement about *what produced the verdict*, and only
    /// the family that ran the engine knows that. A family whose operations declare
    /// `structural` or `policy` verdicts leaves it [`Optional::Absent`], which is what
    /// [`Effect::new`] does; a family that emits a `semantic` verdict without one would
    /// break the rule above, which is why the field exists at all.
    pub assurance: Optional<AssuranceEnvelope>,
    /// Artifacts the call produced or named.
    pub artifacts: Vec<ArtifactRef>,
    /// The INV-007 omission manifest.
    pub omissions: Vec<Omission>,
    /// Typed warnings. Never interpolated prose (INV-016).
    pub warnings: Vec<Warning>,
}

impl Effect {
    /// A result carrying a payload and a verdict and nothing else.
    #[must_use]
    pub fn new(payload: Payload, verdict: Nullable<Verdict>) -> Self {
        Self {
            payload,
            completion: Completion::Answered {
                task: Optional::Absent,
            },
            verdict,
            assurance: Optional::Absent,
            artifacts: Vec::new(),
            omissions: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// The same result at `status = ok`, naming the task it is *about*.
    ///
    /// For the task-observing operations — `task.status`, `task.cancel`,
    /// `task.subscribe`, `task.update_budget`, `verification.result`,
    /// `verification.await` — which the IDL's `task` presence rule covers in its second
    /// clause.
    #[must_use]
    pub fn observing(mut self, task: TaskHandle) -> Self {
        self.completion = Completion::Answered {
            task: Optional::Present(task),
        };
        self
    }

    /// The same result at `status = task_started`.
    #[must_use]
    pub fn started(mut self, task: TaskHandle) -> Self {
        self.completion = Completion::TaskStarted { task };
        self
    }

    /// The same result at `status = task_suspended`, naming the continuation that
    /// resumes the parked task.
    #[must_use]
    pub fn suspended(mut self, task: TaskHandle, continuation: ContinuationHandle) -> Self {
        self.completion = Completion::TaskSuspended { task, continuation };
        self
    }

    /// The same result, naming the artifacts it produced.
    #[must_use]
    pub fn with_artifacts(mut self, artifacts: Vec<ArtifactRef>) -> Self {
        self.artifacts = artifacts;
        self
    }

    /// The same result, carrying the assurance envelope its semantic verdict obliges.
    #[must_use]
    pub fn with_assurance(mut self, assurance: AssuranceEnvelope) -> Self {
        self.assurance = Optional::Present(assurance);
        self
    }

    /// The same result, naming what it deliberately left out (INV-007).
    #[must_use]
    pub fn with_omissions(mut self, omissions: Vec<Omission>) -> Self {
        self.omissions = omissions;
        self
    }
}

/// What a failure offers a caller instead of a dead end.
///
/// > `BudgetExhausted` | resource | task-budget spend was exhausted; carries
/// > `continuation`, or a typed `non_resumable_reason` | no — resume with a larger budget
/// >
/// > — `notes/plan/rfcs/0026-continuumd-native-protocol.md`, §10.3 taxonomy
///
/// The `Error` struct has carried `continuation` and `non_resumable_reason` since 3.0 and
/// this daemon wrote `Absent` into both, so a `BudgetExhausted` reaching a caller was the
/// silent dead end SD-13's `oneOf` — "a `BudgetExhausted` failure carries a non-null
/// continuation or a typed `non_resumable_reason`" — exists to forbid. bn-23j7s makes the
/// choice a value the fault carries, so the two wire fields are written from something
/// rather than defaulted away.
///
/// Three arms, and the first is not a placeholder for the other two: most codes in the
/// §10.3 taxonomy are not about resumption at all, and offering a resumption question on a
/// `MalformedRequest` would invent one nobody asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resumption {
    /// This code says nothing about resuming, so the two wire fields stay absent.
    NotApplicable,
    /// Resume from this continuation. The `cont_*` artifact is already filed.
    From(ContinuationHandle),
    /// There is no continuation, and this is the typed reason there is none.
    ///
    /// `&'static str` for the reason [`Fault::detail`] is one: `rule envelope.no_prose`
    /// forbids interpolating source, logs, model text, or production payloads into a wire
    /// string, and a static string cannot carry any of them.
    NonResumable(&'static str),
}

/// One typed recovery offer: an operation, its pre-filled request body, and why it is offered.
///
/// > **H8 — a stale-handle failure is recoverable and says so.** Every row above carries a
/// > `recovery` list computed under N2, so "re-seal", "re-base", "re-page under the current
/// > epoch", or "resume under the pinned epoch" is executable rather than described.
/// >
/// > — RFC 0027, "Stale handles and stale capabilities"
///
/// This is the typed form of one `Error.recovery` entry (`NextOperation`). The wire field
/// `NextOperation.arguments` is `Opaque` — bytes of the *negotiated* encoding — so the
/// daemon layer, which is encoding-free, carries the typed [`Arguments`] here, and
/// [`crate::transport::Server::answer`] encodes them where the encoding is known. That is
/// the same seam [`ErrorData`] and [`Payload`] use.
///
/// The operation is not a separate field: it is `arguments.operation()`, so an offer whose
/// name and body disagree cannot be built.
///
/// The dispatcher filters every offer under RFC 0027 N2 before it reaches the envelope: an
/// offer the presenting capability would be denied is dropped, never shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryOffer {
    /// The pre-filled request body of the offered operation.
    pub arguments: Arguments,
    /// The typed reason, one of the `RATIONALE_*` tokens. Never interpolated (S1).
    pub rationale: &'static str,
}

/// The rationale of a *currency* refusal's offer: the named snapshot is no longer its
/// lineage's head, and the offered `workspace.seal` names the head that superseded it.
///
/// "Re-seal" in H8's own words. Sealing the head is the step every snapshot-pinned run
/// needs before it is re-issued against the head, and it is convergent when the head is
/// already sealed, so the offer is executable as given in both cases.
pub const RATIONALE_RESEAL_LINEAGE_HEAD: &str = "reseal_lineage_head";

/// The rationale of an *agreement* refusal's offer: the envelope's `snapshot` disagrees
/// with the snapshot the named artifact is stated against, and currency is not in
/// question.
///
/// The offer is the same request body, to re-issue with the envelope `snapshot` null or
/// equal to the artifact's own. Finding F6 of `tests/gate_g2_04_acceptance.rs` is why this
/// token exists: both predicates answer `StaleSnapshot`, and the rationale is what tells an
/// agent which one refused it.
pub const RATIONALE_REISSUE_WITHOUT_SNAPSHOT_PIN: &str = "reissue_without_snapshot_pin";

/// The typed `Error.data` value a fault carries, when its code declares a shape.
///
/// The wire field is `Opaque` — bytes of the *negotiated* encoding — so the daemon
/// layer, which is encoding-free, carries the typed value here and the transport
/// encodes it where the encoding is known, exactly as [`Payload`] travels beside the
/// envelope until [`crate::transport::Server::answer`] splices it in.
///
/// One variant per code that declares a shape, which as of protocol 3.4 is exactly
/// one: `rule encoding.opaque_payloads` — "for every code that declares no shape a
/// daemon MUST leave `data` absent rather than inventing one" — is why this is a
/// closed enum with a [`Self::None`] arm rather than an open bag of bytes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ErrorData {
    /// This code declares no `Error.data` shape; the wire field stays absent.
    #[default]
    None,
    /// The declared shape for `code = CertificateRejected` (RFC 0026 F19, 3.4):
    /// the rejecting kernel and its own reason token, relayed verbatim.
    ///
    /// Boxed so [`Fault`], which carries an `ErrorData`, stays under
    /// `clippy::result_large_err`'s 128-byte bound now that it also carries recovery
    /// offers (bn-27mx7).
    CertificateRejection(Box<CertificateRejection>),
}

impl ErrorData {
    /// Whether there is a typed value to encode.
    #[must_use]
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// A typed failure: the wire code, the stable explanation of it in context, and what the
/// caller can resume from.
///
/// `detail` is `&'static str` rather than `String` by construction, because
/// `rule envelope.no_prose` forbids interpolating source, logs, model text, or production
/// payloads into it and a static string cannot carry any of them. A caller needing the
/// specifics reads them from `code` and, where the code declares a shape, from
/// `Error.data` ([`ErrorData`], RFC 0026 F19).
///
/// Not [`Copy`] as of bn-23j7s: [`Resumption::From`] carries a handle, and a fault that
/// could be duplicated silently would be two answers to "where does this resume".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    /// The plan §10.3 code.
    pub code: ErrorCode,
    /// Stable, non-interpolated explanation of the code in context.
    pub detail: &'static str,
    /// Whether an identical retry can succeed without any change by the caller.
    pub retryable: bool,
    /// What the caller can resume from, for the codes RFC 0026 requires it of.
    pub resumption: Resumption,
    /// The typed `Error.data` specifics, for the codes that declare a shape.
    pub data: ErrorData,
    /// The typed `Error.recovery` offers, before the dispatcher's N2 filter.
    pub recovery: Vec<RecoveryOffer>,
    /// Whether this is the denial [`Call::derived`] raised for a derived handle outside the
    /// grant. Never on the wire: the envelope is the one `CapabilityDenied` (X1). The
    /// dispatcher reads it only to categorize the admission audit record
    /// ([`Denial::DerivedHandle`](super::state::Denial::DerivedHandle)).
    pub derived_denial: bool,
}

impl Fault {
    /// A failure whose `retryable` is its code's row in RFC 0026's error taxonomy
    /// ([`errors::retryable`]): true for `StatusConflict`, `QuotaExhausted`, and
    /// `PublicationAborted`, false for every other code.
    #[must_use]
    pub const fn new(code: ErrorCode, detail: &'static str) -> Self {
        Self {
            code,
            detail,
            retryable: errors::retryable(code),
            resumption: Resumption::NotApplicable,
            data: ErrorData::None,
            recovery: Vec::new(),
            derived_denial: false,
        }
    }

    /// The same fault, carrying typed recovery offers (RFC 0027 H8).
    ///
    /// The dispatcher filters them under N2 before any reaches the envelope.
    #[must_use]
    pub fn with_recovery(mut self, recovery: Vec<RecoveryOffer>) -> Self {
        self.recovery = recovery;
        self
    }

    /// The same fault, marked as one an identical retry cannot clear.
    ///
    /// RFC 0026 makes the per-occurrence value the authority: "the authoritative
    /// per-occurrence value is the **required** `Error.retryable` field". [`Self::new`]
    /// takes the code's taxonomy row as the default. A site whose failure is deterministic —
    /// the same request against the same state fails the same way — overrides it here. The
    /// idempotency ledger reads this occurrence value, so such a failure binds its key
    /// (`Daemon::dispatch` step 7).
    #[must_use]
    pub const fn not_retryable(mut self) -> Self {
        self.retryable = false;
        self
    }

    /// The same fault, carrying the typed `Error.data` value its code declares.
    #[must_use]
    pub fn with_data(mut self, data: ErrorData) -> Self {
        self.data = data;
        self
    }

    /// A `BudgetExhausted` with no continuation, whose typed `non_resumable_reason` is its
    /// own detail.
    ///
    /// The constructor exists so that the SD-13 obligation is discharged where the code is
    /// *chosen* rather than remembered at the envelope: a `BudgetExhausted` built by
    /// [`Self::new`] would reach `result::failure` with both fields absent, and
    /// `tests/pr6_impl02_budget_evidence.rs` holds every one this daemon can raise to
    /// carrying one of the two.
    ///
    /// One string, deliberately. [`region::NO_CONTINUATION`](super::region::NO_CONTINUATION)
    /// already records the design — RFC 0026's `non_resumable_reason` "is a *wire* field of
    /// the task record and is written from the daemon's own fault detail" — so the reason a
    /// `task.status` reader sees and the reason the failing call's envelope carries are the
    /// same value and cannot drift apart. A condition whose reason is genuinely not its
    /// detail builds [`Resumption::NonResumable`] directly.
    #[must_use]
    pub const fn exhausted(detail: &'static str) -> Self {
        Self {
            code: ErrorCode::BudgetExhausted,
            detail,
            retryable: errors::retryable(ErrorCode::BudgetExhausted),
            resumption: Resumption::NonResumable(detail),
            data: ErrorData::None,
            recovery: Vec::new(),
            derived_denial: false,
        }
    }

    /// A `BudgetExhausted` the caller can resume from.
    #[must_use]
    pub fn exhausted_from(detail: &'static str, continuation: ContinuationHandle) -> Self {
        Self {
            code: ErrorCode::BudgetExhausted,
            detail,
            retryable: errors::retryable(ErrorCode::BudgetExhausted),
            resumption: Resumption::From(continuation),
            data: ErrorData::None,
            recovery: Vec::new(),
            derived_denial: false,
        }
    }

    /// The one answer to every authorization failure a *handler* reaches.
    ///
    /// A family raises this where a request names something the daemon does not hold. It
    /// carries the same code, the same constant `detail`, and the same `retryable` as the
    /// denial [`admission`](super::admission) produces, so a read of an artifact that does
    /// not exist and a read of one that exists but is out of scope return byte-identical
    /// envelopes — RFC 0027 X2, which is the rule a distinct not-found would break.
    #[must_use]
    pub const fn denied() -> Self {
        Self::new(ErrorCode::CapabilityDenied, super::result::DENIAL_DETAIL)
    }

    /// Whether this code is one the operation is allowed to return.
    ///
    /// > An operation's `errors` clause lists the codes it may return beyond these. A
    /// > daemon MUST NOT return a code outside that union for the operation.
    /// >
    /// > — `rule errors.common`
    #[must_use]
    pub fn admissible_for(&self, spec: &OperationSpec) -> bool {
        errors::admits(spec, self.code)
    }
}

/// One IDL namespace's operations.
///
/// Object-safe on purpose: the daemon holds `Box<dyn OperationFamily>` so a deployment —
/// and a test — can register exactly the families it wants and get a typed
/// `UnsupportedSemanticFeature` for the rest, which is what `rule
/// errors.unsupported_surface` requires of an operation registered ahead of its subsystem.
pub trait OperationFamily {
    /// The IDL namespace this family answers for: the part of `namespace.verb` before the
    /// dot.
    fn namespace(&self) -> &'static str;

    /// The instances and classes these arguments name, for RFC 0027 T2.
    ///
    /// Pure in `arguments`. It is called before admission, so it must not consult state.
    fn scope(&self, arguments: &Arguments) -> ScopeClaim;

    /// Run the operation.
    ///
    /// # Errors
    ///
    /// A [`Fault`] whose code is inside `rule errors.common` ∪ the operation's `errors`
    /// clause. Returning one outside that union is a defect the dispatcher detects.
    fn handle(
        &self,
        call: &Call<'_>,
        state: &mut DaemonState,
        services: &Services,
        store: &ReferenceStore,
    ) -> Result<Effect, Fault>;
}
