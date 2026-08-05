//! The canonical codec, from outside the crate: the three rules, the round trip, the
//! golden vectors, and the mutation sweep.
//!
//! # What these tests are evidence for
//!
//! `rule conformance.golden_traces` asks for something specific, and it is worth quoting
//! because it is what distinguishes this file from a serialization test:
//!
//! > A golden vector is a byte sequence, not a shape: the JSON and CBOR vectors for one
//! > exchange MUST decode to the same values, and re-encoding either MUST reproduce it
//! > byte for byte. A vector that only round-trips through a permissive parser is not a
//! > golden vector.
//! >
//! > — RFC 0026, "Golden wire vectors"
//!
//! So three separate claims are asserted here, and the third is the one a permissive
//! parser would fail:
//!
//! 1. **round trip** — `decode(encode(v)) == v` for every value the families carry;
//! 2. **golden bytes** — `encode(v)` is a literal byte sequence written into this file, so
//!    a change to the field order, the escape spelling, the integer form, or the union tag
//!    is a failing diff rather than a silent re-spelling;
//! 3. **injectivity under mutation** — for every single-byte mutation of a golden vector,
//!    the bytes either fail to decode or decode to a *different* value, and never to the
//!    same one. That is what "canonical" means operationally: one message, one spelling.
//!    A permissive parser fails it immediately, because it accepts whitespace, a leading
//!    zero, or a `+` as a second spelling of a value it already has.
//!
//! The vectors cover the five exchanges the transport bone named — the handshake, a
//! `workspace.create` request, a `verification.start` request, a denial result, and a task
//! result — plus the F19 exchange the 3.4 bundle added: a `CertificateRejected` result
//! whose `error.data` carries the first declared `Error.data` shape (bn-3jrtz).

use continuumd::codec::json::{Json, JsonError, MAX_EXACT_INTEGER, base64url, from_base64url};
use continuumd::codec::{CodecError, ProtocolValue, from_bytes, to_bytes};
use continuumd::protocol::envelope::{
    ArtifactRef, AssuranceEnvelope, Budget, Cost, EnvelopeDimension, EpochSet, Error, Omission,
    ProducedDimension, RequestEnvelope, ResultEnvelope, SemanticVerdictValue,
    StructuralVerdictValue, UnsupportedDimension, Verdict,
};
use continuumd::protocol::handshake::{ClientHello, ServerReject, VersionRange};
use continuumd::protocol::operations::verification::VerificationStartRequest;
use continuumd::protocol::operations::workspace::WorkspaceCreateRequest;
use continuumd::protocol::scalar::{
    ActorId, ArtifactHandle, AuditCorrelationId, ByteCount, CapabilityHandle, Commitment,
    ContinuationHandle, DurationMs, EpochIdentity, IntentHandle, Opaque, OperationName,
    ProtocolVersion, RequestId, TaskHandle,
};
use continuumd::protocol::shared::{FileComponent, SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, Optional};
use continuumd::protocol::vocabulary::{
    AssuranceClass, Encoding, ErrorCode, OmissionReason, Portfolio, PriorityClass, ResultStatus,
    SemanticVerdict, StructuralOutcome, TargetKind,
};

// --- fixtures -----------------------------------------------------------------------

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 2)
}

fn commitment(text: &str) -> Commitment {
    Commitment::new(text)
}

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
}

fn hello() -> ClientHello {
    ClientHello {
        protocol_versions: VersionRange {
            low: ProtocolVersion::new(3, 0),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson, Encoding::CanonicalCbor],
        client: "continuum-cli/0".to_owned(),
        actor: ActorId::new("agent:builder").expect("a well-formed actor"),
        capability: CapabilityHandle::new("cap_builder").expect("a well-formed capability"),
        features: Optional::Absent,
    }
}

fn components() -> SnapshotComponents {
    SnapshotComponents {
        files: vec![commitment("ws_aaa"), commitment("ws_bbb")],
        cml_modules: Vec::new(),
        rust_extraction: Vec::new(),
        domain_packs: Vec::new(),
        dependencies: Vec::new(),
        epochs: SnapshotEpochs {
            semantic: epoch("semantic-1"),
            proof: epoch("proof-1"),
            toolchain: Optional::Absent,
        },
        intent: IntentHandle::new("in_diehard").expect("a well-formed intent handle"),
        correspondence: Vec::new(),
        proof_environment: Vec::new(),
        configuration: Vec::new(),
        file_components: Optional::Present(vec![
            FileComponent {
                path: "DieHard.ctm".to_owned(),
                commitment: commitment("ws_aaa"),
            },
            FileComponent {
                path: "README.md".to_owned(),
                commitment: commitment("ws_bbb"),
            },
        ]),
    }
}

