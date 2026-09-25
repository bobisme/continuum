//! The operation version gate: a connection is served exactly the operations its
//! negotiated version declares (bn-7xz8v).
//!
//! > Within a protocol major, only compatible changes are permitted, and each MUST raise
//! > the minor version: adding an operation; […]
//! >
//! > — `rule versioning.compatible_change`
//!
//! > Each of the eight is served only on a connection negotiated at 3.8 or later: below it
//! > the request is `MalformedRequest`, decided before admission and before the
//! > idempotency ledger, so the answer does not depend on the grant and no key recorded at
//! > 3.8 replays on an older connection.
//! >
//! > — `rule signing.identities`
//!
//! bn-162z4 found `evidence.link` (`@since("3.3")`) served on a 3.2 connection: the gate
//! was a hand-kept list of the eight 3.8 operations. It now reads each operation's own
//! `@since` (`OperationSpec::since`), which `idl_conformance.rs` holds to the IDL for all 83
//! operations. The refusal is the one `rule signing.identities` names and the one an
//! undeclared operation name gets: `MalformedRequest`, before admission.
//!
//! This file drives every dated operation twice, on a fresh daemon each time: one minor
//! below its date, where it MUST be refused with `MalformedRequest` and change nothing —
//! no admission record, no audit record, no ledger entry, no state — and at its date,
//! where it MUST reach admission. The operation list is the registry's own, so a newly
//! dated operation is driven here without an edit.

use continuum_value::epoch::ProtocolWindow;
use continuumd::codec::operations::decode_arguments;
use continuumd::daemon::evidence::EvidenceFamily;
use continuumd::daemon::family::Arguments;
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::daemon::signing::SigningFamily;
use continuumd::daemon::whiteboard::WhiteboardFamily;
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest};
use continuumd::protocol::envelope::RequestEnvelope;
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::registry::{self, ENCODINGS, OPERATIONS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Opaque, OperationName, ProtocolVersion, RequestId, Timestamp,
};
use continuumd::protocol::spec::{Annotation, Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{AuthorityLevel, DataGrant, Encoding, ErrorCode};

const ROOT_ACTOR: &str = "service:continuumd";

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("capability")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("actor")
}

fn negotiated(version: ProtocolVersion) -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version,
            high: version,
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-version-gate".to_owned(),
        actor: who(ROOT_ACTOR),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version], ProtocolWindow::new(3), ENCODINGS, &hello).expect("served")
}

/// A daemon at `version` with every family a dated operation belongs to, and one root
/// capability that holds every privilege.
fn daemon(version: ProtocolVersion) -> Daemon {
    let privileged: Vec<OperationName> = OPERATIONS
        .iter()
        .filter(|spec| spec.has(Annotation::Privileged))
        .map(|spec| OperationName::new(spec.name).expect("operation"))
        .collect();
    Daemon::builder(Blake3Identity, negotiated(version), cap("cap_root"))
        .now(Timestamp::new("2026-09-24T00:00:00.000Z").expect("timestamp"))
        .capability(
            CapabilityDescriptor {
                capability: cap("cap_root"),
                actor: who(ROOT_ACTOR),
                level: AuthorityLevel::Promote,
                snapshots: Vec::new(),
                intents: Vec::new(),
                artifact_classes: Vec::new(),
                expires_at: Nullable::Null,
                delegation_depth: 1,
                profile: Optional::Present(CapabilityProfile {
                    privileged_operations: privileged,
                    denied_operations: Vec::new(),
                    data_grants: DataGrant::ALL.to_vec(),
                    cross_principal_sharing: false,
                }),
                instances: Optional::Absent,
            },
            None,
        )
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(EvidenceFamily::new())
        .family(ObserveFamily)
        .family(WhiteboardFamily)
        .family(SigningFamily)
        .build()
}

