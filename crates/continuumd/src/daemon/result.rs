//! Building the result envelope, and the three rules that constrain every one of them.
//!
//! 1. **`rule envelope.epochs_named`** — "Every result MUST name all six epochs. An epoch
//!    the result cannot pin reads null; it is never an absent field." The epoch set is
//!    daemon state and is copied onto every result here, success and failure alike, so no
//!    handler can produce one without it.
//! 2. **`rule audit.correlation`** — the identity is a function of the request identity
//!    alone. It is computed once per dispatch, before the outcome is known, and passed in;
//!    nothing here can make it depend on what happened.
//! 3. **`rule envelope.no_prose`** — every `detail` is a `&'static str`, so no source text,
//!    log line, model output, or production payload can reach one.
//!
//! # The two `Opaque` fields, and why they read null here
//!
//! `ResultEnvelope.payload` and `Error.data` are `Opaque`, and this layer has no codec: RFC
//! 0026 fixes two encodings and neither their canonical field order nor their union
//! tagging, and the IDL's open item 3 leaves the envelope's `Opaque` payloads without a
//! declared shape. Inventing bytes for them would be inventing wire format. So the typed
//! response travels beside the envelope as [`Payload`](super::family::Payload) and the
//! envelope's `payload` reads `null` at this layer — a *placeholder for an unencoded value*,
//! not a wire claim, because this layer emits no wire bytes at all. The same reasoning
//! empties `next_operations` and `Error.recovery`: `NextOperation.arguments` is a required
//! `Opaque`, so an offered operation cannot be filled in without the codec, and offering one
//! with fabricated arguments would be worse than offering none.

use crate::protocol::envelope::{Cost, EpochSet, Error, ResultEnvelope};
use crate::protocol::scalar::{AuditCorrelationId, RequestId};
use crate::protocol::spec::{Nullable, Optional};
use crate::protocol::vocabulary::{ErrorCode, ResultStatus};

use super::family::{Effect, Fault};

/// The detail every `CapabilityDenied` carries, whatever the failing test was.
///
/// > A capability refusal MUST NOT distinguish an unregistered token from an expired, a
/// > revoked, or an unauthorized one: every admission failure is one answer (RFC 0027 X1)
/// > […] `detail` MUST NOT vary with which of them occurred.
/// >
/// > — `rule handshake.rejection`
///
/// The rule is written for the connection frame; X1 is not about frames, so the per-request
/// answer is held to the same constant. One `const` is how "MUST NOT vary" stops being a
/// review obligation.
pub const DENIAL_DETAIL: &str = "the presented capability does not admit this operation";

/// A cost report that measures nothing.
///
/// > A dimension the engine does not measure is absent, never zero.
/// >
/// > — `Cost`, IDL §6
///
/// The operation layer runs no engine and spends no budget dimension it can report
/// honestly, so every dimension is absent. Absent is not a claim that the call was free; it
/// is the absence of a measurement, which is exactly what the IDL asks for and what keeps
/// `rule ordering.deterministic`'s "two identical requests […] MUST return identical bytes"
/// true of a wall clock's output.
#[must_use]
pub fn unmeasured() -> Cost {
    Cost {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Absent,
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
        tokenizer_id: Optional::Absent,
    }
}

/// An epoch set that pins the protocol version and names every other epoch as null.
///
/// A daemon that has not adopted a semantic, intent, evidence, proof, corpus, or engine
/// epoch says so; it does not omit the field. A deployment that has adopted them supplies
/// its own set to the builder.
#[must_use]
pub fn unpinned(protocol: crate::protocol::scalar::ProtocolVersion) -> EpochSet {
    EpochSet {
        protocol,
        semantic: Nullable::Null,
        intent: Nullable::Null,
        evidence: Nullable::Null,
        proof: Nullable::Null,
        corpus: Nullable::Null,
        engine: Nullable::Null,
    }
}

/// The result envelope of a successful call.
#[must_use]
pub fn success(
    request_id: &RequestId,
    effect: &Effect,
    epochs: &EpochSet,
    audit: Option<&AuditCorrelationId>,
) -> ResultEnvelope {
    ResultEnvelope {
        request_id: request_id.clone(),
        status: ResultStatus::Ok,
        verdict: effect.verdict.clone(),
        error: Optional::Absent,
        // `rule envelope.assurance_required` attaches the nine-dimension envelope to
        // `semantic` and `evaluation` verdicts. Neither family here produces one — the
        // workspace and intent operations carry `structural` and `policy` verdicts — so
        // the field is absent rather than filled with dimensions nothing established.
        assurance: Optional::Absent,
        artifacts: effect.artifacts.clone(),
        task: Optional::Absent,
        continuation: Optional::Absent,
        omissions: effect.omissions.clone(),
        warnings: effect.warnings.clone(),
        cost: unmeasured(),
        epochs: epochs.clone(),
        next_operations: Vec::new(),
        next_page_token: Optional::Absent,
        payload: Nullable::Null,
        audit: optional(audit),
    }
}

/// The result envelope of a failed call.
#[must_use]
pub fn failure(
    request_id: &RequestId,
    fault: Fault,
    epochs: &EpochSet,
    audit: Option<&AuditCorrelationId>,
) -> ResultEnvelope {
    ResultEnvelope {
        request_id: request_id.clone(),
        status: ResultStatus::Error,
        verdict: Nullable::Null,
        error: Optional::Present(Error {
            code: fault.code,
            detail: fault.detail.to_owned(),
            data: Optional::Absent,
            recovery: Vec::new(),
            continuation: Optional::Absent,
            non_resumable_reason: Optional::Absent,
            retryable: fault.retryable,
        }),
        assurance: Optional::Absent,
        artifacts: Vec::new(),
        task: Optional::Absent,
        continuation: Optional::Absent,
        omissions: Vec::new(),
        warnings: Vec::new(),
        cost: unmeasured(),
        epochs: epochs.clone(),
        next_operations: Vec::new(),
        next_page_token: Optional::Absent,
        payload: Nullable::Null,
        audit: optional(audit),
    }
}

/// The one answer to every admission failure.
///
/// Every field is fixed or derived from the request identity, so two denials that differ
/// only in whether the named artifact exists are the same value — the existence oracle
/// RFC 0027 X2 forbids is not something this function has the inputs to build.
#[must_use]
pub fn denial(
    request_id: &RequestId,
    epochs: &EpochSet,
    audit: &AuditCorrelationId,
) -> ResultEnvelope {
    failure(
        request_id,
        Fault::new(ErrorCode::CapabilityDenied, DENIAL_DETAIL),
        epochs,
        Some(audit),
    )
}

fn optional(audit: Option<&AuditCorrelationId>) -> Optional<AuditCorrelationId> {
    match audit {
        Some(identity) => Optional::Present(identity.clone()),
        None => Optional::Absent,
    }
}
