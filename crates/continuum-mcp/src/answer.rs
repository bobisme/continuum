//! What a typed operation answers, and what it cost.
//!
//! # Three arms, not two
//!
//! An agent's call ends in exactly one of three places, and collapsing any two of them
//! would lose the distinction the caller has to act on:
//!
//! | Arm | What happened | Bytes on the wire |
//! |---|---|---|
//! | [`Outcome::Admitted`] | the daemon ran the operation | request + result frame |
//! | [`Outcome::Refused`] | the daemon answered a typed [`Error`] | request + result frame |
//! | [`Outcome::Unmet`] | the *client* refused: preconditions do not hold | none |
//!
//! [`Outcome::Unmet`] is research/25's "invalid operations become low-cost local feedback",
//! and it is a genuine third arm rather than a refusal in disguise: no daemon saw the call,
//! no idempotency key was spent, and nothing was audited. It is still an **attempt** — see
//! [`Answer::attempted`] — because a validity metric that stopped counting the operations
//! an interface talks its caller out of would be measuring the interface's tact rather than
//! the agent's accuracy.
//!
//! # The cost ledger
//!
//! RFC 0027 makes *interface bytes* the graded cost denominator, "bytes, not tokens", and
//! plan §24.5's ratified ACI margins repeat it. [`CallBytes`] is therefore counted where
//! the bytes actually are — the frame this client wrote and the frame it read — and
//! [`ByteLedger`] is their running total. Neither number is an estimate and neither is a
//! model-relative token count: they are the lengths of two `Vec<u8>`s.
//!
//! The handshake is counted too ([`ByteLedger::handshake`]), because a surface that needed
//! a large negotiation to start would be paying a real cost a per-operation total would
//! hide.
//!
//! [`Error`]: continuumd::protocol::envelope::Error

use continuumd::codec::CodecError;
use continuumd::daemon::family::Payload;
use continuumd::protocol::envelope::{
    ArtifactRef, AssuranceEnvelope, Cost, NextOperation, Omission, Verdict, Warning,
};
use continuumd::protocol::scalar::{ContinuationHandle, RequestId, TaskHandle};
use continuumd::protocol::vocabulary::{ErrorCode, ResultStatus};

use crate::link::LinkError;
use crate::register::Unmet;

/// What one typed operation answered, and what it cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The operation that was called, as the wire names it.
    pub operation: &'static str,
    /// Which of the three arms this call landed on.
    pub outcome: Outcome,
    /// Interface bytes this call spent.
    pub bytes: CallBytes,
}

impl Answer {
    /// Whether this call was an attempt at an interface operation.
    ///
    /// Always true, on every arm. The method exists so that a metric which counts attempts
    /// reads the intent rather than the shape: an [`Outcome::Unmet`] costs no bytes and
    /// reaches no daemon, and it is still one of the operations the agent decided to make.
    #[must_use]
    pub const fn attempted(&self) -> bool {
        true
    }

    /// Whether the daemon admitted the operation.
    ///
    /// "Admitted" is the wire's own reading: a [`ResultEnvelope`] whose `status` is not
    /// `error`. `ok`, `task_started` and `task_suspended` are all admissions — a suspended
    /// task is a budget outcome carrying a continuation, never a rejected request
    /// (docs/49, RFC 0026).
    ///
    /// [`ResultEnvelope`]: continuumd::protocol::envelope::ResultEnvelope
    #[must_use]
    pub const fn admitted(&self) -> bool {
        matches!(self.outcome, Outcome::Admitted(_))
    }

    /// The typed error code, on either refusing arm.
    ///
    /// [`Outcome::Unmet`] has no [`ErrorCode`] and this returns [`None`] for it: the client
    /// refused, and minting a wire code for a call the wire never saw would put a fiction
    /// in the caller's error taxonomy.
    #[must_use]
    pub const fn error_code(&self) -> Option<ErrorCode> {
        match &self.outcome {
            Outcome::Refused(refusal) => Some(refusal.code),
            Outcome::Admitted(_) | Outcome::Unmet(_) => None,
        }
    }

    /// The admitted answer, when there is one.
    #[must_use]
    pub const fn success(&self) -> Option<&Admitted> {
        match &self.outcome {
            Outcome::Admitted(admitted) => Some(admitted),
            Outcome::Refused(_) | Outcome::Unmet(_) => None,
        }
    }

    /// The daemon's typed refusal, when there is one.
    #[must_use]
    pub const fn refusal(&self) -> Option<&Refusal> {
        match &self.outcome {
            Outcome::Refused(refusal) => Some(refusal),
            Outcome::Admitted(_) | Outcome::Unmet(_) => None,
        }
    }

    /// The client's local precondition refusal, when there is one.
    #[must_use]
    pub const fn unmet(&self) -> Option<&Unmet> {
        match &self.outcome {
            Outcome::Unmet(unmet) => Some(unmet),
            Outcome::Admitted(_) | Outcome::Refused(_) => None,
        }
    }
}

