//! The annotation obligations, enforced from registry data rather than per handler.
//!
//! Every obligation here is a sentence the IDL writes on a *field* of `RequestEnvelope` or
//! `ResultEnvelope` and conditions on an *annotation* of the operation:
//!
//! | Field | IDL sentence | Annotation it conditions on |
//! |---|---|---|
//! | `idempotency_key` | "REQUIRED for `@mutation` operations, absent for `@readonly` ones" | [`Annotation::Mutation`], [`Annotation::Readonly`] |
//! | `budget` | "REQUIRED for `@task_starting` operations" | [`Annotation::TaskStarting`] |
//! | `audit` | "REQUIRED on every result of an `@audit_recorded` operation and on every admission denial" | [`Annotation::AuditRecorded`] |
//!
//! [`registry::OPERATIONS`](crate::protocol::registry::OPERATIONS) already carries the
//! annotations as data, so the obligation is a table lookup and a field test. Writing it
//! per handler would be the same sentence 72 times, and the 72nd would be the one that
//! disagreed; more to the point, a family that *forgot* would be admitting an unkeyed
//! mutation, which is the failure `rule idempotency.replay` exists to prevent.
//!
//! # Why an absent key is `MalformedRequest`
//!
//! There is no code for "missing required field". The IDL's own definition of
//! `MalformedRequest` is "The request does not parse, **does not validate against this
//! file**, or uses an unknown closed-enum member", and "REQUIRED for `@mutation`
//! operations" is a statement in this file about what validates. `IdempotencyKeyReused` is
//! not the code: that one is reserved for "the same `idempotency_key` was replayed with a
//! different canonical request", which is a different fact about a key that is present.

use super::family::Fault;
use crate::protocol::envelope::RequestEnvelope;
use crate::protocol::spec::{Annotation, OperationSpec};
use crate::protocol::vocabulary::ErrorCode;

/// The obligation an operation's annotations put on its request envelope.
///
/// # Errors
///
/// A [`Fault`] carrying [`ErrorCode::MalformedRequest`] naming the field, never the value:
/// the value came off the wire and `rule envelope.no_prose` keeps it out of `detail`.
pub fn check_request(spec: &OperationSpec, envelope: &RequestEnvelope) -> Result<(), Fault> {
    if spec.has(Annotation::Mutation) {
        match envelope.idempotency_key.value() {
            Some(key) if !key.is_empty() => {}
            _ => {
                return Err(Fault::new(
                    ErrorCode::MalformedRequest,
                    "a @mutation operation requires a non-empty `idempotency_key`",
                ));
            }
        }
    }

    // The IDL states the converse in the same sentence, and it is not decoration: a key on
    // a read is a caller that believes the call mutates something, and honouring it would
    // make the ledger's contents depend on a misunderstanding.
    if spec.has(Annotation::Readonly) && !envelope.idempotency_key.is_absent() {
        return Err(Fault::new(
            ErrorCode::MalformedRequest,
            "a @readonly operation carries no `idempotency_key`",
        ));
    }

    if spec.has(Annotation::TaskStarting) && envelope.budget.is_absent() {
        return Err(Fault::new(
            ErrorCode::MalformedRequest,
            "a @task_starting operation requires a `budget`",
        ));
    }

    Ok(())
}

/// Whether a result of this operation must carry an audit-correlation identity.
///
/// > It MUST be present on every result of an `@audit_recorded` operation, on every
/// > `@privileged` call (which `@audit_recorded` covers), and on every result whose error
/// > is `CapabilityDenied`.
/// >
/// > — `rule audit.correlation`
///
/// The denial half is not an annotation and is decided by the dispatcher; the two
/// annotations are read here. `@privileged` is tested as well as `@audit_recorded` because
/// the parenthesis is a claim about the registry — every `@privileged` operation carries
/// `@audit_recorded` too — and a claim about the registry is cheaper to *enforce* than to
/// re-verify by reading it.
#[must_use]
pub fn audit_required(spec: &OperationSpec) -> bool {
    spec.has(Annotation::AuditRecorded) || spec.has(Annotation::Privileged)
}