fn create_request() -> WorkspaceCreateRequest {
    WorkspaceCreateRequest {
        components: components(),
        overlay: Optional::Absent,
        seal: Optional::Present(true),
    }
}

fn start_request() -> VerificationStartRequest {
    VerificationStartRequest {
        target: Target {
            kind: TargetKind::AllClaims,
            id: "DieHard".to_owned(),
        },
        portfolio: Portfolio::Interactive,
        context_policy: Optional::Absent,
        priority_class: Optional::Present(PriorityClass::Interactive),
    }
}

fn request_envelope(operation: &str, arguments: Opaque) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new("req_1").expect("a well-formed request id"),
        idempotency_key: Optional::Present("idem-1".to_owned()),
        actor: ActorId::new("agent:builder").expect("a well-formed actor"),
        capability: CapabilityHandle::new("cap_builder").expect("a well-formed capability"),
        operation: OperationName::new(operation).expect("a well-formed operation name"),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments,
        budget: Optional::Present(Budget {
            wall_ms: Optional::Absent,
            cpu_ms: Optional::Absent,
            memory_bytes: Optional::Absent,
            states: Optional::Present(64),
            solver_ms: Optional::Absent,
            proof_ms: Optional::Absent,
            tokens: Optional::Absent,
            candidates: Optional::Absent,
            bytes: Optional::Absent,
        }),
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

fn unmeasured() -> Cost {
    Cost {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(16),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
        tokenizer_id: Optional::Absent,
    }
}

fn epochs() -> EpochSet {
    EpochSet {
        protocol: version(),
        semantic: Nullable::Value(epoch("semantic-1")),
        intent: Nullable::Null,
        evidence: Nullable::Null,
        proof: Nullable::Null,
        corpus: Nullable::Null,
        engine: Nullable::Value(epoch("engine-reference-1")),
    }
}

/// The one answer to every admission failure, as a wire message.
fn denial() -> ResultEnvelope {
    ResultEnvelope {
        request_id: RequestId::new("req_1").expect("a well-formed request id"),
        status: ResultStatus::Error,
        verdict: Nullable::Null,
        error: Optional::Present(Error {
            code: ErrorCode::CapabilityDenied,
            detail: "the presented capability does not admit this operation".to_owned(),
            data: Optional::Absent,
            recovery: Vec::new(),
            continuation: Optional::Absent,
            non_resumable_reason: Optional::Absent,
            retryable: false,
        }),
        assurance: Optional::Absent,
        artifacts: Vec::new(),
        task: Optional::Absent,
        continuation: Optional::Absent,
        omissions: Vec::new(),
        warnings: Vec::new(),
        cost: Cost {
            states: Optional::Absent,
            ..unmeasured()
        },
        epochs: epochs(),
        next_operations: Vec::new(),
        next_page_token: Optional::Absent,
        payload: Nullable::Null,
        audit: Optional::Present(
            AuditCorrelationId::new("audit-1").expect("a well-formed correlation identity"),
        ),
    }
}

