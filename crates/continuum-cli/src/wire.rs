//! The byte boundary a command speaks across, and the typed answer it gets back.
//!
//! # Why a command does not own a daemon
//!
//! A command writes a request frame and reads a result frame; it does not know what is on
//! the other side of [`Transport`], which is the whole point — plan §20's "adapters do not
//! own semantic state" as a property of this module's types rather than a promise in a
//! comment. [`Connection`] holds only the negotiated protocol version and the principal it
//! speaks as, the same two facts `continuum_mcp::AgentClient` holds and no others.
//!
//! # What is not here, and why
//!
//! There is no handshake (`ClientHello`/`ServerWelcome`) and no version negotiation.
//! [`Connection::new`] takes an already-negotiated [`ProtocolVersion`] because the one place
//! negotiation would have a home — a real transport dialing a real deployment — does not
//! exist in this workspace yet (see the crate root's "What 'over the wire' means here,
//! honestly"). Adding a handshake with nothing on the other end to negotiate against would
//! be inventing the deployment story rather than delivering the command surface this bone
//! owes.
//!
//! # Two outcome shapes, and why there are two
//!
//! `task.status`/`task.resume`/`task.cancel` and `evidence.get` decode through
//! `continuumd`'s own [`Arguments`]/[`Payload`] tables, so [`Connection::task_status`] and
//! its siblings return [`Outcome<Payload>`]. [`Connection::context_expand`],
//! [`Connection::debug_open`], and the three other typed-body calls return
//! `Outcome<TheResponseStruct>` — the *typed body*, not the enum — because a command that
//! has already chosen its operation has nothing to gain from re-matching a
//! twenty-eight-variant enum to find the shape it asked for, and because for four of them
//! *there is no variant to match*: `debug.*` and `repair.*` have request and response
//! structs in `continuumd::protocol::operations` and rows in the registry, and no arm in
//! [`Arguments`]. They are built and decoded against those real structs with the same
//! `codec::to_opaque`/`from_opaque` pair `continuumd::transport::encode_request` uses per
//! `Arguments` variant — see [`Connection::invoke_typed`]. Both outcomes share one
//! [`Refusal`] shape, because a daemon's typed refusal is the same three fields (`code`,
//! `detail`, `retryable`) plus the two resumability fields whichever operation it answers.
//!
//! # A surface this daemon does not serve is still a real round trip
//!
//! Nothing here short-circuits an operation the deployment cannot answer. A `debug.open`
//! frame is encoded, written, and read exactly as a `task.status` frame is; what comes back
//! is `codec::operations::decode_arguments`'s own refusal
//! (`CodecError::UnknownOperation` ⇒ [`ErrorCode::UnsupportedSemanticFeature`], `rule
//! errors.unsupported_surface`), rendered faithfully. That is the difference between
//! reporting what the daemon said and guessing on its behalf: the day a `debug` family is
//! registered, the same call is admitted and no line in this module changes. It is the same
//! reading bn-3tz60 gave `context.expand` before bn-28jj served it.

use core::fmt;

use continuumd::codec::{self, CodecError, ProtocolValue};
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::is_mutation;
use continuumd::protocol::envelope::{
    ArtifactRef, Budget, Cost, NextOperation, Omission, RequestEnvelope, ResultEnvelope,
};
use continuumd::protocol::operations::context::{ContextExpandRequest, ContextExpandResponse};
use continuumd::protocol::operations::debug::{
    DebugOpenRequest, DebugOpenResponse, DebugStateRequest, DebugStateResponse,
};
use continuumd::protocol::operations::evidence::EvidenceGetRequest;
use continuumd::protocol::operations::repair::{
    RepairBeginRequest, RepairBeginResponse, RepairReviewRequest, RepairReviewResponse,
};
use continuumd::protocol::operations::task::{
    TaskCancelRequest, TaskResumeRequest, TaskStatusRequest,
};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, ContinuationHandle, Opaque, OperationName, ProtocolVersion,
    RequestId, TaskHandle,
};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{ErrorCode, ResultStatus};
use continuumd::transport::{self, FrameError, LocalPair, Server, TransportError};

