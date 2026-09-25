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

/// Whether an identical retry of a failure carrying `code` can succeed.
///
/// > The last column is guidance for reading the taxonomy; the authoritative per-occurrence
/// > value is the **required** `Error.retryable` field, which a daemon MUST set on every
/// > error it returns.
/// >
/// > — RFC 0026, "Error taxonomy"
///
/// The IDL declares `Error.retryable` ("Whether an identical retry can succeed without any
/// change by the caller") but carries no per-code retry data: `ErrorCode`'s members are bare
/// names. The per-code value is therefore derived here, once, from RFC 0026's taxonomy
/// table, column "Identical retry can succeed?". Three codes answer **yes**:
///
/// - `StatusConflict` — "re-read and retry";
/// - `QuotaExhausted` — "later, or with fewer concurrent tasks";
/// - `PublicationAborted` — "same idempotency key" ("Atomicity of publication": "The client
///   MAY retry with the same idempotency key, and the retry is a fresh publication").
///
/// Every other code answers **no**. `rule handshake.rejection` fixes `retryable` false for
/// the two handshake codes independently, and the table agrees. The match is exhaustive, so
/// a code added to the `@open` enum does not compile until its row is decided here.
///
/// The value also decides what the idempotency ledger keeps (`Daemon::dispatch` step 7): a
/// retryable failure does not bind its key, because a replay of it would make the identical
/// retry the table says can succeed unable to.
#[must_use]
pub const fn retryable(code: ErrorCode) -> bool {
    retryable_code(code)
}

/// The first protocol version that defines `code` (`@since`), or `None` for a code every
/// version of the current major defines.
///
/// The dates are [`crate::protocol::since`]'s: `tests/idl_conformance.rs` requires this
/// function to agree with [`crate::protocol::since::ENUM_MEMBERS`] for every member of
/// `ErrorCode`, and that table to agree with the IDL.
#[must_use]
pub const fn introduced_at(code: ErrorCode) -> Option<crate::protocol::scalar::ProtocolVersion> {
    match code {
        ErrorCode::OutcomeUnknown => Some(crate::protocol::since::OUTCOME_UNKNOWN),
        _ => None,
    }
}

/// Whether a connection negotiated at `version` defines `code`. A daemon MUST NOT emit a
/// member the negotiated version does not define (`rule versioning.enums`), and this is the
/// check at the one boundary every answer, fresh or replayed, crosses (review cr-1dc5ii).
#[must_use]
pub fn defined_at(code: ErrorCode, version: crate::protocol::scalar::ProtocolVersion) -> bool {
    crate::protocol::since::defines(introduced_at(code), version)
}

const fn retryable_code(code: ErrorCode) -> bool {
    match code {
        ErrorCode::StatusConflict | ErrorCode::QuotaExhausted | ErrorCode::PublicationAborted => {
            true
        }
        ErrorCode::StaleSnapshot
        | ErrorCode::UnsupportedSemanticFeature
        | ErrorCode::IntentMutationDenied
        | ErrorCode::InsufficientEvidence
        | ErrorCode::BudgetExhausted
        | ErrorCode::ContinuationEpochMismatch
        | ErrorCode::AmbiguousCorrespondence
        | ErrorCode::UntrustedDomainBoundary
        | ErrorCode::CertificateRejected
        | ErrorCode::ReplayDiverged
        | ErrorCode::CapabilityDenied
        | ErrorCode::PolicyGateFailed
        | ErrorCode::AcceptanceChainInvalid
        | ErrorCode::EpochUnsupported
        | ErrorCode::ProtocolVersionUnsupported
        | ErrorCode::IdempotencyKeyReused
        | ErrorCode::MalformedRequest
        | ErrorCode::OutcomeUnknown => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::spec::ProtocolEnum;

    /// RFC 0026's taxonomy table has exactly three "yes" rows.
    #[test]
    fn exactly_the_three_taxonomy_yes_rows_are_retryable() {
        let yes: Vec<ErrorCode> = ErrorCode::ALL
            .iter()
            .copied()
            .filter(|code| retryable(*code))
            .collect();
        assert_eq!(
            yes,
            [
                ErrorCode::StatusConflict,
                ErrorCode::QuotaExhausted,
                ErrorCode::PublicationAborted,
            ]
        );
    }
}
