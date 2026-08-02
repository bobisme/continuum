//! The client itself: eight typed operations, one typed lower edge, no prose.
//!
//! # The shape of a call
//!
//! Every operation funnels through [`AgentClient::invoke`], which does five things and no
//! others:
//!
//! 1. asks [`register::admits`] whether the operation's preconditions hold in the
//!    **caller's** state, and refuses locally — spending no bytes — when they do not;
//! 2. mints a request identifier and, for a `@mutation`, an idempotency key, both from the
//!    monotone call counter, so a script's bytes are a function of the script (INV-005);
//! 3. builds the [`RequestEnvelope`] and encodes it with `continuumd`'s own
//!    [`encode_request`], which is the same function the daemon's transport tests use;
//! 4. exchanges frames across a [`Transport`], counting both directions;
//! 5. decodes the result frame into a typed [`Answer`].
//!
//! Nothing between those five steps interprets a string. The `detail` on a refusal and the
//! `detail` on a warning are carried and never branched on — an agent that read them would
//! be scraping prose through a typed hole, which is the failure INV-003 names.
//!
//! # Why `invoke` is public beside the eight named operations
//!
//! The named operations are the agent-facing surface: `verification_start` takes a snapshot,
//! a target and a state ceiling, not a hand-built envelope. `invoke` is the same surface one
//! layer down, taking `continuumd`'s closed [`Arguments`] enum, and it is public for two
//! reasons that are not convenience. An orchestrator that has *synthesized* a plan from
//! [`register::OPERATIONS`] holds operations as data and needs to execute them as data. And
//! the daemon's own recovery channel — `Error.recovery`, a list of [`NextOperation`] with
//! pre-filled arguments — is data too: a client that could only call named methods could
//! not execute the recovery the protocol handed it.
//!
//! # Authority
//!
//! The principal — actor plus capability — travels in every envelope and is checked below
//! this layer, independently of handle possession (RFC 0027, G1). [`AgentClient::speak_as`]
//! changes which principal a client speaks as; it cannot change what that principal may do.
//! INV-015 is a property of the daemon's capability table, and nothing here can widen it.
//!
//! [`encode_request`]: continuumd::transport::encode_request
//! [`Arguments`]: continuumd::daemon::family::Arguments
//! [`NextOperation`]: continuumd::protocol::envelope::NextOperation

use continuumd::codec::from_bytes;
use continuumd::daemon::family::Arguments;
use continuumd::daemon::is_mutation;
use continuumd::protocol::envelope::{Budget, RequestEnvelope, ResultEnvelope};
use continuumd::protocol::handshake::{ClientHello, ServerWelcome};
use continuumd::protocol::operations::task::{
    TaskCancelRequest, TaskResumeRequest, TaskStatusRequest,
};
use continuumd::protocol::operations::verification::{
    VerificationResultRequest, VerificationStartRequest,
};
use continuumd::protocol::operations::workspace::{
    WorkspaceCreateRequest, WorkspaceForkRequest, WorkspaceSealRequest,
};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, ContinuationHandle, IntentHandle, Opaque, OperationName,
    ProtocolVersion, RequestId, TaskHandle, WorkspaceHandle,
};
use continuumd::protocol::shared::{FileOverlay, SnapshotComponents, Target};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{Portfolio, ResultStatus};
use continuumd::transport::{decode_result, encode_hello, encode_request};

use crate::answer::{Admitted, Answer, ByteLedger, CallBytes, ClientError, Outcome, Refusal};
use crate::link::Transport;
use crate::register::{self, AgentContext};

/// One typed call: the arguments, plus the three envelope fields that are not arguments.
///
/// `snapshot`, `intent` and `budget` live on the envelope rather than in any operation's
/// request struct (RFC 0026), so a call is not fully described by its arguments alone. This
/// struct is that description, and it is borrowed rather than owned because a caller
/// building one already holds every part.
#[derive(Debug)]
pub struct Call<'a> {
    /// The operation's typed request body.
    pub arguments: &'a Arguments,
    /// The snapshot this call is scoped to, when it has one.
    pub snapshot: Option<&'a WorkspaceHandle>,
    /// The governing intent, when the operation names one.
    pub intent: Option<&'a IntentHandle>,
    /// The declared ceiling, required for a `@task_starting` operation.
    pub budget: Option<Budget>,
}

impl<'a> Call<'a> {
    /// A call carrying only its arguments.
    #[must_use]
    pub const fn new(arguments: &'a Arguments) -> Self {
        Self {
            arguments,
            snapshot: None,
            intent: None,
            budget: None,
        }
    }

