//! `rule errors.common`, as a function of registry data.
//!
//! > Every operation MAY additionally return `MalformedRequest`,
//! > `ProtocolVersionUnsupported`, `CapabilityDenied`, `QuotaExhausted`, and
//! > `EpochUnsupported`; every `@mutation` operation MAY additionally return
//! > `IdempotencyKeyReused` and `PublicationAborted`; every operation taking a non-null
//! > `snapshot` MAY additionally return `StaleSnapshot`. An operation's `errors` clause
//! > lists the codes it may return beyond these. A daemon MUST NOT return a code outside
//! > that union for the operation.
//! >
//! > — `schemas/continuumd-native-protocol.idl`, `rule errors.common`
//!
//! The rule is stated once, here, and read off [`OperationSpec`] — its `annotations` and its
//! `errors` clause — so no handler carries a private list of the codes it is allowed to
//! raise. The dispatcher checks every fault against it on the way out.
//!
//! # The one clause this function reads structurally
//!
//! "every operation taking a non-null `snapshot`" is a property of a *request*, not of the
//! registry: `RequestEnvelope.snapshot` is `nullable`, so whether an operation takes one is
//! decided per call. [`admits`] therefore admits `StaleSnapshot` for any operation whose
//! request struct or envelope can name a snapshot, and [`admits_with_snapshot`] is the
//! per-call form for a caller that knows. The dispatcher uses the permissive form, because
//! a check that rejected a *correct* code would be worse than one that missed an incorrect
//! one, and the operations that can name a snapshot are exactly the ones whose families
//! raise the code.

use crate::protocol::spec::{Annotation, OperationSpec};
use crate::protocol::vocabulary::ErrorCode;

/// The codes every operation may return.
///
/// `UnsupportedSemanticFeature` is the sixth as of protocol 3.2, and it is here because
/// `rule errors.unsupported_surface` requires it of any operation whose producing
/// subsystem has not shipped while this union forbade it for twenty-five of the
/// seventy-two — the contradiction bn-3gi documented and bn-i4aem reconciled. The
/// direction is recorded in `rule errors.common` itself and in RFC 0026 correction 42.
pub const COMMON: &[ErrorCode] = &[
    ErrorCode::MalformedRequest,
    ErrorCode::ProtocolVersionUnsupported,
    ErrorCode::CapabilityDenied,
    ErrorCode::QuotaExhausted,
    ErrorCode::EpochUnsupported,
    ErrorCode::UnsupportedSemanticFeature,
];

/// The codes every `@mutation` operation may additionally return.
pub const MUTATION: &[ErrorCode] = &[
    ErrorCode::IdempotencyKeyReused,
    ErrorCode::PublicationAborted,
];

/// The code an operation taking a non-null `snapshot` may additionally return.
pub const SNAPSHOT: &[ErrorCode] = &[ErrorCode::StaleSnapshot];

/// Whether `code` is inside the union `rule errors.common` allows for `spec`.
#[must_use]
pub fn admits(spec: &OperationSpec, code: ErrorCode) -> bool {
    admits_with_snapshot(spec, code, true)
}

/// [`admits`], with the caller stating whether this call names a snapshot.
#[must_use]
pub fn admits_with_snapshot(spec: &OperationSpec, code: ErrorCode, snapshot: bool) -> bool {
    if COMMON.contains(&code) || spec.errors.contains(&code) {
        return true;
    }
    if spec.has(Annotation::Mutation) && MUTATION.contains(&code) {
        return true;
    }
    snapshot && SNAPSHOT.contains(&code)
}