/// A byte boundary: send a request frame, get a result frame back.
///
/// One method, because that is the whole of what a command needs from a connection once a
/// version is already negotiated (see this module's doc for why negotiation is not this
/// trait's job).
pub trait Transport {
    /// Send `frame` and return the frame that came back.
    ///
    /// # Errors
    ///
    /// [`LinkError`] when the frame cannot be written, or no answer arrives.
    fn exchange(&mut self, frame: &[u8]) -> Result<Vec<u8>, LinkError>;
}

/// Why an exchange failed below the protocol — never a refusal, which is a typed
/// [`ResultEnvelope`] with `status = error` and arrives as an ordinary answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// The frame could not be written or read.
    Frame(FrameError),
    /// The transport failed below the frame layer.
    Transport(TransportError),
    /// The server served the request but wrote no answer.
    NoAnswer,
    /// [`NullTransport`]'s one answer: there is nothing on the other end. See the crate
    /// root doc for why that is the honest state of this workspace today.
    NoTransportConfigured,
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frame(error) => write!(f, "{error}"),
            Self::Transport(error) => write!(f, "{error}"),
            Self::NoAnswer => f.write_str("the server wrote no answer frame"),
            Self::NoTransportConfigured => f.write_str(
                "no daemon transport is configured: continuumd ships no socket or process \
                 transport yet, so this binary has no deployment to dial",
            ),
        }
    }
}

impl core::error::Error for LinkError {}

impl From<FrameError> for LinkError {
    fn from(error: FrameError) -> Self {
        Self::Frame(error)
    }
}

impl From<TransportError> for LinkError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}

/// The in-process byte boundary: a connected [`LocalPair`] and the [`Server`] behind it.
///
/// This is the only [`Transport`] this workspace can build today, because `continuumd`
/// ships no socket or process transport (see the crate root doc). It is what
/// `continuum-mcp`'s `LocalLink` and `continuum-benchmark`'s `Rig::boundary` are for their
/// callers, borrowed rather than owned for the same reason theirs are: provisioning a
/// daemon is administration (IDL §7), not this module's job.
#[derive(Debug)]
pub struct LocalLink<'a> {
    server: &'a mut Server,
    pair: &'a mut LocalPair,
}

impl<'a> LocalLink<'a> {
    /// Borrow a server and a connected pair as one byte boundary.
    pub const fn new(server: &'a mut Server, pair: &'a mut LocalPair) -> Self {
        Self { server, pair }
    }
}

impl Transport for LocalLink<'_> {
    fn exchange(&mut self, frame: &[u8]) -> Result<Vec<u8>, LinkError> {
        self.pair.to_server.put_frame(frame)?;
        self.server.serve(self.pair)?;
        self.pair.to_client.take_frame()?.ok_or(LinkError::NoAnswer)
    }
}

/// The transport `bin/continuum.rs` runs against today: nothing.
///
/// Every call answers [`LinkError::NoTransportConfigured`] — a typed, honest refusal to
/// pretend a deployment exists, rather than a fabricated in-process daemon that would
/// forget every task the moment the process exited. See the crate root doc, "What 'over the
/// wire' means here, honestly".
#[derive(Debug, Clone, Copy, Default)]
pub struct NullTransport;

impl Transport for NullTransport {
    fn exchange(&mut self, _frame: &[u8]) -> Result<Vec<u8>, LinkError> {
        Err(LinkError::NoTransportConfigured)
    }
}

/// Why a call could not be completed — a wire or codec failure, never a refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectError {
    /// A request could not be encoded, or an answer could not be decoded.
    Codec(CodecError),
    /// The byte boundary failed.
    Link(LinkError),
    /// The daemon answered `status = error` with no `error` object, or a non-error status
    /// with one. The envelope contradicts itself and this connection will not guess which
    /// half to believe.
    StatusMismatch {
        /// The status the envelope declared.
        status: ResultStatus,
        /// Whether an `error` object was present.
        error_present: bool,
    },
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(error) => write!(f, "the daemon's answer did not decode: {error}"),
            Self::Link(error) => write!(f, "the connection failed: {error}"),
            Self::StatusMismatch {
                status,
                error_present,
            } => write!(
                f,
                "the daemon's answer contradicts itself: status={status:?}, error present={error_present}"
            ),
        }
    }
}

