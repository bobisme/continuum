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
//! `tests/codec_canonical_cbor.rs` closes the loop from the other end: every variant of
//! [`Arguments`] that these tables produce encodes and decodes back to itself, in both
//! encodings, so a table arm that named the wrong struct could not stay green.
//!
//! # Both encodings, one table
//!
//! Each function comes in two spellings: a `_in::<D>` one that resolves the payload in
//! whichever encoding `D` names, and a plain one fixed to `canonical_json` for the callers
//! that have always used it. There is one *table* behind both, because the table says
//! which struct an operation carries and that is a fact about the operation rather than
//! about the encoding.

use crate::codec::cbor::Cbor;
use crate::codec::json::Json;
use crate::codec::{CodecError, Document, from_opaque_in, to_opaque_in};
use crate::daemon::family::{Arguments, Payload};
use crate::protocol::operations::{evidence, intent, observe, task, verification, workspace};
use crate::protocol::scalar::Opaque;

/// Decode an envelope's `arguments` into the typed request body its operation declares.
///
/// # Errors
///
/// [`CodecError::UnknownOperation`] when this daemon declares no request shape for the
/// named operation — the 47 of the 73 whose families have not landed — and any decode
/// failure of the named struct otherwise.
pub fn decode_arguments_in<D: Document>(
    operation: &str,
    arguments: &Opaque,
) -> Result<Arguments, CodecError> {
    Ok(match operation {
        "workspace.create" => Arguments::WorkspaceCreate(from_opaque_in::<
            D,
            workspace::WorkspaceCreateRequest,
        >(arguments)?),
        "workspace.fork" => Arguments::WorkspaceFork(from_opaque_in::<
            D,
            workspace::WorkspaceForkRequest,
        >(arguments)?),
        "workspace.diff" => Arguments::WorkspaceDiff(from_opaque_in::<
            D,
            workspace::WorkspaceDiffRequest,
        >(arguments)?),
        "workspace.seal" => Arguments::WorkspaceSeal(from_opaque_in::<
            D,
            workspace::WorkspaceSealRequest,
        >(arguments)?),
        "intent.get" => {
            Arguments::IntentGet(from_opaque_in::<D, intent::IntentGetRequest>(arguments)?)
        }
        "intent.diff" => {
            Arguments::IntentDiff(from_opaque_in::<D, intent::IntentDiffRequest>(arguments)?)
        }
        "intent.propose_revision" => Arguments::IntentProposeRevision(from_opaque_in::<
            D,
            intent::IntentProposeRevisionRequest,
        >(arguments)?),
        "intent.accept" => {
            Arguments::IntentAccept(from_opaque_in::<D, intent::IntentAcceptRequest>(arguments)?)
        }
        "intent.reject" => {
            Arguments::IntentReject(from_opaque_in::<D, intent::IntentRejectRequest>(arguments)?)
        }
        "intent.lock" => {
            Arguments::IntentLock(from_opaque_in::<D, intent::IntentLockRequest>(arguments)?)
        }
        "evidence.get" => Arguments::EvidenceGet(
            from_opaque_in::<D, evidence::EvidenceGetRequest>(arguments)?,
        ),
        "evidence.query" => Arguments::EvidenceQuery(from_opaque_in::<
            D,
            evidence::EvidenceQueryRequest,
        >(arguments)?),
        "evidence.verify" => Arguments::EvidenceVerify(from_opaque_in::<
            D,
            evidence::EvidenceVerifyRequest,
        >(arguments)?),
        "evidence.subscribe" => Arguments::EvidenceSubscribe(from_opaque_in::<
            D,
            evidence::EvidenceSubscribeRequest,
        >(arguments)?),
        "evidence.link" => Arguments::EvidenceLink(from_opaque_in::<
            D,
            evidence::EvidenceLinkRequest,
        >(arguments)?),
        "observe.ingest" => Arguments::ObserveIngest(from_opaque_in::<
            D,
            observe::ObserveIngestRequest,
        >(arguments)?),
        "observe.classify" => Arguments::ObserveClassify(from_opaque_in::<
            D,
            observe::ObserveClassifyRequest,
        >(arguments)?),
        "observe.result" => Arguments::ObserveResult(from_opaque_in::<
            D,
            observe::ObserveResultRequest,
        >(arguments)?),
        "verification.start" => Arguments::VerificationStart(from_opaque_in::<
            D,
            verification::VerificationStartRequest,
        >(arguments)?),
        "verification.result" => Arguments::VerificationResult(from_opaque_in::<
            D,
            verification::VerificationResultRequest,
        >(arguments)?),
        "verification.await" => Arguments::VerificationAwait(from_opaque_in::<
            D,
            verification::VerificationAwaitRequest,
        >(arguments)?),
        "task.status" => {
            Arguments::TaskStatus(from_opaque_in::<D, task::TaskStatusRequest>(arguments)?)
        }
        "task.cancel" => {
            Arguments::TaskCancel(from_opaque_in::<D, task::TaskCancelRequest>(arguments)?)
        }
        "task.resume" => {
            Arguments::TaskResume(from_opaque_in::<D, task::TaskResumeRequest>(arguments)?)
        }
        "task.subscribe" => {
            Arguments::TaskSubscribe(from_opaque_in::<D, task::TaskSubscribeRequest>(arguments)?)
        }
        "task.update_budget" => Arguments::TaskUpdateBudget(from_opaque_in::<
            D,
            task::TaskUpdateBudgetRequest,
        >(arguments)?),
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
pub fn encode_payload_in<D: Document>(payload: &Payload) -> Result<Option<Opaque>, CodecError> {
    Ok(Some(match payload {
        Payload::None => return Ok(None),
        Payload::WorkspaceCreate(body) => to_opaque_in::<D, _>(body)?,
        Payload::WorkspaceFork(body) => to_opaque_in::<D, _>(body)?,
        Payload::WorkspaceDiff(body) => to_opaque_in::<D, _>(body)?,
        Payload::WorkspaceSeal(body) => to_opaque_in::<D, _>(body)?,
        Payload::IntentGet(body) => to_opaque_in::<D, _>(body)?,
        Payload::IntentDiff(body) => to_opaque_in::<D, _>(body)?,
        Payload::IntentProposeRevision(body) => to_opaque_in::<D, _>(body)?,
        Payload::IntentAccept(body) => to_opaque_in::<D, _>(body)?,
        Payload::IntentReject(body) => to_opaque_in::<D, _>(body)?,
        Payload::IntentLock(body) => to_opaque_in::<D, _>(body)?,
        Payload::EvidenceGet(body) => to_opaque_in::<D, _>(body)?,
        Payload::EvidenceQuery(body) => to_opaque_in::<D, _>(body)?,
        Payload::EvidenceVerify(body) => to_opaque_in::<D, _>(body)?,
        Payload::EvidenceSubscribe(body) => to_opaque_in::<D, _>(body)?,
        Payload::EvidenceLink(body) => to_opaque_in::<D, _>(body)?,
        Payload::ObserveIngest(body) => to_opaque_in::<D, _>(body)?,
        Payload::ObserveClassify(body) => to_opaque_in::<D, _>(body)?,
        Payload::ObserveResult(body) => to_opaque_in::<D, _>(body)?,
        Payload::VerificationStart(body) => to_opaque_in::<D, _>(body)?,
        Payload::VerificationResult(body) => to_opaque_in::<D, _>(body)?,
        Payload::VerificationAwait(body) => to_opaque_in::<D, _>(body)?,
        Payload::TaskStatus(body) => to_opaque_in::<D, _>(body)?,
        Payload::TaskCancel(body) => to_opaque_in::<D, _>(body)?,
        Payload::TaskResume(body) => to_opaque_in::<D, _>(body)?,
        Payload::TaskSubscribe(body) => to_opaque_in::<D, _>(body)?,
        Payload::TaskUpdateBudget(body) => to_opaque_in::<D, _>(body)?,
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
pub fn decode_payload_in<D: Document>(
    operation: &str,
    payload: &Opaque,
) -> Result<Payload, CodecError> {
    Ok(match operation {
        "workspace.create" => Payload::WorkspaceCreate(from_opaque_in::<D, _>(payload)?),
        "workspace.fork" => Payload::WorkspaceFork(from_opaque_in::<D, _>(payload)?),
        "workspace.diff" => Payload::WorkspaceDiff(from_opaque_in::<D, _>(payload)?),
        "workspace.seal" => Payload::WorkspaceSeal(from_opaque_in::<D, _>(payload)?),
        "intent.get" => Payload::IntentGet(from_opaque_in::<D, _>(payload)?),
        "intent.diff" => Payload::IntentDiff(from_opaque_in::<D, _>(payload)?),
        "intent.propose_revision" => {
            Payload::IntentProposeRevision(from_opaque_in::<D, _>(payload)?)
        }
        "intent.accept" => Payload::IntentAccept(from_opaque_in::<D, _>(payload)?),
        "intent.reject" => Payload::IntentReject(from_opaque_in::<D, _>(payload)?),
        "intent.lock" => Payload::IntentLock(from_opaque_in::<D, _>(payload)?),
        "evidence.get" => Payload::EvidenceGet(from_opaque_in::<D, _>(payload)?),
        "evidence.query" => Payload::EvidenceQuery(from_opaque_in::<D, _>(payload)?),
        "evidence.verify" => Payload::EvidenceVerify(from_opaque_in::<D, _>(payload)?),
        "evidence.subscribe" => Payload::EvidenceSubscribe(from_opaque_in::<D, _>(payload)?),
        "evidence.link" => Payload::EvidenceLink(from_opaque_in::<D, _>(payload)?),
        "observe.ingest" => Payload::ObserveIngest(from_opaque_in::<D, _>(payload)?),
        "observe.classify" => Payload::ObserveClassify(from_opaque_in::<D, _>(payload)?),
        "observe.result" => Payload::ObserveResult(from_opaque_in::<D, _>(payload)?),
        "verification.start" => Payload::VerificationStart(from_opaque_in::<D, _>(payload)?),
        "verification.result" => Payload::VerificationResult(from_opaque_in::<D, _>(payload)?),
        "verification.await" => Payload::VerificationAwait(from_opaque_in::<D, _>(payload)?),
        "task.status" => Payload::TaskStatus(from_opaque_in::<D, _>(payload)?),
        "task.cancel" => Payload::TaskCancel(from_opaque_in::<D, _>(payload)?),
        "task.resume" => Payload::TaskResume(from_opaque_in::<D, _>(payload)?),
        "task.subscribe" => Payload::TaskSubscribe(from_opaque_in::<D, _>(payload)?),
        "task.update_budget" => Payload::TaskUpdateBudget(from_opaque_in::<D, _>(payload)?),
        _ => return Err(CodecError::UnknownOperation),
    })
}

// --- the `canonical_json` spellings -------------------------------------------------
//
// Named without a suffix because they are what every caller in this crate has meant by
// "decode the arguments" since the codec landed, and the transport picks the encoding
// explicitly where a connection negotiated one.

/// Decode an envelope's `arguments`, in `canonical_json`.
///
/// # Errors
///
/// As [`decode_arguments_in`].
pub fn decode_arguments(operation: &str, arguments: &Opaque) -> Result<Arguments, CodecError> {
    decode_arguments_in::<Json>(operation, arguments)
}

/// Encode a typed response body into an `Opaque`, in `canonical_json`.
///
/// # Errors
///
/// As [`encode_payload_in`].
pub fn encode_payload(payload: &Payload) -> Result<Option<Opaque>, CodecError> {
    encode_payload_in::<Json>(payload)
}

/// Decode an envelope's `payload`, in `canonical_json`.
///
/// # Errors
///
/// As [`decode_payload_in`].
pub fn decode_payload(operation: &str, payload: &Opaque) -> Result<Payload, CodecError> {
    decode_payload_in::<Json>(operation, payload)
}

/// Decode an envelope's `arguments`, in `canonical_cbor`.
///
/// # Errors
///
/// As [`decode_arguments_in`].
pub fn decode_cbor_arguments(operation: &str, arguments: &Opaque) -> Result<Arguments, CodecError> {
    decode_arguments_in::<Cbor>(operation, arguments)
}

/// Encode a typed response body into an `Opaque`, in `canonical_cbor`.
///
/// # Errors
///
/// As [`encode_payload_in`].
pub fn encode_cbor_payload(payload: &Payload) -> Result<Option<Opaque>, CodecError> {
    encode_payload_in::<Cbor>(payload)
}

/// Decode an envelope's `payload`, in `canonical_cbor`.
///
/// # Errors
///
/// As [`decode_payload_in`].
pub fn decode_cbor_payload(operation: &str, payload: &Opaque) -> Result<Payload, CodecError> {
    decode_payload_in::<Cbor>(operation, payload)
}