/// The three arms a call can land on.
// `Admitted` carries a whole decoded envelope — the payload, the assurance envelope, the
// omission manifest — and is several times the size of a refusal. Boxing it would put an
// allocation on the *common* arm and make the type's shape argue that a success is the
// unusual case, which is the opposite of what this surface is for. The same choice, for the
// same reason, as `continuumd::daemon::family::Arguments`, whose own comment says it: "the
// uniform shape is worth more than the moved bytes". Nothing here is on a hot path — one
// value per interface call, moved once — and none of these bytes is an *interface* byte,
// which is the only quantity PR 10 grades.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The daemon ran the operation.
    Admitted(Admitted),
    /// The daemon answered a typed error.
    Refused(Refusal),
    /// The client refused: the operation's preconditions do not hold in the caller's own
    /// declared state. No frame was written.
    Unmet(Unmet),
}

/// An admitted answer: the typed payload, and the envelope facts an agent acts on.
///
/// Every field here is read off a decoded [`ResultEnvelope`]; none is inferred, and none is
/// a rendering. The set is not "everything the envelope carries" but everything an agent
/// needs to choose a next operation — which is [`register`](crate::register)'s question, and
/// is why `task`, `continuation` and `status` are all present rather than one of them
/// standing in for the others.
///
/// [`ResultEnvelope`]: continuumd::protocol::envelope::ResultEnvelope
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admitted {
    /// The request identifier the daemon echoed.
    pub request_id: RequestId,
    /// `ok`, `task_started`, or `task_suspended`.
    pub status: ResultStatus,
    /// The operation's typed response body.
    pub payload: Payload,
    /// The operation's typed verdict, when it declares one.
    pub verdict: Option<Verdict>,
    /// The assurance envelope, required on every semantic verdict (plan B11).
    pub assurance: Option<AssuranceEnvelope>,
    /// The task this call started or observes.
    pub task: Option<TaskHandle>,
    /// The continuation a suspension parked.
    pub continuation: Option<ContinuationHandle>,
    /// The INV-007 omission manifest, unabridged. Empty means nothing was omitted.
    pub omissions: Vec<Omission>,
    /// Typed warnings; never interpolated source text (INV-016).
    pub warnings: Vec<Warning>,
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
/// *branched on*: an agent that read the sentence instead of [`Refusal::code`] would be
/// scraping prose through a typed hole, which is the failure INV-003 names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The request identifier the daemon echoed.
    pub request_id: RequestId,
    /// The closed error vocabulary member.
    pub code: ErrorCode,
    /// The stable, non-interpolated explanation of the code in context.
    pub detail: String,
    /// Whether an identical retry can succeed with no change by the caller.
    pub retryable: bool,
    /// The allowed operations from this state, with pre-filled arguments. This is the
    /// protocol's only recovery channel: never a command string (RFC 0026).
    pub recovery: Vec<NextOperation>,
    /// Present when the failure is resumable.
    pub continuation: Option<ContinuationHandle>,
    /// Present when a budget exhaustion is *not* resumable: the typed reason.
    pub non_resumable_reason: Option<String>,
}

/// Interface bytes spent by one call.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct CallBytes {
    /// Bytes of the request frame this client wrote.
    pub sent: u64,
    /// Bytes of the result frame this client read.
    pub received: u64,
}

impl CallBytes {
    /// Nothing was written and nothing was read.
    pub const NONE: Self = Self {
        sent: 0,
        received: 0,
    };

    /// The two directions summed — the RFC 0027 "interface bytes" of this call.
    #[must_use]
    pub const fn total(self) -> u64 {
        self.sent + self.received
    }
}

/// The running interface-byte total of one client.
///
/// Handshake bytes are kept apart from operation bytes rather than folded in, because they
/// answer different questions: `handshake` is what the surface costs to *open*, and
/// `operations` is what it costs to *use*. [`ByteLedger::total`] adds them for the metric
/// RFC 0027 grades.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ByteLedger {
    /// Bytes spent on the connection handshake.
    pub handshake: CallBytes,
    /// Bytes spent on operations.
    pub operations: CallBytes,
    /// How many operation calls reached the wire.
    pub calls: u64,
    /// How many operation calls the register refused locally, spending no bytes.
    pub unmet: u64,
}

impl ByteLedger {
    /// Every interface byte this client has spent.
    #[must_use]
    pub const fn total(self) -> u64 {
        self.handshake.total() + self.operations.total()
    }
}

/// Why a call could not be made or its answer could not be read.
///
/// Never a refusal: a refusal is an [`Outcome::Refused`], answered by a daemon in the
/// protocol's own vocabulary. A [`ClientError`] means this client could not participate —
/// the request would not encode, the wire failed, or the answer was not the shape the
/// operation declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    /// A request could not be encoded, or an answer could not be decoded.
    Codec(CodecError),
    /// The byte boundary failed.
    Link(LinkError),
    /// The daemon answered `status = error` with no `error` object, or a non-error status
    /// with one. The envelope contradicts itself and this client will not guess which half
    /// to believe.
    StatusMismatch {
        /// The status the envelope declared.
        status: ResultStatus,
        /// Whether an `error` object was present.
        error_present: bool,
    },
}

impl core::fmt::Display for ClientError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Codec(error) => write!(f, "{error}"),
            Self::Link(error) => write!(f, "{error}"),
            Self::StatusMismatch {
                status,
                error_present,
            } => write!(
                f,
                "result envelope contradicts itself: status={status:?}, error present={error_present}"
            ),
        }
    }
}

impl core::error::Error for ClientError {}

impl From<CodecError> for ClientError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

impl From<LinkError> for ClientError {
    fn from(error: LinkError) -> Self {
        Self::Link(error)
    }
}