impl core::error::Error for ConnectError {}

impl From<CodecError> for ConnectError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

impl From<LinkError> for ConnectError {
    fn from(error: LinkError) -> Self {
        Self::Link(error)
    }
}

/// One typed answer: either the daemon ran the operation, or it answered a typed refusal.
///
/// There is no third, "the client refused locally" arm here (unlike
/// `continuum_mcp::Outcome::Unmet`): this crate carries no semantic-action grammar of its
/// own to refuse *with* — every call this connection makes reaches the wire, honestly,
/// which is what lets a refusal always be the daemon's own typed answer rather than a mix
/// of two refusal sources under one rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome<T> {
    /// The daemon ran the operation.
    Admitted(Admitted<T>),
    /// The daemon answered a typed [`ErrorCode`].
    Refused(Refusal),
}

/// An admitted answer: the typed payload, and the envelope facts a command renders.
///
/// Every field is read off a decoded [`ResultEnvelope`]; none is inferred. `omissions` is
/// always present — INV-007's "empty list means nothing was omitted; the field is never
/// absent" — so a caller can render it unconditionally rather than branching on whether it
/// exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admitted<T> {
    /// The request identifier the daemon echoed.
    pub request_id: RequestId,
    /// `ok`, `task_started`, or `task_suspended`.
    pub status: ResultStatus,
    /// The operation's typed response body.
    pub payload: T,
    /// The task this call started or observes, when it names one.
    pub task: Option<TaskHandle>,
    /// The continuation a suspension parked, when there is one.
    pub continuation: Option<ContinuationHandle>,
    /// The INV-007 omission manifest, unabridged. Empty means nothing was omitted.
    pub omissions: Vec<Omission>,
    /// Artifacts this call published or names.
    pub artifacts: Vec<ArtifactRef>,
    /// Actual spend, in the nine budget dimensions.
    pub cost: Cost,
    /// The allowed next operations the daemon offered, with pre-filled arguments.
    pub next_operations: Vec<NextOperation>,
}

/// A daemon's typed refusal.
///
/// `detail` is carried because RFC 0026 declares it `required`, and deliberately not
/// *branched on* by any renderer in this crate: an agent that read the sentence instead of
/// [`Refusal::code`] would be scraping prose through a typed hole, which is the failure
/// INV-003 names. It is still printed — a human reads it — but no `match` in this crate
/// compares it to a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The request identifier the daemon echoed.
    pub request_id: RequestId,
    /// The closed error vocabulary member — the typed reason a caller branches on.
    pub code: ErrorCode,
    /// The stable, non-interpolated explanation of the code in context.
    pub detail: String,
    /// Whether an identical retry can succeed with no change by the caller.
    pub retryable: bool,
    /// The allowed operations from this state. The protocol's only recovery channel.
    pub recovery: Vec<NextOperation>,
    /// Present when the failure is resumable.
    pub continuation: Option<ContinuationHandle>,
    /// Present when a budget exhaustion is *not* resumable: the typed reason.
    pub non_resumable_reason: Option<String>,
}

/// The negotiated connection a command's wire calls travel over.
///
/// Holds exactly two facts: the protocol version already negotiated, and the principal
/// this connection speaks as. No snapshot, no task, no continuation — plan §20's "adapters
/// do not own semantic state" as the struct's field list.
#[derive(Debug, Clone)]
pub struct Connection {
    version: ProtocolVersion,
    actor: ActorId,
    capability: CapabilityHandle,
    calls: u64,
}

impl Connection {
    /// A connection speaking `version` as `actor` under `capability`.
    #[must_use]
    pub const fn new(
        version: ProtocolVersion,
        actor: ActorId,
        capability: CapabilityHandle,
    ) -> Self {
        Self {
            version,
            actor,
            capability,
            calls: 0,
        }
    }

