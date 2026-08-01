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
//! - the operation is one of the registry's 72 and `Call::spec` is its entry;
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
    ArtifactRef, AssuranceEnvelope, Omission, RequestEnvelope, Verdict, Warning,
};
use crate::protocol::handshake::CapabilityDescriptor;
use crate::protocol::operations::evidence::{
    EvidenceGetRequest, EvidenceGetResponse, EvidenceQueryRequest, EvidenceQueryResponse,
    EvidenceSubscribeRequest, EvidenceSubscribeResponse, EvidenceVerifyRequest,
    EvidenceVerifyResponse,
};
use crate::protocol::operations::intent::{
    IntentAcceptRequest, IntentAcceptResponse, IntentDiffRequest, IntentDiffResponse,
    IntentGetRequest, IntentGetResponse, IntentLockRequest, IntentLockResponse,
    IntentProposeRevisionRequest, IntentProposeRevisionResponse, IntentRejectRequest,
    IntentRejectResponse,
};
use crate::protocol::operations::observe::{
    ObserveClassifyRequest, ObserveClassifyResponse, ObserveIngestRequest, ObserveIngestResponse,
    ObserveResultRequest,
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
use crate::protocol::operations::workspace::{
    WorkspaceCreateRequest, WorkspaceCreateResponse, WorkspaceDiffRequest, WorkspaceDiffResponse,
    WorkspaceForkRequest, WorkspaceForkResponse, WorkspaceSealRequest, WorkspaceSealResponse,
};
use crate::protocol::scalar::{AuditCorrelationId, IntentHandle, WorkspaceHandle};
use crate::protocol::shared::VerificationResult;
use crate::protocol::spec::{Nullable, OperationSpec, Optional};
use crate::protocol::task::TaskRecord;
use crate::protocol::vocabulary::ErrorCode;

/// A decoded operation request body.
///
/// The envelope's `arguments` field is declared `Opaque`, and this layer deliberately does
/// not decode it: RFC 0026 fixes two encodings but neither their canonical field order nor
/// their union tagging, and the IDL's own open item 3 leaves the envelope's `Opaque`
/// payloads without a declared shape. So the operation layer takes the request *already
/// decoded* and the transport half of PR 5 supplies the decoder. Everything the wire *did*
/// fix — the field names, types, and three-valued presence of each body — is still enforced,
/// because each variant carries the IDL's own struct.
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
        }
    }
}

/// A typed operation response body.
///
/// Carried beside the [`ResultEnvelope`](crate::protocol::envelope::ResultEnvelope) rather
/// than inside its `payload` field for the reason [`Arguments`] gives: there is no codec at
/// this layer, so the envelope's `payload` reads `null` and this value is what the transport
/// encodes into it. `tests/daemon_operations.rs` pins that invariant so the two can never
/// drift into disagreeing.
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
    /// Artifact classes the request touches, as plan §4.4 prefixes without the underscore.
    pub classes: Vec<&'static str>,
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
}

/// What a family produced on success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effect {
    /// The operation's response body.
    pub payload: Payload,
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
            verdict,
            assurance: Optional::Absent,
            artifacts: Vec::new(),
            omissions: Vec::new(),
            warnings: Vec::new(),
        }
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

/// A typed failure: the wire code and the stable explanation of it in context.
///
/// `detail` is `&'static str` rather than `String` by construction, because
/// `rule envelope.no_prose` forbids interpolating source, logs, model text, or production
/// payloads into it and a static string cannot carry any of them. A caller needing the
/// specifics reads them from `code` and, once the codec lands, from `Error.data`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fault {
    /// The plan §10.3 code.
    pub code: ErrorCode,
    /// Stable, non-interpolated explanation of the code in context.
    pub detail: &'static str,
    /// Whether an identical retry can succeed without any change by the caller.
    pub retryable: bool,
}

impl Fault {
    /// A non-retryable failure. Every fault this bone's families raise is non-retryable:
    /// each names something the caller must change.
    #[must_use]
    pub const fn new(code: ErrorCode, detail: &'static str) -> Self {
        Self {
            code,
            detail,
            retryable: false,
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
