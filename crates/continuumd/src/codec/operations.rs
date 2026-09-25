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
use crate::codec::versioned::read_at;
use crate::codec::{CodecError, Document, from_opaque_in, to_opaque_in};
use crate::daemon::family::{Arguments, ErrorData, Payload};
use crate::protocol::operations::{
    context, evidence, intent, observe, signing, task, verification, whiteboard, workspace,
};
use crate::protocol::scalar::{Opaque, ProtocolVersion};
use crate::protocol::vocabulary::ErrorCode;

/// Decode an envelope's `arguments` into the typed request body its operation declares.
///
/// # Errors
///
/// [`CodecError::UnknownOperation`] when this daemon declares no request shape for the
/// named operation — the 45 of the 83 whose families have not landed — and any decode
/// failure of the named struct otherwise.
pub fn decode_arguments_in<D: Document>(
    operation: &str,
    arguments: &Opaque,
) -> Result<Arguments, CodecError> {
    decode_arguments_with::<D>(operation, arguments, None)
}

/// Decode an envelope's `arguments` as a connection negotiated at `version` reads them:
/// a request field `version` does not define is ignored, not decoded, and every other
/// field is decoded strictly ([`crate::codec::versioned`], bn-7xz8v). This is what the
/// transport calls.
///
/// # Errors
///
/// As [`decode_arguments_in`].
pub fn decode_arguments_at<D: Document>(
    operation: &str,
    arguments: &Opaque,
    version: ProtocolVersion,
) -> Result<Arguments, CodecError> {
    decode_arguments_with::<D>(operation, arguments, Some(version))
}