/// A `CertificateRejected` result carrying the declared `Error.data` shape — the F19
/// exchange (RFC 0026, protocol 3.4, bn-3jrtz).
///
/// The one vector whose envelope is pinned at `3.4` rather than this file's `3.2`,
/// because the shape it exercises is `@since("3.4")`: `data` is the canonical encoding of
/// a `CertificateRejection`, spliced exactly where `rule encoding.opaque_payloads` puts
/// it, resolved by the carrying object's own `code`. The tokens are the core kernel's own
/// spellings for a trailing byte — the fixture `daemon_evidence.rs` rejects for real.
fn certificate_rejected_result() -> ResultEnvelope {
    ResultEnvelope {
        request_id: RequestId::new("req_3").expect("a well-formed request id"),
        status: ResultStatus::Error,
        verdict: Nullable::Null,
        error: Optional::Present(Error {
            code: ErrorCode::CertificateRejected,
            detail: "continuum-kernel-core rejected this certificate: the verdict is that \
                     kernel's own, over the bytes the daemon holds"
                .to_owned(),
            data: Optional::Present(
                continuumd::codec::to_opaque(
                    &continuumd::protocol::envelope::CertificateRejection {
                        checker: "continuum-kernel-core".to_owned(),
                        reason: "trailing-bytes".to_owned(),
                        field: Optional::Absent,
                    },
                )
                .expect("the declared shape encodes"),
            ),
            recovery: Vec::new(),
            continuation: Optional::Absent,
            non_resumable_reason: Optional::Absent,
            retryable: false,
        }),
        assurance: Optional::Absent,
        artifacts: Vec::new(),
        task: Optional::Absent,
        continuation: Optional::Absent,
        omissions: Vec::new(),
        warnings: Vec::new(),
        cost: Cost {
            states: Optional::Absent,
            ..unmeasured()
        },
        epochs: EpochSet {
            protocol: ProtocolVersion::new(3, 4),
            ..epochs()
        },
        next_operations: Vec::new(),
        next_page_token: Optional::Absent,
        payload: Nullable::Null,
        audit: Optional::Absent,
    }
}

/// A suspended task result: the lane bn-i4aem item 9 made reachable, with the
/// nine-dimension envelope its semantic verdict obliges.
fn task_result() -> ResultEnvelope {
    let produced = EnvelopeDimension::Produced(ProducedDimension {
        engine: "continuum-engine-reference".to_owned(),
        summary: "exact finite reachability".to_owned(),
    });
    let unsupported = EnvelopeDimension::Unsupported(UnsupportedDimension {
        reason: "sequential-consistency-only".to_owned(),
    });
    ResultEnvelope {
        request_id: RequestId::new("req_2").expect("a well-formed request id"),
        status: ResultStatus::TaskSuspended,
        verdict: Nullable::Value(Verdict::Semantic(SemanticVerdictValue {
            verdict: SemanticVerdict::Inconclusive,
            inconclusive_reason: Optional::Present(
                continuumd::protocol::vocabulary::InconclusiveReason::ResourceExhausted,
            ),
            assurance_class: AssuranceClass::Observed,
        })),
        error: Optional::Absent,
        assurance: Optional::Present(AssuranceEnvelope {
            bounds: produced.clone(),
            faults: unsupported.clone(),
            fairness: unsupported.clone(),
            values: produced.clone(),
            schedules: unsupported.clone(),
            memory_model: unsupported.clone(),
            observer: unsupported.clone(),
            proof_status: unsupported.clone(),
            unknowns: unsupported,
        }),
        artifacts: vec![ArtifactRef {
            kind: "task".to_owned(),
            handle: ArtifactHandle::new("task_die_hard").expect("a well-formed artifact handle"),
            commitment: Optional::Absent,
            redacted: Optional::Absent,
        }],
        task: Optional::Present(TaskHandle::new("task_die_hard").expect("a well-formed handle")),
        continuation: Optional::Present(
            ContinuationHandle::new("cont_die_hard").expect("a well-formed handle"),
        ),
        omissions: vec![Omission {
            reason: OmissionReason::Unsupported,
            subject: "cost.wall_ms".to_owned(),
            recoverable_by: Optional::Absent,
        }],
        warnings: Vec::new(),
        cost: unmeasured(),
        epochs: epochs(),
        next_operations: Vec::new(),
        next_page_token: Optional::Absent,
        payload: Nullable::Null,
        audit: Optional::Absent,
    }
}

fn reject() -> ServerReject {
    ServerReject {
        code: ErrorCode::ProtocolVersionUnsupported,
        detail: "the offered major is outside the served N/N−1 window".to_owned(),
        retryable: false,
        majors_served: vec![3, 2],
    }
}

// --- the three canonical-form rules -------------------------------------------------

#[test]
fn object_keys_are_written_in_code_point_order_whatever_order_they_were_built_in() {
    // `rule encoding.canonical_form`: "Field order is ascending Unicode code-point order
    // of the field name". The declaration order of `ClientHello` is
    // protocol_versions, encodings, client, actor, capability, features — nothing like
    // alphabetical — so this is a real test of the rule rather than of a coincidence.
    let bytes = to_bytes(&hello()).expect("the hello encodes");
    let text = String::from_utf8(bytes).expect("canonical output is UTF-8");
    let keys: Vec<&str> = [
        "actor",
        "capability",
        "client",
        "encodings",
        "protocol_versions",
    ]
    .into_iter()
    .collect();
    let mut at = 0;
    for key in &keys {
        let quoted = format!("\"{key}\":");
        let found = text[at..]
            .find(&quoted)
            .unwrap_or_else(|| panic!("`{key}` is present"));
        at += found + quoted.len();
    }
    // `features` is `optional` and absent, so it is omitted rather than null — the other
    // half of the presence rule, asserted here because an encoder that wrote `null` would
    // still order its keys correctly.
    assert!(!text.contains("features"));
}