    /// The same call, scoped to a snapshot.
    #[must_use]
    pub const fn over(mut self, snapshot: &'a WorkspaceHandle) -> Self {
        self.snapshot = Some(snapshot);
        self
    }

    /// The same call, with a declared ceiling.
    #[must_use]
    pub fn within(mut self, budget: Budget) -> Self {
        self.budget = Some(budget);
        self
    }
}

/// A `states`-only budget.
///
/// `states` is the one budget dimension this daemon enforces
/// (`continuumd::daemon::verification`), and the other eight are reported as typed
/// omissions when a caller declares them. Declaring only what is enforced keeps the
/// omission manifest about the *daemon's* limits rather than about the client's habits.
#[must_use]
pub fn states_budget(states: u64) -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(states),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

/// The minimal typed client.
///
/// It holds a connection's negotiated version, the principal it speaks as, a monotone call
/// counter, and a byte ledger. It holds no snapshot, no task, no continuation, and no
/// cache: plan §20's "adapters do not own semantic state", as a property of the struct's
/// fields rather than as a promise in a comment.
#[derive(Debug, Clone)]
pub struct AgentClient {
    version: ProtocolVersion,
    actor: ActorId,
    capability: CapabilityHandle,
    calls: u64,
    ledger: ByteLedger,
}

