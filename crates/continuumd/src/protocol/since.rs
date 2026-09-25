//! What each protocol version defines: the IDL's `@since` dates, as data (bn-7xz8v).
//!
//! > `@since("X.Y")` — first protocol version defining the item. Omitted means the first
//! > minor of the current major (`3.0`), never `protocol.version`.
//! >
//! > — `notes/plan/schemas/continuumd-native-protocol.idl`, §1
//!
//! `rule versioning.compatible_change` lets a minor add an operation, an `optional` field,
//! a member of an `@open` enum, or an error code. A connection negotiates one version, and a
//! daemon serves that version and nothing newer: it refuses an operation the version does
//! not declare, it ignores a request field the version does not define (`rule
//! versioning.compatible_change`: "Servers MUST ignore unknown optional request fields"),
//! and it never emits a field, an enum member, or an error code the version does not define
//! (`rule versioning.compatible_change`, `rule versioning.enums`).
//!
//! This module is the one place those dates live. Every version gate in the daemon reads a
//! constant here (one through an alias, `INSTANCE_SCOPE_SINCE`) — the dispatch gate reads an operation's date from
//! [`OperationSpec::since`](super::spec::OperationSpec::since), whose values are these
//! constants — and `tests/idl_conformance.rs` parses the IDL independently and requires
//! the whole set to agree with it, date for date: every dated operation, every dated field
//! ([`FIELDS`]), every dated enum member ([`ENUM_MEMBERS`]), and every dated named
//! declaration ([`DECLARATIONS`]). An item the IDL dates and this module omits fails that
//! test, and so does the reverse.
//!
//! Where each date is enforced:
//!
//! - an operation: `Daemon::dispatch` step 2, `MalformedRequest` before admission and
//!   before the idempotency ledger (`rule signing.identities` names the code); recovery
//!   offers naming it are dropped;
//! - `ResultEnvelope.audit`: withheld from every answer below 3.1 (`Daemon`'s
//!   `emitted_at`); `rule audit.correlation` applies from 3.1 (RFC 0026 correction 62);
//! - `CertificateRejection`: the `Error.data` of `CertificateRejected` is withheld below 3.4
//!   (`emitted_at`, `rule encoding.opaque_payloads`, correction 62);
//! - `CapabilityDescriptor.profile` and `.instances`: withheld from the welcome's grant
//!   (`CapabilityDescriptor::as_reported_at`);
//! - `SnapshotComponents.file_components`: ignored in a request (`workspace.create`);
//! - `EvidenceGetResponse.signature`: withheld (`evidence.get`);
//! - `ErrorCode::OutcomeUnknown`: never emitted (`daemon::errors::defined_at`), and a
//!   custody daemon refuses the writes that could need it (`rule signing.custody`);
//! - a named declaration: through the items that reach it (checked by
//!   `idl_conformance.rs`).

use super::scalar::ProtocolVersion;

/// `ResultEnvelope.audit` (`@since("3.1")`).
pub const RESULT_AUDIT: ProtocolVersion = ProtocolVersion::new(3, 1);

/// `CapabilityDescriptor.profile` (`@since("3.1")`).
pub const CAPABILITY_PROFILE: ProtocolVersion = ProtocolVersion::new(3, 1);

/// `ServerReject` (`@since("3.1")`): the frame a daemon sends only to a client whose offer
/// reaches this version (`rule handshake.rejection`).
pub const SERVER_REJECT: ProtocolVersion = ProtocolVersion::new(3, 1);

/// `SnapshotComponents.file_components` (`@since("3.2")`).
pub const FILE_COMPONENTS: ProtocolVersion = ProtocolVersion::new(3, 2);

/// `CertificateRejection` (`@since("3.4")`), the declared shape of `Error.data` on a
/// `CertificateRejected` error (RFC 0026 F19). Below 3.4 no code declares a shape, so the
/// daemon leaves `data` absent there (`rule encoding.opaque_payloads`, correction 62).
pub const CERTIFICATE_REJECTION: ProtocolVersion = ProtocolVersion::new(3, 4);

/// `evidence.link` (`@since("3.3")`).
pub const EVIDENCE_LINK: ProtocolVersion = ProtocolVersion::new(3, 3);

/// `whiteboard.compile` (`@since("3.5")`).
pub const WHITEBOARD_COMPILE: ProtocolVersion = ProtocolVersion::new(3, 5);

/// `workspace.create_by_reference` (`@since("3.6")`).
pub const CREATE_BY_REFERENCE: ProtocolVersion = ProtocolVersion::new(3, 6);