#[test]
fn a_union_is_one_key_named_by_its_declared_variant() {
    // `rule encoding.union_tagging`. `refuted` is a member of both `SemanticVerdict` and
    // `EvaluationVerdict`, and the tag is what disambiguates them (RFC 0026, "Verdicts").
    let semantic = Verdict::Semantic(SemanticVerdictValue {
        verdict: SemanticVerdict::Refuted,
        inconclusive_reason: Optional::Absent,
        assurance_class: AssuranceClass::Validated,
    });
    let bytes = to_bytes(&semantic).expect("the verdict encodes");
    assert_eq!(
        String::from_utf8(bytes.clone()).expect("UTF-8"),
        r#"{"semantic":{"assurance_class":"validated","verdict":"refuted"}}"#
    );
    assert_eq!(
        from_bytes::<Verdict>(&bytes).expect("it decodes"),
        semantic,
        "the tag round-trips"
    );

    // A structural verdict carrying the *same* token would be a different message.
    let structural = Verdict::Structural(StructuralVerdictValue {
        outcome: StructuralOutcome::Rejected,
    });
    assert_ne!(
        to_bytes(&structural).expect("it encodes"),
        bytes,
        "two verdict families are two messages"
    );

    // Zero keys, two keys, and an undeclared key are each malformed.
    for spelling in [
        r#"{}"#,
        r#"{"semantic":{"assurance_class":"validated","verdict":"refuted"},"structural":{"outcome":"rejected"}}"#,
    ] {
        assert_eq!(
            from_bytes::<Verdict>(spelling.as_bytes()).expect_err("arity"),
            CodecError::UnionArity { union: "Verdict" }
        );
    }
    assert_eq!(
        from_bytes::<Verdict>(br#"{"conjectural":{"outcome":"rejected"}}"#)
            .expect_err("an undeclared variant"),
        CodecError::UnknownVariant { union: "Verdict" }
    );
}

#[test]
fn an_opaque_payload_is_the_operations_own_struct_and_is_carried_verbatim() {
    // `rule encoding.opaque_payloads`. The request struct goes into the envelope's
    // `arguments`, and reading it back out needs nothing but the envelope itself: its
    // `operation` field is `required` and names the shape.
    let arguments = continuumd::codec::to_opaque(&create_request()).expect("the body encodes");
    let envelope = request_envelope("workspace.create", arguments);
    let bytes = to_bytes(&envelope).expect("the envelope encodes");
    let decoded: RequestEnvelope = from_bytes(&bytes).expect("the envelope decodes");
    assert_eq!(decoded, envelope);

    let body: WorkspaceCreateRequest =
        continuumd::codec::from_opaque(&decoded.arguments).expect("the body decodes");
    assert_eq!(body, create_request());

    // The payload is spliced into the enclosing document rather than escaped into a
    // string: it is a canonical value, and the canonical form of the whole message
    // applies to it.
    let text = String::from_utf8(bytes).expect("UTF-8");
    assert!(text.contains(r#""arguments":{"components":{"#), "{text}");
}

// --- round trip ---------------------------------------------------------------------

/// Assert `decode(encode(v)) == v` and that re-encoding reproduces the same bytes.
fn round_trip<T: ProtocolValue + PartialEq + core::fmt::Debug>(value: &T) -> Vec<u8> {
    let bytes = to_bytes(value).expect("the value encodes");
    let decoded: T = from_bytes(&bytes).expect("the value decodes");
    assert_eq!(&decoded, value, "decode(encode(v)) == v");
    let again = to_bytes(&decoded).expect("the decoded value re-encodes");
    assert_eq!(again, bytes, "encode(decode(b)) == b");
    bytes
}

#[test]
fn every_representative_message_round_trips_through_the_codec() {
    round_trip(&hello());
    round_trip(&reject());
    round_trip(&create_request());
    round_trip(&start_request());
    round_trip(&components());
    round_trip(&denial());
    round_trip(&task_result());
    round_trip(&request_envelope(
        "workspace.create",
        continuumd::codec::to_opaque(&create_request()).expect("the body encodes"),
    ));
    round_trip(&request_envelope(
        "verification.start",
        continuumd::codec::to_opaque(&start_request()).expect("the body encodes"),
    ));
}

#[test]
fn presence_is_three_valued_in_both_directions() {
    // "Absent and null are distinct and MUST NOT be conflated in either direction"
    // (RFC 0026). Four assertions, one per way to conflate them.
    let mut envelope = request_envelope("workspace.create", Opaque::from_bytes(b"{}".to_vec()));
    envelope.idempotency_key = Optional::Absent;
    let text = String::from_utf8(to_bytes(&envelope).expect("encodes")).expect("UTF-8");
    assert!(
        !text.contains("idempotency_key"),
        "an absent `optional` field is omitted, never null"
    );
    assert!(
        text.contains(r#""intent":null"#),
        "a `nullable` field is present and null, never omitted"
    );

    // An `optional` field spelled `null` is malformed.
    let with_null = text.replace(
        r#""intent":null"#,
        r#""idempotency_key":null,"intent":null"#,
    );
    assert_eq!(
        from_bytes::<RequestEnvelope>(with_null.as_bytes()).expect_err("null optional"),
        CodecError::UnexpectedNull {
            declared_by: "RequestEnvelope",
            field: "idempotency_key",
        }
    );

    // A `nullable` field omitted is malformed.
    let without = text.replace(r#""intent":null,"#, "");
    assert_eq!(
        from_bytes::<RequestEnvelope>(without.as_bytes()).expect_err("omitted nullable"),
        CodecError::MissingField {
            declared_by: "RequestEnvelope",
            field: "intent",
        }
    );
}

#[test]
fn an_unknown_optional_field_is_ignored_and_an_unknown_closed_enum_member_is_not() {
    // `rule envelope.unknown_fields` is the protocol's only forward-compatibility
    // mechanism, and `rule versioning.enums` is where it stops.
    let text = String::from_utf8(to_bytes(&start_request()).expect("encodes")).expect("UTF-8");
    let widened = text.replace('{', r#"{"a_field_from_a_later_minor":7,"#);
    assert_eq!(
        from_bytes::<VerificationStartRequest>(widened.as_bytes()).expect("ignored"),
        start_request(),
        "an unknown field is ignored, and changes nothing"
    );

    // `Portfolio` is closed: an unrecognized member is malformed.
    let closed = text.replace(r#""portfolio":"interactive""#, r#""portfolio":"whatever""#);
    assert_eq!(
        from_bytes::<VerificationStartRequest>(closed.as_bytes()).expect_err("closed enum"),
        CodecError::UnknownMember {
            enum_name: "Portfolio",
        }
    );

    // `TargetKind` is `@open`: the token is surfaced verbatim rather than rejected, and
    // the answer is `UnsupportedSemanticFeature` — admissible for every operation as of
    // protocol 3.2.
    let open = text.replace(r#""kind":"all_claims""#, r#""kind":"all_conjectures""#);
    let error = from_bytes::<VerificationStartRequest>(open.as_bytes()).expect_err("open enum");
    assert_eq!(
        error,
        CodecError::UnknownOpenMember {
            enum_name: "TargetKind",
            token: "all_conjectures".into(),
        }
    );
    assert_eq!(error.code(), ErrorCode::UnsupportedSemanticFeature);
}

#[test]
fn u64_has_one_spelling_per_magnitude_and_no_second_one() {
    // The IDL's `U64` rule, both directions. A number inside the exactly-representable
    // range is a JSON number; one outside it is a decimal string; and each rejects the
    // other's spelling, because two spellings of one value is what a canonical form
    // exists to prevent.
    assert_eq!(
        to_bytes(&ByteCount::new(MAX_EXACT_INTEGER)).expect("encodes"),
        MAX_EXACT_INTEGER.to_string().into_bytes()
    );
    let beyond = MAX_EXACT_INTEGER + 1;
    assert_eq!(
        to_bytes(&ByteCount::new(beyond)).expect("encodes"),
        format!("\"{beyond}\"").into_bytes()
    );
    assert_eq!(
        from_bytes::<ByteCount>(format!("\"{MAX_EXACT_INTEGER}\"").as_bytes())
            .expect_err("a string inside the exact range is a second spelling"),
        CodecError::IntegerRange { expected: "U64" }
    );
    assert_eq!(
        Json::parse(beyond.to_string().as_bytes())
            .expect_err("a number beyond the exact range is malformed"),
        JsonError::IntegerRange { at: 0 }
    );
    // Round trip through the type, both sides of the boundary.
    for value in [0, 1, MAX_EXACT_INTEGER, beyond, u64::MAX] {
        round_trip(&DurationMs::new(value));
    }
}

#[test]
fn the_reader_rejects_every_second_spelling_the_canonical_form_excludes() {
    // Each of these is a document some JSON parser accepts and this one must not, because
    // accepting it would give one value two byte spellings.
    let cases: &[(&str, &str)] = &[
        (r#"{ "a":1 }"#, "insignificant whitespace"),
        (r#"{"a":01}"#, "a leading zero"),
        (r#"{"a":-1}"#, "a sign"),
        (r#"{"a":1.0}"#, "a fraction"),
        (r#"{"a":1e3}"#, "an exponent"),
        (r#"{"a":1,"a":2}"#, "a duplicate key"),
        (r#"{"b":1,"a":2}"#, "keys out of canonical order"),
        (r#"{"a":1}{"b":2}"#, "trailing bytes"),
        ("{\"a\":\"\u{1}\"}", "an unescaped control"),
        (r#"{"a":"\x41"}"#, "an undefined escape"),
    ];
    for (spelling, why) in cases {
        assert!(
            Json::parse(spelling.as_bytes()).is_err(),
            "{why} must be rejected: {spelling}"
        );
    }
    // Escapes the reader *does* accept, and normalizes to one spelling on the way out.
    let parsed = Json::parse(r#"{"a":"A\/\u00e9"}"#.as_bytes()).expect("legal escapes parse");
    assert_eq!(parsed.to_canonical_bytes(), r#"{"a":"A/é"}"#.as_bytes());
}

#[test]
fn bytes_are_unpadded_base64url_with_one_spelling() {
    for case in [
        &b""[..],
        b"f",
        b"fo",
        b"foo",
        b"foob",
        &[0x00, 0xFF, 0x10, 0x83, 0x7A][..],
    ] {
        let text = base64url(case);
        assert!(!text.contains('='), "unpadded");
        assert_eq!(from_base64url(&text).expect("decodes"), case);
    }
    // A final quantum whose unused low bits are non-zero is a second spelling of a byte
    // string that already has one, and is rejected rather than truncated.
    assert!(from_base64url("QQ").is_ok());
    assert!(from_base64url("QR").is_err());
    assert!(
        from_base64url("A").is_err(),
        "a length no encoding produces"
    );
    assert!(
        from_base64url("QQ==").is_err(),
        "padding is not the spelling"
    );
}

// --- golden vectors ------------------------------------------------------------------

/// The golden vectors, each a byte sequence written here rather than computed. Five since
/// the transport bone; a sixth at 3.4 (`certificate.rejected.result`, the F19 exchange).
///
/// A change to the field order, an escape spelling, an integer form, a union tag, or a
/// presence rule moves one of these and the diff names which.
fn golden() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        (
            "handshake.client_hello",
            to_bytes(&hello()).expect("encodes"),
        ),
        (
            "handshake.server_reject",
            to_bytes(&reject()).expect("encodes"),
        ),
        (
            "workspace.create.request",
            to_bytes(&request_envelope(
                "workspace.create",
                continuumd::codec::to_opaque(&create_request()).expect("encodes"),
            ))
            .expect("encodes"),
        ),
        (
            "verification.start.request",
            to_bytes(&request_envelope(
                "verification.start",
                continuumd::codec::to_opaque(&start_request()).expect("encodes"),
            ))
            .expect("encodes"),
        ),
        ("denial.result", to_bytes(&denial()).expect("encodes")),
        (
            "certificate.rejected.result",
            to_bytes(&certificate_rejected_result()).expect("encodes"),
        ),
        ("task.result", to_bytes(&task_result()).expect("encodes")),
    ]
}

#[test]
fn the_golden_vectors_are_the_bytes_this_file_declares() {
    let expected: Vec<(&str, &str)> = vec![
        (
            "handshake.client_hello",
            r#"{"actor":"agent:builder","capability":"cap_builder","client":"continuum-cli/0","encodings":["canonical_json","canonical_cbor"],"protocol_versions":{"high":"3.2","low":"3.0"}}"#,
        ),
        (
            "handshake.server_reject",
            r#"{"code":"ProtocolVersionUnsupported","detail":"the offered major is outside the served N/N−1 window","majors_served":[3,2],"retryable":false}"#,
        ),
        (
            "workspace.create.request",
            r#"{"actor":"agent:builder","arguments":{"components":{"cml_modules":[],"configuration":[],"correspondence":[],"dependencies":[],"domain_packs":[],"epochs":{"proof":"proof-1","semantic":"semantic-1"},"file_components":[{"commitment":"ws_aaa","path":"DieHard.ctm"},{"commitment":"ws_bbb","path":"README.md"}],"files":["ws_aaa","ws_bbb"],"intent":"in_diehard","proof_environment":[],"rust_extraction":[]},"seal":true},"budget":{"states":64},"capability":"cap_builder","idempotency_key":"idem-1","intent":null,"operation":"workspace.create","protocol_version":"3.2","request_id":"req_1","snapshot":null}"#,
        ),
        (
            "verification.start.request",
            r#"{"actor":"agent:builder","arguments":{"portfolio":"interactive","priority_class":"interactive","target":{"id":"DieHard","kind":"all_claims"}},"budget":{"states":64},"capability":"cap_builder","idempotency_key":"idem-1","intent":null,"operation":"verification.start","protocol_version":"3.2","request_id":"req_1","snapshot":null}"#,
        ),
        (
            "denial.result",
            r#"{"artifacts":[],"audit":"audit-1","cost":{},"epochs":{"corpus":null,"engine":"engine-reference-1","evidence":null,"intent":null,"proof":null,"protocol":"3.2","semantic":"semantic-1"},"error":{"code":"CapabilityDenied","detail":"the presented capability does not admit this operation","recovery":[],"retryable":false},"next_operations":[],"omissions":[],"payload":null,"request_id":"req_1","status":"error","verdict":null,"warnings":[]}"#,
        ),
        (
            "certificate.rejected.result",
            r#"{"artifacts":[],"cost":{},"epochs":{"corpus":null,"engine":"engine-reference-1","evidence":null,"intent":null,"proof":null,"protocol":"3.4","semantic":"semantic-1"},"error":{"code":"CertificateRejected","data":{"checker":"continuum-kernel-core","reason":"trailing-bytes"},"detail":"continuum-kernel-core rejected this certificate: the verdict is that kernel's own, over the bytes the daemon holds","recovery":[],"retryable":false},"next_operations":[],"omissions":[],"payload":null,"request_id":"req_3","status":"error","verdict":null,"warnings":[]}"#,
        ),
        (
            "task.result",
            r#"{"artifacts":[{"handle":"task_die_hard","kind":"task"}],"assurance":{"bounds":{"produced":{"engine":"continuum-engine-reference","summary":"exact finite reachability"}},"fairness":{"unsupported":{"reason":"sequential-consistency-only"}},"faults":{"unsupported":{"reason":"sequential-consistency-only"}},"memory_model":{"unsupported":{"reason":"sequential-consistency-only"}},"observer":{"unsupported":{"reason":"sequential-consistency-only"}},"proof_status":{"unsupported":{"reason":"sequential-consistency-only"}},"schedules":{"unsupported":{"reason":"sequential-consistency-only"}},"unknowns":{"unsupported":{"reason":"sequential-consistency-only"}},"values":{"produced":{"engine":"continuum-engine-reference","summary":"exact finite reachability"}}},"continuation":"cont_die_hard","cost":{"states":16},"epochs":{"corpus":null,"engine":"engine-reference-1","evidence":null,"intent":null,"proof":null,"protocol":"3.2","semantic":"semantic-1"},"next_operations":[],"omissions":[{"reason":"unsupported","subject":"cost.wall_ms"}],"payload":null,"request_id":"req_2","status":"task_suspended","task":"task_die_hard","verdict":{"semantic":{"assurance_class":"observed","inconclusive_reason":"ResourceExhausted","verdict":"inconclusive"}},"warnings":[]}"#,
        ),
    ];
    let actual = golden();
    assert_eq!(actual.len(), expected.len());
    for ((name, bytes), (expected_name, expected_bytes)) in actual.iter().zip(&expected) {
        assert_eq!(name, expected_name);
        assert_eq!(
            String::from_utf8(bytes.clone()).expect("UTF-8"),
            *expected_bytes,
            "golden vector `{name}`"
        );
    }
}

#[test]
fn a_golden_vector_re_encodes_to_itself_byte_for_byte() {
    // "re-encoding either MUST reproduce it byte for byte". Parsing and re-writing is the
    // normalizing operation that makes this a claim about the *writer* rather than about
    // the parser having been lenient.
    for (name, bytes) in golden() {
        let parsed = Json::parse(&bytes).unwrap_or_else(|_| panic!("`{name}` parses"));
        assert_eq!(parsed.to_canonical_bytes(), bytes, "`{name}` re-encodes");
    }
}

// --- the mutation sweep ---------------------------------------------------------------

#[test]
fn no_single_byte_mutation_of_a_golden_vector_decodes_to_the_same_document() {
    // Canonicality, operationally: one message has one spelling, so two distinct byte
    // strings never denote one document. Every mutant therefore either fails to parse or
    // parses to something *different* — and the second half is the one a permissive parser
    // fails, because it would read `{ "a":1}` and `{"a":1}` as the same document.
    //
    // Three replacement bytes per position rather than all 255: they cover the three ways
    // a mutation can matter — a structural character, a digit, and a bit flip inside a
    // string — and the sweep is over six vectors of several hundred bytes each.
    let mut mutants = 0_u64;
    let mut rejected = 0_u64;
    for (name, bytes) in golden() {
        let original = Json::parse(&bytes).unwrap_or_else(|_| panic!("`{name}` parses"));
        for index in 0..bytes.len() {
            for replacement in [b' ', b'0', bytes[index] ^ 0x01] {
                if replacement == bytes[index] {
                    continue;
                }
                let mut mutant = bytes.clone();
                mutant[index] = replacement;
                mutants += 1;
                match Json::parse(&mutant) {
                    Err(_) => rejected += 1,
                    Ok(parsed) => {
                        assert_ne!(
                            parsed, original,
                            "`{name}` byte {index} -> {replacement:#04x} decoded to the \
                             original document, so two byte strings denote one message"
                        );
                        // Whatever it decoded to, it is still canonical: re-encoding a
                        // mutant that parsed reproduces the mutant.
                        assert_eq!(
                            parsed.to_canonical_bytes(),
                            mutant,
                            "`{name}` byte {index} parsed but is not its own canonical form"
                        );
                    }
                }
            }
        }
    }
    assert!(
        mutants > 3_000,
        "the sweep is not vacuous: {mutants} mutants"
    );
    // Roughly a third of the positions are structural — punctuation, key bytes, enum
    // tokens, digits — and those are rejected outright. The rest fall inside a `detail`,
    // a client name, or an identifier, where a changed byte is a legal *different*
    // message; that is exactly why the injectivity half above is the load-bearing
    // assertion and this one is only a non-vacuity floor.
    assert!(
        rejected > mutants / 4,
        "the structural mutations must be rejected outright: {rejected} of {mutants}"
    );
}

#[test]
fn a_mutation_inside_a_typed_field_is_rejected_by_the_type_and_not_only_by_the_parser() {
    // The parser accepts a mutated *string*; the type layer is what rejects it. Each of
    // these is a document that parses cleanly and is still not a message.
    let bytes = to_bytes(&hello()).expect("encodes");
    let text = String::from_utf8(bytes).expect("UTF-8");

    for (from, to, why) in [
        (
            r#""cap_builder""#,
            r#""builder""#,
            "a handle without its class prefix",
        ),
        (
            r#""agent:builder""#,
            r#""nobody:builder""#,
            "an actor outside the four-member scheme",
        ),
        (r#""3.2""#, r#""3.02""#, "a version with a leading zero"),
        (r#""canonical_json""#, r#""json""#, "an undeclared encoding"),
    ] {
        let mutated = text.replacen(from, to, 1);
        assert_ne!(mutated, text, "the mutation applied: {why}");
        assert!(
            Json::parse(mutated.as_bytes()).is_ok(),
            "the mutant is still canonical JSON: {why}"
        );
        assert!(
            from_bytes::<ClientHello>(mutated.as_bytes()).is_err(),
            "the type rejects it: {why}"
        );
    }
}