impl AgentClient {
    /// A client speaking `version` as `actor` under `capability`.
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
            ledger: ByteLedger {
                handshake: CallBytes::NONE,
                operations: CallBytes::NONE,
                calls: 0,
                unmet: 0,
            },
        }
    }

    /// Speak as a different principal from here on.
    ///
    /// This changes whose authority the daemon checks, not what that authority is. A
    /// principal with less authority gets less; a principal with none gets
    /// `CapabilityDenied` (INV-015).
    pub fn speak_as(&mut self, actor: ActorId, capability: CapabilityHandle) {
        self.actor = actor;
        self.capability = capability;
    }

    /// The principal this client currently speaks as.
    #[must_use]
    pub const fn principal(&self) -> (&ActorId, &CapabilityHandle) {
        (&self.actor, &self.capability)
    }

    /// Every interface byte this client has spent.
    #[must_use]
    pub const fn ledger(&self) -> ByteLedger {
        self.ledger
    }

    /// How many calls this client has made, including locally refused ones.
    #[must_use]
    pub const fn attempts(&self) -> u64 {
        self.calls + self.ledger.unmet
    }

    /// Open the connection: offer `hello`, read the welcome.
    ///
    /// # Errors
    ///
    /// [`ClientError`] when the hello cannot be encoded, the wire fails, or the answer is
    /// not a `ServerWelcome`. A *rejected* connection is a `ServerReject` frame and decodes
    /// as a codec error here, which is the honest reading: this method's contract is "the
    /// connection opened", and a rejection did not open one.
    pub fn open(
        &mut self,
        link: &mut dyn Transport,
        hello: &ClientHello,
    ) -> Result<ServerWelcome, ClientError> {
        let frame = encode_hello(hello)?;
        let answer = link.open(&frame)?;
        self.ledger.handshake = CallBytes {
            sent: frame.len() as u64,
            received: answer.len() as u64,
        };
        Ok(from_bytes(&answer)?)
    }

    /// Perform one typed call.
    ///
    /// # Errors
    ///
    /// [`ClientError`] when the request cannot be encoded, the wire fails, or the answer is
    /// not the shape the operation declares. A daemon's refusal is **not** an error: it is
    /// an [`Outcome::Refused`] inside the returned [`Answer`].
    pub fn invoke(
        &mut self,
        link: &mut dyn Transport,
        context: &AgentContext,
        call: &Call<'_>,
    ) -> Result<Answer, ClientError> {
        let operation = call.arguments.operation();
        if let Err(unmet) = register::admits(context, operation) {
            self.ledger.unmet += 1;
            return Ok(Answer {
                operation,
                outcome: Outcome::Unmet(unmet),
                bytes: CallBytes::NONE,
            });
        }

        self.calls += 1;
        let envelope = self.envelope(operation, call);
        let frame = encode_request(&envelope, call.arguments)?;
        let answer = link.exchange(&frame)?;
        let bytes = CallBytes {
            sent: frame.len() as u64,
            received: answer.len() as u64,
        };
        self.ledger.operations.sent += bytes.sent;
        self.ledger.operations.received += bytes.received;
        self.ledger.calls += 1;

        let (result, payload) = decode_result(operation, &answer)?;
        let outcome = classify(result, payload)?;
        Ok(Answer {
            operation,
            outcome,
            bytes,
        })
    }

    /// The request envelope for one call.
    ///
    /// The idempotency key is minted for a `@mutation` and withheld for a `@readonly`
    /// operation, because RFC 0026 requires the first and forbids the second. Which is which
    /// is read from `continuumd::daemon::is_mutation` — the daemon's own predicate — rather
    /// than from a second list here.
    fn envelope(&self, operation: &'static str, call: &Call<'_>) -> RequestEnvelope {
        let sequence = self.calls;
        RequestEnvelope {
            protocol_version: self.version,
            request_id: RequestId::new(&format!("req_c{sequence:06}"))
                .expect("a counter renders a well-formed request id"),
            idempotency_key: if is_mutation(operation) {
                Optional::Present(format!("idem-c{sequence:06}"))
            } else {
                Optional::Absent
            },
            actor: self.actor.clone(),
            capability: self.capability.clone(),
            operation: OperationName::new(operation).expect("a registry name is well formed"),
            snapshot: match call.snapshot {
                Some(handle) => Nullable::Value(handle.clone()),
                None => Nullable::Null,
            },
            intent: match call.intent {
                Some(handle) => Nullable::Value(handle.clone()),
                None => Nullable::Null,
            },
            arguments: Opaque::from_bytes(Vec::new()),
            budget: match &call.budget {
                Some(budget) => Optional::Present(budget.clone()),
                None => Optional::Absent,
            },
            output_policy: Optional::Absent,
            trace: Optional::Absent,
            page: Optional::Absent,
        }
    }

    // --- the eight named operations ---------------------------------------------------

    /// `workspace.create` — import a snapshot, optionally sealing it on creation.
    ///
    /// # Errors
    ///
    /// [`ClientError`] as [`AgentClient::invoke`].
    pub fn workspace_create(
        &mut self,
        link: &mut dyn Transport,
        context: &AgentContext,
        components: SnapshotComponents,
        seal: bool,
    ) -> Result<Answer, ClientError> {
        let arguments = Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components,
            overlay: Optional::Absent,
            seal: Optional::Present(seal),
        });
        self.invoke(link, context, &Call::new(&arguments))
    }

    /// `workspace.fork` — derive a snapshot with one file overlaid.
    ///
    /// # Errors
    ///
    /// [`ClientError`] as [`AgentClient::invoke`].
    pub fn workspace_fork(
        &mut self,
        link: &mut dyn Transport,
        context: &AgentContext,
        base: &WorkspaceHandle,
        overlay: Vec<FileOverlay>,
    ) -> Result<Answer, ClientError> {
        let arguments = Arguments::WorkspaceFork(WorkspaceForkRequest {
            base: base.clone(),
            overlay: Optional::Present(overlay),
            patches: Optional::Absent,
        });
        self.invoke(link, context, &Call::new(&arguments))
    }

    /// `workspace.seal` — freeze a snapshot so verification may run over it.
    ///
    /// # Errors
    ///
    /// [`ClientError`] as [`AgentClient::invoke`].
    pub fn workspace_seal(
        &mut self,
        link: &mut dyn Transport,
        context: &AgentContext,
        snapshot: &WorkspaceHandle,
    ) -> Result<Answer, ClientError> {
        let arguments = Arguments::WorkspaceSeal(WorkspaceSealRequest {
            snapshot: snapshot.clone(),
        });
        self.invoke(link, context, &Call::new(&arguments))
    }

    /// `verification.start` — begin a campaign over a sealed snapshot under a declared budget.
    ///
    /// The budget is the caller's whole declaration rather than a state count, because a
    /// caller that declares a dimension this daemon does not meter is owed the typed INV-007
    /// omission that says so — and a convenience signature that could only spell `states`
    /// would make that manifest unreachable through the named surface.
    ///
    /// # Errors
    ///
    /// [`ClientError`] as [`AgentClient::invoke`].
    pub fn verification_start(
        &mut self,
        link: &mut dyn Transport,
        context: &AgentContext,
        snapshot: &WorkspaceHandle,
        target: Target,
        budget: Budget,
    ) -> Result<Answer, ClientError> {
        let arguments = Arguments::VerificationStart(VerificationStartRequest {
            target,
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        });
        self.invoke(
            link,
            context,
            &Call::new(&arguments).over(snapshot).within(budget),
        )
    }

    /// `verification.result` — read a campaign's verdict.
    ///
    /// # Errors
    ///
    /// [`ClientError`] as [`AgentClient::invoke`].
    pub fn verification_result(
        &mut self,
        link: &mut dyn Transport,
        context: &AgentContext,
        task: &TaskHandle,
    ) -> Result<Answer, ClientError> {
        let arguments =
            Arguments::VerificationResult(VerificationResultRequest { task: task.clone() });
        self.invoke(link, context, &Call::new(&arguments))
    }

    /// `task.status` — read a task record.
    ///
    /// # Errors
    ///
    /// [`ClientError`] as [`AgentClient::invoke`].
    pub fn task_status(
        &mut self,
        link: &mut dyn Transport,
        context: &AgentContext,
        task: &TaskHandle,
    ) -> Result<Answer, ClientError> {
        let arguments = Arguments::TaskStatus(TaskStatusRequest { task: task.clone() });
        self.invoke(link, context, &Call::new(&arguments))
    }

    /// `task.cancel` — close a campaign, keeping whatever it committed.
    ///
    /// # Errors
    ///
    /// [`ClientError`] as [`AgentClient::invoke`].
    pub fn task_cancel(
        &mut self,
        link: &mut dyn Transport,
        context: &AgentContext,
        task: &TaskHandle,
    ) -> Result<Answer, ClientError> {
        let arguments = Arguments::TaskCancel(TaskCancelRequest { task: task.clone() });
        self.invoke(link, context, &Call::new(&arguments))
    }

    /// `task.resume` — spend a continuation under a new declared budget.
    ///
    /// # Errors
    ///
    /// [`ClientError`] as [`AgentClient::invoke`].
    pub fn task_resume(
        &mut self,
        link: &mut dyn Transport,
        context: &AgentContext,
        continuation: &ContinuationHandle,
        budget: Budget,
    ) -> Result<Answer, ClientError> {
        let arguments = Arguments::TaskResume(TaskResumeRequest {
            continuation: continuation.clone(),
            budget: Optional::Present(budget.clone()),
        });
        // `task.resume` is `@task_starting` as well as `@mutation`, so RFC 0026 makes the
        // *envelope's* budget required too. The ceiling is declared twice because the IDL
        // asks for it twice, and a client that filled in only the body would be answered
        // `MalformedRequest` — which is a fact about the protocol worth encoding once, here,
        // rather than rediscovering per caller.
        self.invoke(link, context, &Call::new(&arguments).within(budget))
    }
}

