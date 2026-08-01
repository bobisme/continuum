//! Resolving the envelope's `Opaque` payloads to the structs the operation declares.
//!
//! > `RequestEnvelope.arguments` is the request struct of the operation the same
//! > envelope's `operation` field names, and `ResultEnvelope.payload` is that operation's
//! > response struct. […] Each resolution is a function of the message alone.
//! >
//! > — `rule encoding.opaque_payloads`
//!
//! # Why the two tables live here and not in the transport
//!
//! `daemon::family` already names, once, which request struct each operation carries and
//! which response struct it answers with: [`Arguments`] and [`Payload`] are closed enums
//! with one variant per operation, and each has an `operation()` that returns the wire
//! name. This module is the *codec* half of that same seam, and putting it beside those
//! enums rather than inside the transport keeps a single answer to "what shape does this
//! operation carry": adding an operation is one variant in each enum and one arm in each
//! table here, and a mismatch between the four is a compile error rather than a decode
//! that silently produces the wrong shape.
//!
//! `every_operation_the_families_serve_round_trips_through_the_codec` in
//! `tests/codec_round_trip.rs` closes the loop from the other end: every variant of
//! [`Arguments`] encodes and decodes back to itself, so a table arm that named the wrong
//! struct could not stay green.

use crate::codec::{CodecError, from_opaque, to_opaque};
use crate::daemon::family::{Arguments, Payload};
use crate::protocol::operations::{evidence, intent, observe, task, verification, workspace};
use crate::protocol::scalar::Opaque;

/// Decode an envelope's `arguments` into the typed request body its operation declares.
///
/// # Errors
///
/// [`CodecError::UnknownOperation`] when this daemon declares no request shape for the
/// named operation — the 47 of the 72 whose families have not landed — and any decode
/// failure of the named struct otherwise.
pub fn decode_arguments(operation: &str, arguments: &Opaque) -> Result<Arguments, CodecError> {
    Ok(match operation {
        "workspace.create" => {
            Arguments::WorkspaceCreate(from_opaque::<workspace::WorkspaceCreateRequest>(arguments)?)
        }
        "workspace.fork" => {
            Arguments::WorkspaceFork(from_opaque::<workspace::WorkspaceForkRequest>(arguments)?)
        }
        "workspace.diff" => {
            Arguments::WorkspaceDiff(from_opaque::<workspace::WorkspaceDiffRequest>(arguments)?)
        }
        "workspace.seal" => {
            Arguments::WorkspaceSeal(from_opaque::<workspace::WorkspaceSealRequest>(arguments)?)
        }
        "intent.get" => Arguments::IntentGet(from_opaque::<intent::IntentGetRequest>(arguments)?),
        "intent.diff" => {
            Arguments::IntentDiff(from_opaque::<intent::IntentDiffRequest>(arguments)?)
        }
        "intent.propose_revision" => Arguments::IntentProposeRevision(from_opaque::<
            intent::IntentProposeRevisionRequest,
        >(arguments)?),
        "intent.accept" => {
            Arguments::IntentAccept(from_opaque::<intent::IntentAcceptRequest>(arguments)?)
        }
        "intent.reject" => {
            Arguments::IntentReject(from_opaque::<intent::IntentRejectRequest>(arguments)?)
        }
        "intent.lock" => {
            Arguments::IntentLock(from_opaque::<intent::IntentLockRequest>(arguments)?)
        }
        "evidence.get" => {
            Arguments::EvidenceGet(from_opaque::<evidence::EvidenceGetRequest>(arguments)?)
        }
        "evidence.query" => {
            Arguments::EvidenceQuery(from_opaque::<evidence::EvidenceQueryRequest>(arguments)?)
        }
        "evidence.verify" => {
            Arguments::EvidenceVerify(from_opaque::<evidence::EvidenceVerifyRequest>(arguments)?)
        }
        "evidence.subscribe" => Arguments::EvidenceSubscribe(from_opaque::<
            evidence::EvidenceSubscribeRequest,
        >(arguments)?),
        "observe.ingest" => {
            Arguments::ObserveIngest(from_opaque::<observe::ObserveIngestRequest>(arguments)?)
        }
        "observe.classify" => {
            Arguments::ObserveClassify(from_opaque::<observe::ObserveClassifyRequest>(arguments)?)
        }
        "observe.result" => {
            Arguments::ObserveResult(from_opaque::<observe::ObserveResultRequest>(arguments)?)
        }
        "verification.start" => Arguments::VerificationStart(from_opaque::<
            verification::VerificationStartRequest,
        >(arguments)?),
        "verification.result" => Arguments::VerificationResult(from_opaque::<
            verification::VerificationResultRequest,
        >(arguments)?),
        "verification.await" => Arguments::VerificationAwait(from_opaque::<
            verification::VerificationAwaitRequest,
        >(arguments)?),
        "task.status" => Arguments::TaskStatus(from_opaque::<task::TaskStatusRequest>(arguments)?),
        "task.cancel" => Arguments::TaskCancel(from_opaque::<task::TaskCancelRequest>(arguments)?),
        "task.resume" => Arguments::TaskResume(from_opaque::<task::TaskResumeRequest>(arguments)?),
        "task.subscribe" => {
            Arguments::TaskSubscribe(from_opaque::<task::TaskSubscribeRequest>(arguments)?)
        }
        "task.update_budget" => {
            Arguments::TaskUpdateBudget(from_opaque::<task::TaskUpdateBudgetRequest>(arguments)?)
        }
        _ => return Err(CodecError::UnknownOperation),
    })
}