    fn request_id(&mut self) -> RequestId {
        self.calls += 1;
        RequestId::new(&format!("req_cli{:06}", self.calls))
            .expect("a counter renders a well-formed request id")
    }

    fn envelope(&mut self, operation: &'static str, budget: Optional<Budget>) -> RequestEnvelope {
        let idempotency_key = if is_mutation(operation) {
            Optional::Present(format!("cli-idem-{:06}", self.calls + 1))
        } else {
            Optional::Absent
        };
        RequestEnvelope {
            protocol_version: self.version,
            request_id: self.request_id(),
            idempotency_key,
            actor: self.actor.clone(),
            capability: self.capability.clone(),
            operation: OperationName::new(operation).expect("a registry name is well formed"),
            snapshot: Nullable::Null,
            intent: Nullable::Null,
            arguments: Opaque::from_bytes(Vec::new()),
            budget,
            output_policy: Optional::Absent,
            trace: Optional::Absent,
            page: Optional::Absent,
        }
    }

    /// `task.status` — read a task record.
    ///
    /// # Errors
    ///
    /// [`ConnectError`] when the request cannot be encoded, the wire fails, or the answer
    /// is not the shape the operation declares.
    pub fn task_status(
        &mut self,
        transport: &mut dyn Transport,
        task: &TaskHandle,
    ) -> Result<Outcome<Payload>, ConnectError> {
        let arguments = Arguments::TaskStatus(TaskStatusRequest { task: task.clone() });
        self.invoke(transport, &arguments, Optional::Absent)
    }

    /// `task.cancel` — close a campaign, keeping whatever it committed (PR-6's
    /// `rule task.cancel_correct`).
    ///
    /// # Errors
    ///
    /// [`ConnectError`] as [`Connection::task_status`].
    pub fn task_cancel(
        &mut self,
        transport: &mut dyn Transport,
        task: &TaskHandle,
    ) -> Result<Outcome<Payload>, ConnectError> {
        let arguments = Arguments::TaskCancel(TaskCancelRequest { task: task.clone() });
        self.invoke(transport, &arguments, Optional::Absent)
    }

    /// `task.resume` — spend a continuation under a declared budget.
    ///
    /// `task.resume` is `@task_starting`, so RFC 0026 requires the envelope's own `budget`
    /// whenever the body's is present; both are set from `budget` here, mirroring
    /// `continuum_mcp::AgentClient::task_resume`'s identical reading of the same rule.
    ///
    /// # Errors
    ///
    /// [`ConnectError`] as [`Connection::task_status`].
    pub fn task_resume(
        &mut self,
        transport: &mut dyn Transport,
        continuation: &ContinuationHandle,
        budget: Budget,
    ) -> Result<Outcome<Payload>, ConnectError> {
        let arguments = Arguments::TaskResume(TaskResumeRequest {
            continuation: continuation.clone(),
            budget: Optional::Present(budget.clone()),
        });
        self.invoke(transport, &arguments, Optional::Present(budget))
    }

    fn invoke(
        &mut self,
        transport: &mut dyn Transport,
        arguments: &Arguments,
        budget: Optional<Budget>,
    ) -> Result<Outcome<Payload>, ConnectError> {
        let envelope = self.envelope(arguments.operation(), budget);
        let frame = transport::encode_request(&envelope, arguments)?;
        let answer = transport.exchange(&frame)?;
        let (result, payload) = transport::decode_result(arguments.operation(), &answer)?;
        classify(result, payload)
    }

    /// `evidence.get` — read one PR-7 evidence-graph node or edge by handle.
    ///
    /// Goes through the [`Arguments`]/[`Payload`] tables rather than
    /// [`Connection::invoke_typed`], because `evidence.get` has an arm in both and a call
    /// that has one has no reason to hand-roll the encoding — the same split
    /// [`Connection::task_status`] is on, and for the same reason.
    ///
    /// # Errors
    ///
    /// [`ConnectError`] as [`Connection::task_status`].
    pub fn evidence_get(
        &mut self,
        transport: &mut dyn Transport,
        request: &EvidenceGetRequest,
    ) -> Result<Outcome<Payload>, ConnectError> {
        let arguments = Arguments::EvidenceGet(request.clone());
        self.invoke(transport, &arguments, Optional::Absent)
    }