/// Turn a decoded result envelope and payload into the typed outcome.
///
/// The one thing this function refuses to do is guess. `status = error` with no `error`
/// object, or any other status with one, is a self-contradicting envelope, and picking a
/// half to believe would let a malformed answer become a semantic one.
fn classify(
    result: ResultEnvelope,
    payload: continuumd::daemon::family::Payload,
) -> Result<Outcome, ClientError> {
    let is_error = result.status == ResultStatus::Error;
    let error = result.error;
    match (is_error, error) {
        (true, Optional::Present(error)) => Ok(Outcome::Refused(Refusal {
            request_id: result.request_id,
            code: error.code,
            detail: error.detail,
            retryable: error.retryable,
            recovery: error.recovery,
            continuation: match error.continuation {
                Optional::Present(handle) => Some(handle),
                Optional::Absent => None,
            },
            non_resumable_reason: match error.non_resumable_reason {
                Optional::Present(reason) => Some(reason),
                Optional::Absent => None,
            },
        })),
        (false, Optional::Absent) => Ok(Outcome::Admitted(Admitted {
            request_id: result.request_id,
            status: result.status,
            payload,
            verdict: match result.verdict {
                Nullable::Value(verdict) => Some(verdict),
                Nullable::Null => None,
            },
            assurance: match result.assurance {
                Optional::Present(assurance) => Some(assurance),
                Optional::Absent => None,
            },
            task: match result.task {
                Optional::Present(handle) => Some(handle),
                Optional::Absent => None,
            },
            continuation: match result.continuation {
                Optional::Present(handle) => Some(handle),
                Optional::Absent => None,
            },
            omissions: result.omissions,
            warnings: result.warnings,
            artifacts: result.artifacts,
            cost: result.cost,
            next_operations: result.next_operations,
        })),
        (status, error) => Err(ClientError::StatusMismatch {
            status: if status {
                ResultStatus::Error
            } else {
                result.status
            },
            error_present: !error.is_absent(),
        }),
    }
}