/// Encode a typed response body into the `Opaque` the envelope's `payload` carries.
///
/// [`Payload::None`] returns [`None`], which the caller writes as the envelope's `null` —
/// "the operation's response struct. Null on `status = error`".
///
/// # Errors
///
/// [`CodecError`] when the body carries a non-canonical `Opaque` of its own.
pub fn encode_payload(payload: &Payload) -> Result<Option<Opaque>, CodecError> {
    Ok(Some(match payload {
        Payload::None => return Ok(None),
        Payload::WorkspaceCreate(body) => to_opaque(body)?,
        Payload::WorkspaceFork(body) => to_opaque(body)?,
        Payload::WorkspaceDiff(body) => to_opaque(body)?,
        Payload::WorkspaceSeal(body) => to_opaque(body)?,
        Payload::IntentGet(body) => to_opaque(body)?,
        Payload::IntentDiff(body) => to_opaque(body)?,
        Payload::IntentProposeRevision(body) => to_opaque(body)?,
        Payload::IntentAccept(body) => to_opaque(body)?,
        Payload::IntentReject(body) => to_opaque(body)?,
        Payload::IntentLock(body) => to_opaque(body)?,
        Payload::EvidenceGet(body) => to_opaque(body)?,
        Payload::EvidenceQuery(body) => to_opaque(body)?,
        Payload::EvidenceVerify(body) => to_opaque(body)?,
        Payload::EvidenceSubscribe(body) => to_opaque(body)?,
        Payload::ObserveIngest(body) => to_opaque(body)?,
        Payload::ObserveClassify(body) => to_opaque(body)?,
        Payload::ObserveResult(body) => to_opaque(body)?,
        Payload::VerificationStart(body) => to_opaque(body)?,
        Payload::VerificationResult(body) => to_opaque(body)?,
        Payload::VerificationAwait(body) => to_opaque(body)?,
        Payload::TaskStatus(body) => to_opaque(body)?,
        Payload::TaskCancel(body) => to_opaque(body)?,
        Payload::TaskResume(body) => to_opaque(body)?,
        Payload::TaskSubscribe(body) => to_opaque(body)?,
        Payload::TaskUpdateBudget(body) => to_opaque(body)?,
    }))
}

/// Decode an envelope's `payload` into the typed response body its operation declares.
///
/// The mirror of [`encode_payload`], and the half a *client* needs: a client reads the
/// result envelope, and the operation it invoked names the struct the payload carries.
///
/// # Errors
///
/// [`CodecError::UnknownOperation`] for an operation with no response shape here, and any
/// decode failure of the named struct otherwise.
pub fn decode_payload(operation: &str, payload: &Opaque) -> Result<Payload, CodecError> {
    Ok(match operation {
        "workspace.create" => Payload::WorkspaceCreate(from_opaque(payload)?),
        "workspace.fork" => Payload::WorkspaceFork(from_opaque(payload)?),
        "workspace.diff" => Payload::WorkspaceDiff(from_opaque(payload)?),
        "workspace.seal" => Payload::WorkspaceSeal(from_opaque(payload)?),
        "intent.get" => Payload::IntentGet(from_opaque(payload)?),
        "intent.diff" => Payload::IntentDiff(from_opaque(payload)?),
        "intent.propose_revision" => Payload::IntentProposeRevision(from_opaque(payload)?),
        "intent.accept" => Payload::IntentAccept(from_opaque(payload)?),
        "intent.reject" => Payload::IntentReject(from_opaque(payload)?),
        "intent.lock" => Payload::IntentLock(from_opaque(payload)?),
        "evidence.get" => Payload::EvidenceGet(from_opaque(payload)?),
        "evidence.query" => Payload::EvidenceQuery(from_opaque(payload)?),
        "evidence.verify" => Payload::EvidenceVerify(from_opaque(payload)?),
        "evidence.subscribe" => Payload::EvidenceSubscribe(from_opaque(payload)?),
        "observe.ingest" => Payload::ObserveIngest(from_opaque(payload)?),
        "observe.classify" => Payload::ObserveClassify(from_opaque(payload)?),
        "observe.result" => Payload::ObserveResult(from_opaque(payload)?),
        "verification.start" => Payload::VerificationStart(from_opaque(payload)?),
        "verification.result" => Payload::VerificationResult(from_opaque(payload)?),
        "verification.await" => Payload::VerificationAwait(from_opaque(payload)?),
        "task.status" => Payload::TaskStatus(from_opaque(payload)?),
        "task.cancel" => Payload::TaskCancel(from_opaque(payload)?),
        "task.resume" => Payload::TaskResume(from_opaque(payload)?),
        "task.subscribe" => Payload::TaskSubscribe(from_opaque(payload)?),
        "task.update_budget" => Payload::TaskUpdateBudget(from_opaque(payload)?),
        _ => return Err(CodecError::UnknownOperation),
    })
}