    /// `context.expand` — follow a PR-11 expansion handle along a relation.
    ///
    /// `context.expand` is `@mutation` and `@task_starting` per the registry, so the
    /// envelope always carries an idempotency key and a budget; both are read now that the
    /// family has landed (bn-28jj), by the dispatcher's obligation step and by the
    /// expansion's own byte ceiling respectively, where before bn-28jj the call was refused
    /// at `codec::operations::decode_arguments` before either was consulted.
    ///
    /// # Errors
    ///
    /// [`ConnectError`] as [`Connection::task_status`].
    pub fn context_expand(
        &mut self,
        transport: &mut dyn Transport,
        request: &ContextExpandRequest,
        states: Optional<u64>,
    ) -> Result<Outcome<ContextExpandResponse>, ConnectError> {
        let budget = Optional::Present(Budget {
            wall_ms: Optional::Absent,
            cpu_ms: Optional::Absent,
            memory_bytes: Optional::Absent,
            states,
            solver_ms: Optional::Absent,
            proof_ms: Optional::Absent,
            tokens: Optional::Absent,
            candidates: Optional::Absent,
            bytes: Optional::Absent,
        });
        self.invoke_typed(transport, "context.expand", request, budget)
    }

    /// `debug.open` — open a PR-19 causal-debugger branch on an explicit failure artifact.
    ///
    /// `@mutation`, so the envelope carries an idempotency key; not `@task_starting`, so it
    /// carries no budget (the registry's annotations decide, through
    /// [`Connection::envelope`]).
    ///
    /// # Errors
    ///
    /// [`ConnectError`] as [`Connection::task_status`].
    pub fn debug_open(
        &mut self,
        transport: &mut dyn Transport,
        request: &DebugOpenRequest,
    ) -> Result<Outcome<DebugOpenResponse>, ConnectError> {
        self.invoke_typed(transport, "debug.open", request, Optional::Absent)
    }

    /// `debug.state` — the state view at an explicit branch's frontier.
    ///
    /// The branch is a request field and never a remembered cursor: this connection holds
    /// no `dbg_*` of its own, which is INV-002 as the struct's field list rather than as a
    /// promise (see [`Connection`]).
    ///
    /// # Errors
    ///
    /// [`ConnectError`] as [`Connection::task_status`].
    pub fn debug_state(
        &mut self,
        transport: &mut dyn Transport,
        request: &DebugStateRequest,
    ) -> Result<Outcome<DebugStateResponse>, ConnectError> {
        self.invoke_typed(transport, "debug.state", request, Optional::Absent)
    }

    /// `repair.begin` — open a PR-20 repair transaction against an explicit failure.
    ///
    /// # Errors
    ///
    /// [`ConnectError`] as [`Connection::task_status`].
    pub fn repair_begin(
        &mut self,
        transport: &mut dyn Transport,
        request: &RepairBeginRequest,
    ) -> Result<Outcome<RepairBeginResponse>, ConnectError> {
        self.invoke_typed(transport, "repair.begin", request, Optional::Absent)
    }

    /// `repair.review` — the reviewer projection of an explicit repair transaction.
    ///
    /// # Errors
    ///
    /// [`ConnectError`] as [`Connection::task_status`].
    pub fn repair_review(
        &mut self,
        transport: &mut dyn Transport,
        request: &RepairReviewRequest,
    ) -> Result<Outcome<RepairReviewResponse>, ConnectError> {
        self.invoke_typed(transport, "repair.review", request, Optional::Absent)
    }