fn decode_arguments_with<D: Document>(
    operation: &str,
    arguments: &Opaque,
    version: Option<ProtocolVersion>,
) -> Result<Arguments, CodecError> {
    Ok(match operation {
        "workspace.create" => Arguments::WorkspaceCreate(read_at::<
            D,
            workspace::WorkspaceCreateRequest,
        >(arguments.as_bytes(), version)?),
        "workspace.fork" => Arguments::WorkspaceFork(
            read_at::<D, workspace::WorkspaceForkRequest>(arguments.as_bytes(), version)?,
        ),
        "workspace.diff" => Arguments::WorkspaceDiff(
            read_at::<D, workspace::WorkspaceDiffRequest>(arguments.as_bytes(), version)?,
        ),
        "workspace.seal" => Arguments::WorkspaceSeal(
            read_at::<D, workspace::WorkspaceSealRequest>(arguments.as_bytes(), version)?,
        ),
        "intent.get" => Arguments::IntentGet(read_at::<D, intent::IntentGetRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "intent.diff" => Arguments::IntentDiff(read_at::<D, intent::IntentDiffRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "intent.propose_revision" => {
            Arguments::IntentProposeRevision(read_at::<D, intent::IntentProposeRevisionRequest>(
                arguments.as_bytes(),
                version,
            )?)
        }
        "intent.accept" => Arguments::IntentAccept(read_at::<D, intent::IntentAcceptRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "intent.reject" => Arguments::IntentReject(read_at::<D, intent::IntentRejectRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "intent.lock" => Arguments::IntentLock(read_at::<D, intent::IntentLockRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "evidence.get" => Arguments::EvidenceGet(read_at::<D, evidence::EvidenceGetRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "evidence.query" => Arguments::EvidenceQuery(read_at::<D, evidence::EvidenceQueryRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "evidence.verify" => Arguments::EvidenceVerify(read_at::<
            D,
            evidence::EvidenceVerifyRequest,
        >(arguments.as_bytes(), version)?),
        "evidence.subscribe" => {
            Arguments::EvidenceSubscribe(read_at::<D, evidence::EvidenceSubscribeRequest>(
                arguments.as_bytes(),
                version,
            )?)
        }
        "evidence.link" => Arguments::EvidenceLink(read_at::<D, evidence::EvidenceLinkRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "observe.ingest" => Arguments::ObserveIngest(read_at::<D, observe::ObserveIngestRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "observe.classify" => Arguments::ObserveClassify(read_at::<
            D,
            observe::ObserveClassifyRequest,
        >(arguments.as_bytes(), version)?),
        "observe.result" => Arguments::ObserveResult(read_at::<D, observe::ObserveResultRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "verification.start" => {
            Arguments::VerificationStart(read_at::<D, verification::VerificationStartRequest>(
                arguments.as_bytes(),
                version,
            )?)
        }
        "verification.result" => {
            Arguments::VerificationResult(read_at::<D, verification::VerificationResultRequest>(
                arguments.as_bytes(),
                version,
            )?)
        }
        "verification.await" => {
            Arguments::VerificationAwait(read_at::<D, verification::VerificationAwaitRequest>(
                arguments.as_bytes(),
                version,
            )?)
        }
        "task.status" => Arguments::TaskStatus(read_at::<D, task::TaskStatusRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "task.cancel" => Arguments::TaskCancel(read_at::<D, task::TaskCancelRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "task.resume" => Arguments::TaskResume(read_at::<D, task::TaskResumeRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "task.subscribe" => Arguments::TaskSubscribe(read_at::<D, task::TaskSubscribeRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "task.update_budget" => {
            Arguments::TaskUpdateBudget(read_at::<D, task::TaskUpdateBudgetRequest>(
                arguments.as_bytes(),
                version,
            )?)
        }
        "context.compile" => Arguments::ContextCompile(
            read_at::<D, context::ContextCompileRequest>(arguments.as_bytes(), version)?,
        ),
        "context.expand" => Arguments::ContextExpand(read_at::<D, context::ContextExpandRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "whiteboard.compile" => {
            Arguments::WhiteboardCompile(read_at::<D, whiteboard::WhiteboardCompileRequest>(
                arguments.as_bytes(),
                version,
            )?)
        }
        "workspace.create_by_reference" => {
            Arguments::WorkspaceCreateByReference(read_at::<
                D,
                workspace::WorkspaceCreateByReferenceRequest,
            >(arguments.as_bytes(), version)?)
        }
        "intent.export_bundle" => {
            Arguments::IntentExportBundle(read_at::<D, intent::IntentExportBundleRequest>(
                arguments.as_bytes(),
                version,
            )?)
        }
        "intent.import_bundle" => {
            Arguments::IntentImportBundle(read_at::<D, intent::IntentImportBundleRequest>(
                arguments.as_bytes(),
                version,
            )?)
        }
        "signing.mint" => Arguments::SigningMint(read_at::<D, signing::SigningMintRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "signing.rotate" => Arguments::SigningRotate(read_at::<D, signing::SigningRotateRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "signing.revoke" => Arguments::SigningRevoke(read_at::<D, signing::SigningRevokeRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "signing.registry" => Arguments::SigningRegistry(read_at::<
            D,
            signing::SigningRegistryRequest,
        >(arguments.as_bytes(), version)?),
        "signing.verify" => Arguments::SigningVerify(read_at::<D, signing::SigningVerifyRequest>(
            arguments.as_bytes(),
            version,
        )?),
        "signing.sign_pack" => Arguments::SigningSignPack(read_at::<
            D,
            signing::SigningSignPackRequest,
        >(arguments.as_bytes(), version)?),
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
        Payload::ContextCompile(body) => to_opaque_in::<D, _>(body)?,
        Payload::ContextExpand(body) => to_opaque_in::<D, _>(body)?,
        Payload::WhiteboardCompile(body) => to_opaque_in::<D, _>(body)?,
        Payload::WorkspaceCreateByReference(body) => to_opaque_in::<D, _>(body)?,
        Payload::IntentExportBundle(body) => to_opaque_in::<D, _>(body)?,
        Payload::IntentImportBundle(body) => to_opaque_in::<D, _>(body)?,
        Payload::SigningMint(body) => to_opaque_in::<D, _>(body)?,
        Payload::SigningRotate(body) => to_opaque_in::<D, _>(body)?,
        Payload::SigningRevoke(body) => to_opaque_in::<D, _>(body)?,
        Payload::SigningRegistry(body) => to_opaque_in::<D, _>(body)?,
        Payload::SigningVerify(body) => to_opaque_in::<D, _>(body)?,
        Payload::SigningSignPack(body) => to_opaque_in::<D, _>(body)?,
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
        "context.compile" => Payload::ContextCompile(from_opaque_in::<D, _>(payload)?),
        "context.expand" => Payload::ContextExpand(from_opaque_in::<D, _>(payload)?),
        "whiteboard.compile" => Payload::WhiteboardCompile(from_opaque_in::<D, _>(payload)?),
        "workspace.create_by_reference" => {
            Payload::WorkspaceCreateByReference(from_opaque_in::<D, _>(payload)?)
        }
        "intent.export_bundle" => Payload::IntentExportBundle(from_opaque_in::<D, _>(payload)?),
        "intent.import_bundle" => Payload::IntentImportBundle(from_opaque_in::<D, _>(payload)?),
        "signing.mint" => Payload::SigningMint(from_opaque_in::<D, _>(payload)?),
        "signing.rotate" => Payload::SigningRotate(from_opaque_in::<D, _>(payload)?),
        "signing.revoke" => Payload::SigningRevoke(from_opaque_in::<D, _>(payload)?),
        "signing.registry" => Payload::SigningRegistry(from_opaque_in::<D, _>(payload)?),
        "signing.verify" => Payload::SigningVerify(from_opaque_in::<D, _>(payload)?),
        "signing.sign_pack" => Payload::SigningSignPack(from_opaque_in::<D, _>(payload)?),
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

/// Encode a typed `Error.data` value into the `Opaque` the error carries.
///
/// The `Error.data` half of the same seam the payload tables above are: the daemon layer
/// carries the typed [`ErrorData`] beside the envelope and this is where it becomes bytes
/// of the negotiated encoding. [`ErrorData::None`] returns [`None`], which the caller
/// leaves as the absent field `rule encoding.opaque_payloads` requires of every code with
/// no declared shape (RFC 0026 F19, protocol 3.4).
///
/// # Errors
///
/// [`CodecError`] when the declared struct cannot be encoded.
pub fn encode_error_data_in<D: Document>(data: &ErrorData) -> Result<Option<Opaque>, CodecError> {
    Ok(Some(match data {
        ErrorData::None => return Ok(None),
        ErrorData::CertificateRejection(body) => to_opaque_in::<D, _>(body.as_ref())?,
    }))
}

/// Decode an error's `data` into the typed value its `code` declares.
///
/// The resolution is a function of the message alone — `Error.code` is `required` in the
/// same object — exactly as the envelope's payloads resolve through `operation`
/// (`rule encoding.opaque_payloads`). A code that declares no shape answers
/// [`CodecError::UnknownOperation`]'s sibling refusal here: there is nothing declared for
/// the bytes to validate against, and inventing a reading would be the interpretation the
/// rule forbids.
///
/// # Errors
///
/// [`CodecError::UndeclaredErrorData`] when `code` declares no `Error.data` shape, and
/// any decode failure of the declared struct otherwise.
pub fn decode_error_data_in<D: Document>(
    code: ErrorCode,
    data: &Opaque,
) -> Result<ErrorData, CodecError> {
    match code {
        ErrorCode::CertificateRejected => {
            Ok(ErrorData::CertificateRejection(Box::new(from_opaque_in::<
                D,
                crate::protocol::envelope::CertificateRejection,
            >(
                data
            )?)))
        }
        _ => Err(CodecError::UndeclaredErrorData),
    }
}

/// Encode a typed `Error.data` value, in `canonical_json`.
///
/// # Errors
///
/// As [`encode_error_data_in`].
pub fn encode_error_data(data: &ErrorData) -> Result<Option<Opaque>, CodecError> {
    encode_error_data_in::<Json>(data)
}

/// Decode an error's `data`, in `canonical_json`.
///
/// # Errors
///
/// As [`decode_error_data_in`].
pub fn decode_error_data(code: ErrorCode, data: &Opaque) -> Result<ErrorData, CodecError> {
    decode_error_data_in::<Json>(code, data)
}