/// `CapabilityDescriptor.instances` (`@since("3.7")`, `rule capability.instance_scope`).
pub const INSTANCE_SCOPE: ProtocolVersion = ProtocolVersion::new(3, 7);

/// The signing wire (`@since("3.8")`): the six `signing` operations,
/// `intent.export_bundle`, `intent.import_bundle`, and `EvidenceGetResponse.signature`.
pub const SIGNING_WIRE: ProtocolVersion = ProtocolVersion::new(3, 8);

/// `ErrorCode::OutcomeUnknown` (`@since("3.9")`, `rule signing.custody`).
pub const OUTCOME_UNKNOWN: ProtocolVersion = ProtocolVersion::new(3, 9);

/// The first version of the current major, `3.0`: what an undated item means ("Omitted
/// means the first minor of the current major", IDL §1). `tests/idl_conformance.rs` holds
/// it to `protocol.version`'s major.
pub const CURRENT_MAJOR_BASE: ProtocolVersion = ProtocolVersion::new(3, 0);

/// Whether a connection negotiated at `version` defines an item dated `since`.
///
/// A (major, minor) relation. `None` is an undated item, defined from
/// [`CURRENT_MAJOR_BASE`]. This IDL defines the current major only, so a connection of
/// another major defines nothing here, dated or undated. The N−1 major is served as a
/// negotiation window with no surface until a 2.x IDL exists: such a connection negotiates,
/// and every operation it names is refused as not declared at its version (RFC 0026
/// correction 62, user ruling on cr-88az2y). Serving the 3.x surface to a 2.x connection,
/// as before, was a defect.
#[must_use]
pub fn defines(since: Option<ProtocolVersion>, version: ProtocolVersion) -> bool {
    let since = since.unwrap_or(CURRENT_MAJOR_BASE);
    version.major() == CURRENT_MAJOR_BASE.major() && version >= since
}

/// A dated field: `(declaring type, field, @since)`. The declaring type is the IDL's name,
/// or the generated name of an anonymous body (`EvidenceGetResponse`).
pub const FIELDS: &[(&str, &str, ProtocolVersion)] = &[
    ("ResultEnvelope", "audit", RESULT_AUDIT),
    ("CapabilityDescriptor", "profile", CAPABILITY_PROFILE),
    ("CapabilityDescriptor", "instances", INSTANCE_SCOPE),
    ("SnapshotComponents", "file_components", FILE_COMPONENTS),
    ("EvidenceGetResponse", "signature", SIGNING_WIRE),
];

/// A dated enum member: `(enum, member identifier, @since)`.
pub const ENUM_MEMBERS: &[(&str, &str, ProtocolVersion)] =
    &[("ErrorCode", "OutcomeUnknown", OUTCOME_UNKNOWN)];

/// A dated named declaration (struct, enum, or alias): `(name, @since)`.
///
/// A declaration is reached on the wire only through a field, a union variant, or an
/// operation body, so its date is enforced through theirs; `tests/idl_conformance.rs`
/// checks that nothing reaches a dated declaration through an item dated earlier.
pub const DECLARATIONS: &[(&str, ProtocolVersion)] = &[
    ("AuditCorrelationId", RESULT_AUDIT),
    ("SignerHandle", SIGNING_WIRE),
    ("CertificateRejection", CERTIFICATE_REJECTION),
    ("ServerReject", SERVER_REJECT),
    ("DataGrant", CAPABILITY_PROFILE),
    ("CapabilityProfile", CAPABILITY_PROFILE),
    ("WhiteboardTaskProposal", WHITEBOARD_COMPILE),
    ("SignedArtifactKind", SIGNING_WIRE),
    ("RevocationReason", SIGNING_WIRE),
    ("SignatureOutcome", SIGNING_WIRE),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_undated_item_is_defined_at_every_version_of_the_current_major_only() {
        assert!(defines(None, ProtocolVersion::new(3, 0)));
        assert!(defines(None, ProtocolVersion::new(3, 9)));
        assert!(!defines(None, ProtocolVersion::new(2, 0)));
        assert!(!defines(None, ProtocolVersion::new(2, 9)));
        assert!(!defines(None, ProtocolVersion::new(4, 0)));
    }

    #[test]
    fn a_dated_item_is_defined_from_its_date_on() {
        assert!(!defines(Some(EVIDENCE_LINK), ProtocolVersion::new(3, 2)));
        assert!(defines(Some(EVIDENCE_LINK), ProtocolVersion::new(3, 3)));
        assert!(defines(Some(EVIDENCE_LINK), ProtocolVersion::new(3, 9)));
        assert!(!defines(Some(EVIDENCE_LINK), ProtocolVersion::new(2, 9)));
    }
}