/// A well-formed request body for each dated operation, so that at its date the request
/// is the shape the operation declares. What the handler then decides does not matter
/// here: the gate is decided before admission, and admission is what this file observes.
fn arguments(operation: &str) -> Arguments {
    let signer = format!("signer_{}", "a".repeat(64));
    let body = match operation {
        "evidence.link" => {
            r#"{"checker_profile":"p","receipt":"sha256:00","subject":"ev_x"}"#.to_owned()
        }
        "whiteboard.compile" => r#"{"note":{}}"#.to_owned(),
        "workspace.create_by_reference" => {
            r#"{"components":"sha256:00","epochs":{"proof":"p","semantic":"s"},"intent":"in_x"}"#
                .to_owned()
        }
        "signing.mint" => r#"{"kinds":["receipt"]}"#.to_owned(),
        "signing.rotate" => format!(r#"{{"signer":"{signer}"}}"#),
        "signing.revoke" => format!(r#"{{"reason":"compromised","signer":"{signer}"}}"#),
        "signing.registry" => "{}".to_owned(),
        "signing.verify" => r#"{"artifact":"AA","kind":"receipt"}"#.to_owned(),
        "signing.sign_pack" => r#"{"pack":"AA"}"#.to_owned(),
        "intent.export_bundle" => r#"{"intents":["in_x"]}"#.to_owned(),
        "intent.import_bundle" => r#"{"content":"AA"}"#.to_owned(),
        other => panic!(
            "{other} is dated in the IDL and this file has no body for it: add one, so the \
             gate is driven for every dated operation"
        ),
    };
    decode_arguments(operation, &Opaque::from_bytes(body.into_bytes()))
        .unwrap_or_else(|error| panic!("{operation}: the body decodes: {error:?}"))
}

fn call(daemon: &mut Daemon, version: ProtocolVersion, operation: &str) -> OperationOutcome {
    let spec = registry::operation(operation).expect("a declared operation");
    let envelope = RequestEnvelope {
        protocol_version: version,
        request_id: RequestId::new("req_gate").expect("request id"),
        idempotency_key: if spec.has(Annotation::Mutation) {
            Optional::Present("gate-key".to_owned())
        } else {
            Optional::Absent
        },
        actor: who(ROOT_ACTOR),
        capability: cap("cap_root"),
        operation: OperationName::new(operation).expect("operation"),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: Optional::Absent,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    };
    daemon.dispatch(&OperationRequest {
        envelope,
        arguments: arguments(operation),
    })
}

/// Everything a refused request could have changed, as one comparable value.
fn observable(daemon: &Daemon) -> String {
    format!("{:?}\n{:?}", daemon.state(), daemon.store_audit())
}

/// One minor below `since`. Every dated operation is dated at a minor above `.0`.
fn below(since: ProtocolVersion) -> ProtocolVersion {
    assert!(since.minor() > 0, "a dated operation is dated above x.0");
    ProtocolVersion::new(since.major(), since.minor() - 1)
}

const UNDECLARED: &str = "the request names an operation this protocol version does not declare";

fn dated() -> Vec<(&'static str, ProtocolVersion)> {
    OPERATIONS
        .iter()
        .filter_map(|spec| spec.since.map(|since| (spec.name, since)))
        .collect()
}

#[test]
fn every_dated_operation_is_driven_here() {
    // Eleven today: evidence.link (3.3), whiteboard.compile (3.5),
    // workspace.create_by_reference (3.6), and the eight of the signing wire (3.8).
    let dated = dated();
    assert_eq!(dated.len(), 11, "{dated:?}");
    for (operation, _) in dated {
        let _ = arguments(operation);
    }
}

#[test]
fn one_minor_below_its_date_every_operation_is_refused_and_changes_nothing() {
    for (operation, since) in dated() {
        let version = below(since);
        let mut daemon = daemon(version);
        let before = observable(&daemon);
        let outcome = call(&mut daemon, version, operation);
        let error = outcome
            .envelope
            .error
            .value()
            .unwrap_or_else(|| panic!("{operation} at {version} was served"));
        assert_eq!(
            error.code,
            ErrorCode::MalformedRequest,
            "{operation} at {version}: the code `rule signing.identities` names"
        );
        assert_eq!(error.detail, UNDECLARED, "{operation} at {version}");
        assert!(!error.retryable, "{operation} at {version}");
        assert!(
            daemon.state().admissions().is_empty(),
            "{operation} at {version}: refused before admission"
        );
        assert_eq!(
            observable(&daemon),
            before,
            "{operation} at {version}: a refused request changes nothing"
        );

        // The same key again, still below the date: nothing was recorded, so the answer is
        // the same refusal, never a replay or an `IdempotencyKeyReused`.
        let again = call(&mut daemon, version, operation);
        assert_eq!(
            again.envelope.error.value().map(|error| error.code),
            Some(ErrorCode::MalformedRequest),
            "{operation} at {version}: no ledger entry was written"
        );
        assert_eq!(observable(&daemon), before);
    }
}

#[test]
fn at_its_date_every_operation_reaches_admission() {
    for (operation, since) in dated() {
        let mut daemon = daemon(since);
        let outcome = call(&mut daemon, since, operation);
        if let Some(error) = outcome.envelope.error.value() {
            assert_ne!(
                error.detail, UNDECLARED,
                "{operation} at {since}: the version gate refused it at its own date"
            );
        }
        let admissions = daemon.state().admissions();
        assert_eq!(
            admissions.len(),
            1,
            "{operation} at {since}: the request passed the gate and reached admission"
        );
        assert_eq!(admissions[0].operation, operation);
    }
}

#[test]
fn an_undated_operation_is_served_at_every_version_of_the_major() {
    // The gate refuses nothing it should serve: an undated operation at 3.0.
    let version = ProtocolVersion::new(3, 0);
    let mut daemon = daemon(version);
    let envelope = RequestEnvelope {
        protocol_version: version,
        request_id: RequestId::new("req_undated").expect("request id"),
        idempotency_key: Optional::Absent,
        actor: who(ROOT_ACTOR),
        capability: cap("cap_root"),
        operation: OperationName::new("evidence.query").expect("operation"),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: Optional::Absent,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    };
    let arguments = decode_arguments(
        "evidence.query",
        &Opaque::from_bytes(br#"{"query":{}}"#.to_vec()),
    )
    .expect("the body decodes");
    let outcome = daemon.dispatch(&OperationRequest {
        envelope,
        arguments,
    });
    if let Some(error) = outcome.envelope.error.value() {
        assert_ne!(error.detail, UNDECLARED);
    }
    assert_eq!(daemon.state().admissions().len(), 1);
}

// =====================================================================================
// Dated fields on the answer (bn-7xz8v audit)
// =====================================================================================

#[test]
fn every_dated_field_has_named_enforcement() {
    // `idl_conformance.rs` holds `protocol::since::FIELDS` to the IDL. This holds each row
    // to the code that enforces it, so a new dated field fails here until someone decides
    // how the daemon keeps it off (or ignores it on) an older connection.
    let enforced: &[(&str, &str, &str)] = &[
        (
            "ResultEnvelope",
            "audit",
            "Daemon emitted_at (this file, correction 62)",
        ),
        (
            "CapabilityDescriptor",
            "profile",
            "CapabilityDescriptor::as_reported_at (this file)",
        ),
        (
            "CapabilityDescriptor",
            "instances",
            "CapabilityDescriptor::as_reported_at (this file, gate_g1_07)",
        ),
        (
            "SnapshotComponents",
            "file_components",
            "workspace::declared_placement and registration (workspace.rs unit tests)",
        ),
        (
            "EvidenceGetResponse",
            "signature",
            "evidence.get (daemon_signing.rs)",
        ),
    ];
    let tabled: Vec<(&str, &str)> = continuumd::protocol::since::FIELDS
        .iter()
        .map(|(owner, field, _)| (*owner, *field))
        .collect();
    let named: Vec<(&str, &str)> = enforced
        .iter()
        .map(|(owner, field, _)| (*owner, *field))
        .collect();
    assert_eq!(tabled, named, "every dated field names its enforcement");
}

#[test]
fn a_too_new_operation_whose_body_does_not_decode_gets_the_same_refusal() {
    // The transport decodes the body before dispatch, and a body that does not decode is
    // answered by `Daemon::refuse`. For an operation the version does not declare, that
    // answer is the dispatch gate's own: same code, same detail, no audit record.
    for (operation, since) in dated() {
        let version = below(since);
        let daemon = daemon(version);
        let envelope = RequestEnvelope {
            protocol_version: version,
            request_id: RequestId::new("req_undecodable").expect("request id"),
            idempotency_key: Optional::Present("k".to_owned()),
            actor: who(ROOT_ACTOR),
            capability: cap("cap_root"),
            operation: OperationName::new(operation).expect("operation"),
            snapshot: Nullable::Null,
            intent: Nullable::Null,
            arguments: Opaque::from_bytes(Vec::new()),
            budget: Optional::Absent,
            output_policy: Optional::Absent,
            trace: Optional::Absent,
            page: Optional::Absent,
        };
        let refused = daemon.refuse(&envelope, ErrorCode::MalformedRequest);
        let error = refused.error.value().expect("an error");
        assert_eq!(
            error.code,
            ErrorCode::MalformedRequest,
            "{operation} at {version}"
        );
        assert_eq!(error.detail, UNDECLARED, "{operation} at {version}");
        assert!(
            refused.audit.is_absent(),
            "{operation} at {version}: no audit"
        );
    }
}

/// A request the root capability does not present: always `CapabilityDenied`, which
/// `rule audit.correlation` requires to carry `audit` from 3.1.
fn denied_at(version: ProtocolVersion) -> (Daemon, OperationOutcome) {
    let mut daemon = daemon(version);
    let envelope = RequestEnvelope {
        protocol_version: version,
        request_id: RequestId::new("req_denied").expect("request id"),
        idempotency_key: Optional::Absent,
        actor: who("agent:stranger"),
        capability: cap("cap_stranger"),
        operation: OperationName::new("evidence.query").expect("operation"),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: Optional::Absent,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    };
    let arguments = decode_arguments(
        "evidence.query",
        &Opaque::from_bytes(br#"{"query":{}}"#.to_vec()),
    )
    .expect("the body decodes");
    let outcome = daemon.dispatch(&OperationRequest {
        envelope,
        arguments,
    });
    (daemon, outcome)
}

#[test]
fn the_audit_correlation_is_recorded_but_not_emitted_below_its_date() {
    // `ResultEnvelope.audit` is `@since("3.1")`. `rule versioning.compatible_change`
    // governs, and `rule audit.correlation` applies from 3.1 (RFC 0026 correction 62).
    let (daemon, at_3_0) = denied_at(ProtocolVersion::new(3, 0));
    assert_eq!(
        at_3_0.envelope.error.value().map(|error| error.code),
        Some(ErrorCode::CapabilityDenied)
    );
    assert!(
        at_3_0.envelope.audit.is_absent(),
        "3.0 does not define `audit`"
    );
    assert_eq!(
        daemon.state().admissions().len(),
        1,
        "the denial is still recorded daemon-side"
    );
    assert!(!daemon.state().admissions()[0].audit.is_empty());

    let (_, at_3_1) = denied_at(ProtocolVersion::new(3, 1));
    assert!(
        !at_3_1.envelope.audit.is_absent(),
        "3.1 defines it, and a denial carries it (`rule audit.correlation`)"
    );
}

#[test]
fn the_grant_report_keeps_each_dated_field_off_a_connection_below_its_date() {
    use continuumd::protocol::scalar::ArtifactHandle;

    let full = CapabilityDescriptor {
        capability: cap("cap_x"),
        actor: who("agent:x"),
        level: AuthorityLevel::Read,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: 0,
        profile: Optional::Present(CapabilityProfile {
            privileged_operations: Vec::new(),
            denied_operations: Vec::new(),
            data_grants: Vec::new(),
            cross_principal_sharing: false,
        }),
        instances: Optional::Present(vec![ArtifactHandle::new("ev_x").expect("a handle")]),
    };
    let at = |minor| full.as_reported_at(ProtocolVersion::new(3, minor));
    // `profile` is `@since("3.1")`, `instances` `@since("3.7")`.
    assert!(at(0).profile.is_absent() && at(0).instances.is_absent());
    assert!(!at(1).profile.is_absent() && at(1).instances.is_absent());
    assert!(!at(6).profile.is_absent() && at(6).instances.is_absent());
    assert!(!at(7).profile.is_absent() && !at(7).instances.is_absent());
}

// =====================================================================================
// Dated request fields are ignored below their date (bn-7xz8v, cr-88az2y)
// =====================================================================================

/// Every `(operation, owner, field)` where a dated field is reachable from the operation's
/// request body, walked over the registry's struct specs (held to the IDL by
/// `idl_conformance.rs`). `RequestEnvelope` is walked as the pseudo-operation `envelope`.
fn dated_request_fields() -> std::collections::BTreeSet<(String, String, String)> {
    use continuumd::codec::versioned::edges;
    use continuumd::protocol::spec::ProtocolStruct;
    use std::collections::BTreeSet;

    // The walk uses the codec's own type graph (`codec::versioned::edges`: struct fields
    // and union variants), so the test and the skip cannot disagree about what a request
    // reaches. The field names come from the specs.
    let roots: Vec<(String, &str)> = OPERATIONS
        .iter()
        .map(|spec| (spec.name.to_owned(), spec.request.name))
        .chain([(
            "envelope".to_owned(),
            <RequestEnvelope as ProtocolStruct>::STRUCT_NAME,
        )])
        .collect();
    let mut found = BTreeSet::new();
    for (operation, root) in roots {
        let mut seen = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(ty) = pending.pop() {
            if !seen.insert(ty) {
                continue;
            }
            if let Some(fields) = continuumd::codec::versioned::structs().get(ty) {
                for field in *fields {
                    if continuumd::protocol::since::FIELDS
                        .iter()
                        .any(|(owner, name, _)| *owner == ty && *name == field.name)
                    {
                        found.insert((operation.clone(), ty.to_owned(), field.name.to_owned()));
                    }
                }
            }
            pending.extend(edges(ty));
        }
    }
    found
}

#[test]
fn every_dated_request_field_is_ignored_below_its_date() {
    // Coverage by construction: the codec's skip set is `protocol::since::FIELDS`, found
    // through the same specs (`codec::versioned`). This pins which request paths reach a
    // dated field, and drives each one with a value of the wrong type one minor below its
    // date (ignored) and at its date (refused), in both encodings. A newly dated request
    // field fails the first assertion until a body for it is added below.
    let reached = dated_request_fields();
    let expected: Vec<(String, String, String)> = vec![(
        "workspace.create".to_owned(),
        "SnapshotComponents".to_owned(),
        "file_components".to_owned(),
    )];
    assert_eq!(reached.into_iter().collect::<Vec<_>>(), expected);

    for (operation, owner, field) in &expected {
        let since = continuumd::protocol::since::FIELDS
            .iter()
            .find(|(o, f, _)| o == owner && f == field)
            .map(|(_, _, at)| *at)
            .expect("dated");
        assert!(
            continuumd::codec::versioned::reaching().contains(owner.as_str()),
            "the skip reaches {owner}"
        );
        let body = match (operation.as_str(), field.as_str()) {
            ("workspace.create", "file_components") => {
                br#"{"components":{"cml_modules":[],"configuration":[],"correspondence":[],"dependencies":[],"domain_packs":[],"epochs":{"proof":"p","semantic":"s"},"file_components":42,"files":[],"intent":"in_x","proof_environment":[],"rust_extraction":[]}}"#
                    .to_vec()
            }
            other => panic!("no garbage body for {other:?}"),
        };
        let json = Opaque::from_bytes(body.clone());
        let tree = continuumd::codec::json::Json::parse(&body).expect("well-framed");
        let cbor_bytes = continuumd::codec::Document::to_canonical_bytes(&to_cbor(&tree));
        let cbor = Opaque::from_bytes(cbor_bytes);
        let below = below(since);
        use continuumd::codec::cbor::Cbor;
        use continuumd::codec::json::Json;
        use continuumd::codec::operations::decode_arguments_at;
        assert!(
            decode_arguments_at::<Json>(operation, &json, below).is_ok(),
            "{operation} at {below}: {field} is ignored (json)"
        );
        assert!(
            decode_arguments_at::<Cbor>(operation, &cbor, below).is_ok(),
            "{operation} at {below}: {field} is ignored (cbor)"
        );
        assert_eq!(
            decode_arguments_at::<Json>(operation, &json, since)
                .map(|_| ())
                .map_err(|e| e.code()),
            Err(ErrorCode::MalformedRequest),
            "{operation} at {since}: {field} is decoded strictly"
        );
        assert!(decode_arguments_at::<Cbor>(operation, &cbor, since).is_err());
        // A broken frame is refused at every version: at the end of the body, and inside
        // the very field that is skipped below its date.
        let truncated = Opaque::from_bytes(body[..body.len() - 3].to_vec());
        let text = String::from_utf8(body.clone()).expect("UTF-8");
        let needle = format!("\"{field}\":42,");
        assert_eq!(text.matches(&needle).count(), 1);
        let inside = Opaque::from_bytes(
            text.replace(&needle, &format!("\"{field}\":[42,"))
                .into_bytes(),
        );
        for broken in [&truncated, &inside] {
            assert!(decode_arguments_at::<Json>(operation, broken, below).is_err());
            assert!(decode_arguments_at::<Json>(operation, broken, since).is_err());
        }
    }
}

/// The same document in canonical CBOR, built through the `Document` model. The bodies
/// here carry only objects, arrays, strings, and integers.
fn to_cbor(json: &continuumd::codec::json::Json) -> continuumd::codec::cbor::Cbor {
    use continuumd::codec::Document;
    use continuumd::codec::cbor::Cbor;
    if let Some(entries) = json.as_entries() {
        return Cbor::from_entries(
            entries
                .iter()
                .map(|(key, value)| (key.clone(), to_cbor(value)))
                .collect(),
        );
    }
    if let Some(items) = json.as_items() {
        return Cbor::from_items(items.iter().map(to_cbor).collect());
    }
    if let Some(text) = json.as_text() {
        return Cbor::from_text(text);
    }
    Cbor::from_unsigned(json.as_integer("integer").expect("an integer"))
}

// =====================================================================================
// The N−1 major: a negotiation window with no surface (RFC 0026 correction 62)
// =====================================================================================

#[test]
fn the_current_major_base_is_the_idls_current_major() {
    let version: ProtocolVersion = continuumd::protocol::registry::PROTOCOL_VERSION
        .parse()
        .expect("major.minor");
    assert_eq!(
        continuumd::protocol::since::CURRENT_MAJOR_BASE,
        ProtocolVersion::new(version.major(), 0)
    );
    assert_eq!(
        continuumd::protocol::registry::MAJORS_SERVED,
        &[version.major(), version.major() - 1]
    );
}

#[test]
fn a_2_x_connection_negotiates() {
    // The window admits 2.x (`majors_served = [3, 2]`): the handshake is served.
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: ProtocolVersion::new(2, 0),
            high: ProtocolVersion::new(2, 9),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-version-gate".to_owned(),
        actor: who(ROOT_ACTOR),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    let negotiated = negotiate(
        &[ProtocolVersion::new(2, 9), ProtocolVersion::new(3, 9)],
        ProtocolWindow::new(3),
        ENCODINGS,
        &hello,
    )
    .expect("2.x is inside the window");
    assert_eq!(negotiated.protocol_version(), ProtocolVersion::new(2, 9));
}

#[test]
fn on_a_2_x_connection_every_operation_is_refused_as_undeclared_and_changes_nothing() {
    let version = ProtocolVersion::new(2, 9);
    // Every operation this file can build a body for: undated and dated alike.
    for operation in dated()
        .into_iter()
        .map(|(operation, _)| operation)
        .chain(["evidence.query"])
    {
        let mut daemon = daemon(version);
        let before = observable(&daemon);
        let outcome = if operation == "evidence.query" {
            let envelope = RequestEnvelope {
                protocol_version: version,
                request_id: RequestId::new("req_two").expect("request id"),
                idempotency_key: Optional::Absent,
                actor: who(ROOT_ACTOR),
                capability: cap("cap_root"),
                operation: OperationName::new(operation).expect("operation"),
                snapshot: Nullable::Null,
                intent: Nullable::Null,
                arguments: Opaque::from_bytes(Vec::new()),
                budget: Optional::Absent,
                output_policy: Optional::Absent,
                trace: Optional::Absent,
                page: Optional::Absent,
            };
            daemon.dispatch(&OperationRequest {
                envelope,
                arguments: decode_arguments(
                    operation,
                    &Opaque::from_bytes(br#"{"query":{}}"#.to_vec()),
                )
                .expect("decodes"),
            })
        } else {
            call(&mut daemon, version, operation)
        };
        let error = outcome.envelope.error.value().expect("refused");
        assert_eq!(
            error.code,
            ErrorCode::MalformedRequest,
            "{operation} at 2.9"
        );
        assert_eq!(error.detail, UNDECLARED, "{operation} at 2.9");
        // No dated field: `audit` (3.1) is absent. No recovery offer.
        assert!(
            outcome.envelope.audit.is_absent(),
            "{operation}: no `audit` at 2.x"
        );
        assert!(
            outcome.recovery.is_empty(),
            "{operation}: no recovery offer at 2.x"
        );
        assert!(error.recovery.is_empty());
        assert!(
            daemon.state().admissions().is_empty(),
            "{operation}: before admission"
        );
        assert_eq!(observable(&daemon), before, "{operation}: nothing changed");
    }
}

#[test]
fn on_a_2_x_connection_nothing_in_the_idl_is_defined() {
    use continuumd::protocol::since::{DECLARATIONS, ENUM_MEMBERS, FIELDS, defines};
    let version = ProtocolVersion::new(2, 9);
    assert!(OPERATIONS.iter().all(|spec| !spec.defined_at(version)));
    assert!(FIELDS.iter().all(|(_, _, at)| !defines(Some(*at), version)));
    assert!(
        ENUM_MEMBERS
            .iter()
            .all(|(_, _, at)| !defines(Some(*at), version))
    );
    assert!(
        DECLARATIONS
            .iter()
            .all(|(_, at)| !defines(Some(*at), version))
    );
    // Undated items too: the IDL's undated surface is `3.0`'s.
    assert!(!defines(None, version));
    // So the error-code emission gate defines no code, dated or not.
    for code in ErrorCode::ALL {
        assert!(
            !continuumd::daemon::errors::defined_at(*code, version),
            "{code:?}"
        );
    }
}

#[test]
fn the_2_x_handshake_frames_follow_the_handshake_rules() {
    use continuumd::protocol::handshake::NegotiationError;
    use continuumd::protocol::handshake::ServerReject;
    // The welcome's grant carries no dated field on a 2.x connection.
    let grant = CapabilityDescriptor {
        capability: cap("cap_x"),
        actor: who("agent:x"),
        level: AuthorityLevel::Read,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: 0,
        profile: Optional::Present(CapabilityProfile {
            privileged_operations: Vec::new(),
            denied_operations: Vec::new(),
            data_grants: Vec::new(),
            cross_principal_sharing: false,
        }),
        instances: Optional::Present(Vec::new()),
    };
    let reported = grant.as_reported_at(ProtocolVersion::new(2, 9));
    assert!(reported.profile.is_absent() && reported.instances.is_absent());

    // A refusal frame is gated on the offer reaching 3.1 (`rule handshake.rejection`):
    // a 2.x-only offer gets none, an offer reaching 3.1 does.
    let offer = |low: (u32, u32), high: (u32, u32)| ClientHello {
        protocol_versions: VersionRange {
            low: ProtocolVersion::new(low.0, low.1),
            high: ProtocolVersion::new(high.0, high.1),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "c".to_owned(),
        actor: who(ROOT_ACTOR),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    let refuse = |hello: &ClientHello| {
        ServerReject::for_negotiation(
            NegotiationError::NoCommonVersion,
            hello,
            &[3, 2],
            "no protocol version is common to client and daemon",
        )
    };
    assert!(refuse(&offer((2, 0), (2, 9))).is_none());
    assert!(refuse(&offer((2, 0), (3, 1))).is_some());
}