    /// One round trip against an operation's own request and response structs.
    ///
    /// Encodes `request` into the envelope's `arguments` opaque field with
    /// `continuumd::codec::to_opaque` and decodes the answer's payload as `Res` — see this
    /// module's doc for why the typed body rather than the [`Payload`] enum, and for why
    /// this path exists at all for the four `debug`/`repair` operations that have no
    /// [`Arguments`] arm to travel through.
    ///
    /// The envelope is built by [`Connection::envelope`] from the operation's *registry*
    /// annotations, so an idempotency key travels exactly when the registry says
    /// `@mutation` and nothing here restates that table.
    ///
    /// # Errors
    ///
    /// [`ConnectError`] as [`Connection::task_status`].
    fn invoke_typed<Req: ProtocolValue, Res: ProtocolValue>(
        &mut self,
        transport: &mut dyn Transport,
        operation: &'static str,
        request: &Req,
        budget: Optional<Budget>,
    ) -> Result<Outcome<Res>, ConnectError> {
        let mut envelope = self.envelope(operation, budget);
        envelope.arguments = codec::to_opaque(request)?;
        let frame = codec::to_bytes(&envelope)?;
        let answer = transport.exchange(&frame)?;
        let result: ResultEnvelope = codec::from_bytes(&answer)?;
        let payload = match &result.payload {
            Nullable::Null => None,
            Nullable::Value(opaque) => Some(codec::from_opaque::<Res>(opaque)?),
        };
        classify_typed(result, payload)
    }
}

/// Turn a decoded task-operation result into the typed outcome.
///
/// The one thing this refuses to do is guess: `status = error` with no `error` object, or
/// any other status with one, is a self-contradicting envelope, and picking a half to
/// believe would let a malformed answer become a semantic one — the same reading
/// `continuum_mcp::client::classify` gives the identical shape.
fn classify(result: ResultEnvelope, payload: Payload) -> Result<Outcome<Payload>, ConnectError> {
    let is_error = result.status == ResultStatus::Error;
    match (is_error, result.error) {
        (true, Optional::Present(error)) => Ok(Outcome::Refused(Refusal {
            request_id: result.request_id,
            code: error.code,
            detail: error.detail,
            retryable: error.retryable,
            recovery: error.recovery,
            continuation: error.continuation.value().cloned(),
            non_resumable_reason: error.non_resumable_reason.value().cloned(),
        })),
        (false, Optional::Absent) => Ok(Outcome::Admitted(Admitted {
            request_id: result.request_id,
            status: result.status,
            payload,
            task: result.task.value().cloned(),
            continuation: result.continuation.value().cloned(),
            omissions: result.omissions,
            artifacts: result.artifacts,
            cost: result.cost,
            next_operations: result.next_operations,
        })),
        (status, error) => Err(ConnectError::StatusMismatch {
            status: if status {
                ResultStatus::Error
            } else {
                result.status
            },
            error_present: !error.is_absent(),
        }),
    }
}

/// [`classify`], for a hand-decoded typed payload ([`Connection::invoke_typed`]).
///
/// `payload` is `None` on every refusal and `Some` on a success; the two envelope halves
/// are still cross-checked exactly as [`classify`] checks them, so a malformed answer is a
/// [`ConnectError`] on this path too. Generic rather than one function per operation
/// because the check is about the *envelope*, which is the same document whichever body it
/// carries — a per-operation copy would be five chances to check it four ways.
fn classify_typed<T>(
    result: ResultEnvelope,
    payload: Option<T>,
) -> Result<Outcome<T>, ConnectError> {
    let is_error = result.status == ResultStatus::Error;
    match (is_error, result.error, payload) {
        (true, Optional::Present(error), None) => Ok(Outcome::Refused(Refusal {
            request_id: result.request_id,
            code: error.code,
            detail: error.detail,
            retryable: error.retryable,
            recovery: error.recovery,
            continuation: error.continuation.value().cloned(),
            non_resumable_reason: error.non_resumable_reason.value().cloned(),
        })),
        (false, Optional::Absent, Some(payload)) => Ok(Outcome::Admitted(Admitted {
            request_id: result.request_id,
            status: result.status,
            payload,
            task: result.task.value().cloned(),
            continuation: result.continuation.value().cloned(),
            omissions: result.omissions,
            artifacts: result.artifacts,
            cost: result.cost,
            next_operations: result.next_operations,
        })),
        (status, error, _) => Err(ConnectError::StatusMismatch {
            status: if status {
                ResultStatus::Error
            } else {
                result.status
            },
            error_present: !error.is_absent(),
        }),
    }
}
