//! `canonical_cbor`, from outside the crate: the second encoding, the cross-encoding
//! identity, the conforming-daemon golden set, and the mutation sweep.
//!
//! # What this file is evidence for
//!
//! `tests/codec_canonical_form.rs` established the three claims RFC 0026 asks of a golden
//! vector for `canonical_json` — round trip, literal bytes, injectivity under mutation.
//! This file owes the same three for `canonical_cbor`, plus the one that only exists once
//! there are two encodings:
//!
//! > A golden vector is a byte sequence, not a shape: the JSON and CBOR vectors for one
//! > exchange MUST decode to the same values, and re-encoding either MUST reproduce it
//! > byte for byte.
//! >
//! > — RFC 0026, "Golden wire vectors"
//!
//! and the rule the two encodings are held to together:
//!
//! > **Field order is ascending Unicode code-point order of the field name**, in both
//! > encodings and at every nesting depth. […] A canonical-CBOR encoder therefore orders
//! > map keys by field name in code-point order and MUST NOT use the length-first map
//! > ordering the CBOR specification's own deterministic-encoding section fixes.
//! >
//! > — `rule encoding.canonical_form`
//!
//! # The five claims, and which test carries each
//!
//! 1. **round trip** — every representative message and every registry body decodes and
//!    re-encodes to itself in *both* encodings;
//! 2. **golden bytes** — the vectors are literal byte sequences written into this file,
//!    JSON as text and CBOR as hex, so a change to the field order, a head width, a leaf
//!    spelling, or a union tag is a failing diff rather than a silent re-spelling;
//! 3. **cross-encoding identity** — one message has one canonical field sequence, and the
//!    JSON and CBOR documents of a message agree on it key for key at every depth;
//! 4. **injectivity under mutation** — no single-byte mutation of a CBOR vector decodes to
//!    the same document, and every mutant that decodes at all is its own canonical form;
//! 5. **derived, not chosen** — each rule of the canonical CBOR subset is shown to follow
//!    from "one message has exactly one byte spelling" by exhibiting the second spelling it
//!    excludes.
//!
//! # Coverage of the registry, and the honest boundary of it
//!
//! The bone this file answers owes "the conforming-daemon golden set", and the registry is
//! 83 operations as of protocol 3.8. Two devices carry that between them and they cover
//! different things:
//!
//! - the **literal** vectors are exchanges and per-namespace bodies — bytes a second
//!   implementation can be tested against directly;
//! - the **registry sweep** is mechanical: every one of the 83 operations' request and
//!   response structs, and every named struct besides, is decoded from a document
//!   synthesized out of its own `FieldSpec` list and re-encoded, in both encodings, with
//!   the field sequences compared across them. It is not a literal byte sequence and does
//!   not pretend to be; it is the claim that *no declared shape in the protocol* has a
//!   codec that disagrees with its declaration or between the encodings.
//!
//! The synthesized documents are reached through `StructSpec`'s own round-trip closures,
//! so the swept set is exactly the registry's set: there is no second list of type names
//! here to drift from it.

use std::collections::BTreeMap;

use continuumd::codec::cbor::{Cbor, CborError, MAX_DEPTH};
use continuumd::codec::json::Json;
use continuumd::codec::operations::{decode_arguments_in, encode_payload_in};
use continuumd::codec::{CodecError, Document, ProtocolValue, read_in, to_opaque_in, write_in};
use continuumd::daemon::family::Payload;
use continuumd::protocol::envelope::{
    ArtifactRef, AssuranceEnvelope, Budget, CertificateRejection, Cost, EnvelopeDimension,
    EpochSet, Error, Omission, ProducedDimension, RequestEnvelope, ResultEnvelope,
    SemanticVerdictValue, UnsupportedDimension, Verdict,
};
use continuumd::protocol::handshake::{ClientHello, ServerReject, VersionRange};
use continuumd::protocol::operations::evidence::EvidenceLinkRequest;
use continuumd::protocol::operations::workspace::WorkspaceCreateRequest;
use continuumd::protocol::registry::{
    ENUMS, HANDLES, NAMED_STRUCTS, OPERATION_COUNT, OPERATIONS, UNIONS,
};
use continuumd::protocol::scalar::{
    ActorId, ArtifactHandle, AuditCorrelationId, Bytes, CapabilityHandle, Commitment,
    ContinuationHandle, EpochIdentity, EvidenceHandle, IntentHandle, Opaque, OperationName,
    ProtocolVersion, RequestId, TaskHandle,
};
use continuumd::protocol::shared::{FileComponent, SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{FieldSpec, Nullable, Optional, Presence, StructSpec};
use continuumd::protocol::vocabulary::{
    AssuranceClass, Encoding, ErrorCode, InconclusiveReason, OmissionReason, Portfolio,
    ResultStatus, SemanticVerdict, TargetKind,
};

// --- hex, for the CBOR literals ------------------------------------------------------

/// Lowercase hex, the spelling every CBOR literal in this file is written in.
///
/// A byte sequence in a Rust source file has to be *some* text, and hex is the one that
/// keeps a diff readable: a changed head width moves two characters and the reviewer can
/// see which byte moved.
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit(u32::from(byte >> 4), 16).expect("a nibble is a hex digit"));
        out.push(char::from_digit(u32::from(byte & 0x0F), 16).expect("a nibble is a hex digit"));
    }
    out
}

fn unhex(text: &str) -> Vec<u8> {
    let digits: Vec<u8> = text.bytes().collect();
    assert_eq!(digits.len() % 2, 0, "a hex literal has an even length");
    digits
        .chunks(2)
        .map(|pair| {
            let high = char::from(pair[0])
                .to_digit(16)
                .expect("a hex literal has hex digits");
            let low = char::from(pair[1])
                .to_digit(16)
                .expect("a hex literal has hex digits");
            u8::try_from(high * 16 + low).expect("two nibbles are a byte")
        })
        .collect()
}

// --- fixtures ------------------------------------------------------------------------
//
// The same shapes `tests/codec_canonical_form.rs` fixed for the JSON vectors, at protocol
// 3.3, plus the `evidence.link` request that arrived with it. The envelope fixtures are
// generic over the encoding because an `Opaque` carries bytes of the negotiated encoding —
// see `codec`'s module documentation — so "the same message" in the two encodings is two
// Rust values that differ in exactly that field and in nothing else.

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 3)
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
        encodings: vec![Encoding::CanonicalCbor, Encoding::CanonicalJson],
        client: "continuum-cli/0".to_owned(),
        actor: ActorId::new("agent:builder").expect("a well-formed actor"),
        capability: CapabilityHandle::new("cap_builder").expect("a well-formed capability"),
        features: Optional::Absent,
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

fn link_request() -> EvidenceLinkRequest {
    EvidenceLinkRequest {
        subject: EvidenceHandle::new("ev_diehard").expect("a well-formed evidence handle"),
        receipt: commitment("rcpt_diehard"),
        checker_profile: "continuum-kernel-core/0".to_owned(),
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

/// A `workspace.create` request envelope with its body staged in the encoding `D`.
fn create_envelope<D: Document>() -> RequestEnvelope {
    request_envelope(
        "workspace.create",
        to_opaque_in::<D, _>(&create_request()).expect("the body encodes"),
    )
}

/// An `evidence.link` request envelope — the 73rd operation, new at protocol 3.3.
fn link_envelope<D: Document>() -> RequestEnvelope {
    request_envelope(
        "evidence.link",
        to_opaque_in::<D, _>(&link_request()).expect("the body encodes"),
    )
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

/// A `CertificateRejected` result carrying the declared `Error.data` shape — the F19
/// exchange (RFC 0026, protocol 3.4, bn-3jrtz). Generic over the encoding because `data`
/// is an `Opaque`: bytes of the encoding the message travels in, so "the same message" in
/// the two encodings differs in exactly that field.
fn certificate_rejected_result<D: Document>() -> ResultEnvelope {
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
                to_opaque_in::<D, _>(&CertificateRejection {
                    checker: "continuum-kernel-core".to_owned(),
                    reason: "trailing-bytes".to_owned(),
                    field: Optional::Absent,
                })
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

/// A suspended task result, with the nine-dimension envelope its semantic verdict obliges.
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
            inconclusive_reason: Optional::Present(InconclusiveReason::ResourceExhausted),
            assurance_class: AssuranceClass::Observed,
        })),
        error: Optional::Absent,
        assurance: Optional::Present(AssuranceEnvelope {
            bounds: produced.clone(),
            faults: unsupported.clone(),
            fairness: unsupported.clone(),
            values: produced,
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

/// A `verification.start` request body, for the leaf-spelling tests.
fn target() -> Target {
    Target {
        kind: TargetKind::AllClaims,
        id: "DieHard".to_owned(),
    }
}

// --- the field sequence, the thing both encodings must agree on ----------------------

/// The canonical field sequence of a document: every map key and array index, in order,
/// with its path, at every depth.
///
/// This is what `rule encoding.canonical_form` fixes and what "identical canonical field
/// order" names. It deliberately records *structure and keys only*: the two encodings
/// spell two leaves differently on purpose (IDL §3), so a comparison that included leaf
/// bytes would be asserting the opposite of what the IDL says.
fn sequence<D: Document>(value: &D, path: &str, out: &mut Vec<String>) {
    if let Some(entries) = value.as_entries() {
        for (key, inner) in entries {
            let child = format!("{path}.{key}");
            out.push(child.clone());
            sequence(inner, &child, out);
        }
    } else if let Some(items) = value.as_items() {
        for (index, item) in items.iter().enumerate() {
            let child = format!("{path}[{index}]");
            out.push(child.clone());
            sequence(item, &child, out);
        }
    }
}

fn field_sequence<D: Document>(value: &D) -> Vec<String> {
    let mut out = Vec::new();
    sequence(value, "", &mut out);
    out
}

// --- the document synthesizer, driven by the registry's own declarations -------------

/// A canonical document that is a valid instance of `spec`, built from `spec` alone.
///
/// Every value is the *first* thing the declaration admits — the first enum member, the
/// first union variant, a handle's prefix plus one character — so the document is a
/// function of the declaration and of nothing else. Two properties make that worth having:
/// it needs no fixture per struct, so the swept set cannot drift from the registry; and it
/// is identical in shape across the two encodings by construction, which is exactly the
/// claim the cross-encoding test makes.
///
/// `optional` fields are *present*: an absent field is covered by the hand-written vectors
/// and a present one exercises more of the codec.
fn synthesize<D: Document>(spec: &StructSpec, depth: usize) -> D {
    assert!(
        depth < 24,
        "`{}` nests further than any declaration",
        spec.name
    );
    let mut entries: BTreeMap<String, D> = BTreeMap::new();
    for field in spec.fields {
        entries.insert(field.name.to_owned(), field_value::<D>(field, depth));
    }
    D::from_entries(entries)
}

fn field_value<D: Document>(field: &FieldSpec, depth: usize) -> D {
    // A `nullable` field is spelled null in half the cases, by a rule that is a function of
    // the field name so that the choice is stable across runs and across encodings.
    if field.presence == Presence::Nullable && field.name.len() % 2 == 0 {
        return D::from_null();
    }
    shape_value::<D>(field.ty, depth)
}

fn shape_value<D: Document>(ty: &str, depth: usize) -> D {
    if let Some(inner) = ty.strip_prefix("list<").and_then(|t| t.strip_suffix('>')) {
        return D::from_items(vec![type_value::<D>(inner, depth + 1)]);
    }
    if let Some(inner) = ty
        .strip_prefix("map<String,")
        .and_then(|t| t.strip_suffix('>'))
    {
        let mut entries = BTreeMap::new();
        entries.insert("k".to_owned(), type_value::<D>(inner, depth + 1));
        return D::from_entries(entries);
    }
    type_value::<D>(ty, depth)
}

/// A value of the IDL type named `ty`, resolved through the registry's own tables.
fn type_value<D: Document>(ty: &str, depth: usize) -> D {
    // IDL §3 primitives.
    match ty {
        "Bool" => return D::from_bool(true),
        "U32" => return D::from_unsigned(7),
        // Deliberately beyond `2^53 − 1`: this is the magnitude where the two encodings
        // spell a `U64` differently, so every swept struct with a `U64` exercises both.
        "U64" | "DurationMs" | "ByteCount" => return D::from_unsigned(9_007_199_254_740_993),
        "String" => return D::from_text("x"),
        "Bytes" => return D::from_byte_string(&[0x00, 0xFF, 0x10, 0x83, 0x7A]),
        "Timestamp" => return D::from_text("2026-01-01T00:00:00.000Z"),
        // An `Opaque` is a canonical value of the negotiated encoding, carried verbatim.
        "Opaque" => return D::from_entries(BTreeMap::new()),
        _ => {}
    }
    // IDL §5 string aliases. The literal satisfies the `@pattern` the registry records;
    // there is no regular-expression engine in this workspace to generate one from.
    match ty {
        "ArtifactHandle" => return D::from_text("artifact_x"),
        "RequestId" => return D::from_text("req_x"),
        "ActorId" => return D::from_text("agent:x"),
        "ProtocolVersion" => return D::from_text("3.3"),
        "EpochIdentity" => return D::from_text("e1"),
        "Commitment" | "PageToken" => return D::from_text("c1"),
        "OperationName" => return D::from_text("workspace.create"),
        "AuditCorrelationId" => return D::from_text("a1"),
        "SignerHandle" => {
            return D::from_text(
                "signer_0000000000000000000000000000000000000000000000000000000000000000",
            );
        }
        _ => {}
    }
    if let Some(handle) = HANDLES.iter().find(|handle| handle.name == ty) {
        return D::from_text(&format!("{}x", handle.prefix));
    }
    if let Some(declared) = ENUMS.iter().find(|declared| declared.name == ty) {
        return D::from_text(declared.members[0].wire);
    }
    if let Some(declared) = UNIONS.iter().find(|declared| declared.name == ty) {
        let variant = declared.variants[0];
        let mut entries = BTreeMap::new();
        entries.insert(
            variant.ident.to_owned(),
            type_value::<D>(variant.ty, depth + 1),
        );
        return D::from_entries(entries);
    }
    if let Some(declared) = NAMED_STRUCTS.iter().find(|declared| declared.name == ty) {
        return synthesize::<D>(declared, depth + 1);
    }
    panic!("no registry table declares the type `{ty}`");
}

/// Every struct the registry declares: the 83 operations' request and response bodies and
/// every named struct, each named for the failure message.
fn every_declared_struct() -> Vec<(String, &'static StructSpec)> {
    let mut out = Vec::new();
    for operation in OPERATIONS {
        out.push((format!("{} request", operation.name), &operation.request));
        out.push((format!("{} response", operation.name), &operation.response));
    }
    for declared in NAMED_STRUCTS {
        out.push((declared.name.to_owned(), declared));
    }
    out
}

// --- the golden set -------------------------------------------------------------------

/// The golden vectors, each computed here and pinned in [`GOLDEN`] as literal bytes.
///
/// Two groups. The **exchange** vectors are hand-built messages: the handshake both ways,
/// two request envelopes, and three result envelopes (the third is the F19 exchange, new
/// at 3.4 — bn-3jrtz), which are the frames a conforming daemon actually exchanges. The **body** vectors are one operation body per IDL
/// namespace, synthesized from the declaration, which is what makes the set cover the
/// registry's breadth rather than one family's depth.
fn golden() -> Vec<(String, Vec<u8>, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>, Vec<u8>)> = vec![
        pair("handshake.client_hello", &hello(), &hello()),
        pair("handshake.server_reject", &reject(), &reject()),
        pair(
            "workspace.create.request",
            &create_envelope::<Json>(),
            &create_envelope::<Cbor>(),
        ),
        pair(
            "evidence.link.request",
            &link_envelope::<Json>(),
            &link_envelope::<Cbor>(),
        ),
        pair("denial.result", &denial(), &denial()),
        pair(
            "certificate.rejected.result",
            &certificate_rejected_result::<Json>(),
            &certificate_rejected_result::<Cbor>(),
        ),
        pair("task.result", &task_result(), &task_result()),
    ];
    for (namespace, spec) in first_operation_per_namespace() {
        out.push((
            format!("{namespace}.body"),
            synthesize::<Json>(spec, 0).to_canonical_bytes(),
            synthesize::<Cbor>(spec, 0).to_canonical_bytes(),
        ));
    }
    out
}

/// One vector: the same message written in both encodings.
///
/// The two values are separate arguments rather than one because an `Opaque` carries bytes
/// of the encoding it is written in, so a message with a payload is two Rust values that
/// differ in that field alone.
fn pair<T: ProtocolValue, U: ProtocolValue>(
    name: &str,
    json: &T,
    cbor: &U,
) -> (String, Vec<u8>, Vec<u8>) {
    (
        name.to_owned(),
        write_in::<Json, _>(json).expect("the message encodes as JSON"),
        write_in::<Cbor, _>(cbor).expect("the message encodes as CBOR"),
    )
}

/// The first operation of each of the IDL's 20 namespaces, in registry order.
fn first_operation_per_namespace() -> Vec<(&'static str, &'static StructSpec)> {
    let mut seen: Vec<&'static str> = Vec::new();
    let mut out = Vec::new();
    for operation in OPERATIONS {
        let namespace = operation
            .name
            .split('.')
            .next()
            .expect("an operation name is `namespace.verb`");
        if !seen.contains(&namespace) {
            seen.push(namespace);
            out.push((operation.name, &operation.request));
        }
    }
    out
}

/// The golden bytes, written here rather than computed: JSON as text, CBOR as hex.
///
/// A change to the field order, an escape spelling, an integer form, a head width, a union
/// tag, or a presence rule moves one of these and the diff names which.
const GOLDEN: &[(&str, &str, &str)] = &[
    (
        "handshake.client_hello",
        r#"{"actor":"agent:builder","capability":"cap_builder","client":"continuum-cli/0","encodings":["canonical_cbor","canonical_json"],"protocol_versions":{"high":"3.3","low":"3.0"}}"#,
        "a5656163746f726d6167656e743a6275696c6465726a6361706162696c6974796b6361705f6275696c64657266636c69656e746f636f6e74696e75756d2d636c692f3069656e636f64696e6773826e63616e6f6e6963616c5f63626f726e63616e6f6e6963616c5f6a736f6e7170726f746f636f6c5f76657273696f6e73a2646869676863332e33636c6f7763332e30",
    ),
    (
        "handshake.server_reject",
        r#"{"code":"ProtocolVersionUnsupported","detail":"the offered major is outside the served N/N−1 window","majors_served":[3,2],"retryable":false}"#,
        "a464636f6465781a50726f746f636f6c56657273696f6e556e737570706f727465646664657461696c7836746865206f666665726564206d616a6f72206973206f7574736964652074686520736572766564204e2f4ee28892312077696e646f776d6d616a6f72735f73657276656482030269726574727961626c65f4",
    ),
    (
        "workspace.create.request",
        r#"{"actor":"agent:builder","arguments":{"components":{"cml_modules":[],"configuration":[],"correspondence":[],"dependencies":[],"domain_packs":[],"epochs":{"proof":"proof-1","semantic":"semantic-1"},"file_components":[{"commitment":"ws_aaa","path":"DieHard.ctm"},{"commitment":"ws_bbb","path":"README.md"}],"files":["ws_aaa","ws_bbb"],"intent":"in_diehard","proof_environment":[],"rust_extraction":[]},"seal":true},"budget":{"states":64},"capability":"cap_builder","idempotency_key":"idem-1","intent":null,"operation":"workspace.create","protocol_version":"3.3","request_id":"req_1","snapshot":null}"#,
        "aa656163746f726d6167656e743a6275696c64657269617267756d656e7473a26a636f6d706f6e656e7473ab6b636d6c5f6d6f64756c6573806d636f6e66696775726174696f6e806e636f72726573706f6e64656e6365806c646570656e64656e63696573806c646f6d61696e5f7061636b73806665706f636873a26570726f6f666770726f6f662d316873656d616e7469636a73656d616e7469632d316f66696c655f636f6d706f6e656e747382a26a636f6d6d69746d656e746677735f61616164706174686b446965486172642e63746da26a636f6d6d69746d656e746677735f626262647061746869524541444d452e6d646566696c6573826677735f6161616677735f62626266696e74656e746a696e5f646965686172647170726f6f665f656e7669726f6e6d656e74806f727573745f65787472616374696f6e80647365616cf566627564676574a16673746174657318406a6361706162696c6974796b6361705f6275696c6465726f6964656d706f74656e63795f6b6579666964656d2d3166696e74656e74f6696f7065726174696f6e70776f726b73706163652e6372656174657070726f746f636f6c5f76657273696f6e63332e336a726571756573745f6964657265715f3168736e617073686f74f6",
    ),
    (
        "evidence.link.request",
        r#"{"actor":"agent:builder","arguments":{"checker_profile":"continuum-kernel-core/0","receipt":"rcpt_diehard","subject":"ev_diehard"},"budget":{"states":64},"capability":"cap_builder","idempotency_key":"idem-1","intent":null,"operation":"evidence.link","protocol_version":"3.3","request_id":"req_1","snapshot":null}"#,
        "aa656163746f726d6167656e743a6275696c64657269617267756d656e7473a36f636865636b65725f70726f66696c6577636f6e74696e75756d2d6b65726e656c2d636f72652f3067726563656970746c726370745f64696568617264677375626a6563746a65765f6469656861726466627564676574a16673746174657318406a6361706162696c6974796b6361705f6275696c6465726f6964656d706f74656e63795f6b6579666964656d2d3166696e74656e74f6696f7065726174696f6e6d65766964656e63652e6c696e6b7070726f746f636f6c5f76657273696f6e63332e336a726571756573745f6964657265715f3168736e617073686f74f6",
    ),
    (
        "denial.result",
        r#"{"artifacts":[],"audit":"audit-1","cost":{},"epochs":{"corpus":null,"engine":"engine-reference-1","evidence":null,"intent":null,"proof":null,"protocol":"3.3","semantic":"semantic-1"},"error":{"code":"CapabilityDenied","detail":"the presented capability does not admit this operation","recovery":[],"retryable":false},"next_operations":[],"omissions":[],"payload":null,"request_id":"req_1","status":"error","verdict":null,"warnings":[]}"#,
        "ac69617274696661637473806561756469746761756469742d3164636f7374a06665706f636873a766636f72707573f666656e67696e6572656e67696e652d7265666572656e63652d316865766964656e6365f666696e74656e74f66570726f6f66f66870726f746f636f6c63332e336873656d616e7469636a73656d616e7469632d31656572726f72a464636f6465704361706162696c69747944656e6965646664657461696c78367468652070726573656e746564206361706162696c69747920646f6573206e6f742061646d69742074686973206f7065726174696f6e687265636f766572798069726574727961626c65f46f6e6578745f6f7065726174696f6e7380696f6d697373696f6e7380677061796c6f6164f66a726571756573745f6964657265715f3166737461747573656572726f726776657264696374f6687761726e696e677380",
    ),
    (
        "certificate.rejected.result",
        r#"{"artifacts":[],"cost":{},"epochs":{"corpus":null,"engine":"engine-reference-1","evidence":null,"intent":null,"proof":null,"protocol":"3.4","semantic":"semantic-1"},"error":{"code":"CertificateRejected","data":{"checker":"continuum-kernel-core","reason":"trailing-bytes"},"detail":"continuum-kernel-core rejected this certificate: the verdict is that kernel's own, over the bytes the daemon holds","recovery":[],"retryable":false},"next_operations":[],"omissions":[],"payload":null,"request_id":"req_3","status":"error","verdict":null,"warnings":[]}"#,
        "ab696172746966616374738064636f7374a06665706f636873a766636f72707573f666656e67696e6572656e67696e652d7265666572656e63652d316865766964656e6365f666696e74656e74f66570726f6f66f66870726f746f636f6c63332e346873656d616e7469636a73656d616e7469632d31656572726f72a564636f646573436572746966696361746552656a65637465646464617461a267636865636b657275636f6e74696e75756d2d6b65726e656c2d636f726566726561736f6e6e747261696c696e672d62797465736664657461696c7872636f6e74696e75756d2d6b65726e656c2d636f72652072656a656374656420746869732063657274696669636174653a2074686520766572646963742069732074686174206b65726e656c2773206f776e2c206f7665722074686520627974657320746865206461656d6f6e20686f6c6473687265636f766572798069726574727961626c65f46f6e6578745f6f7065726174696f6e7380696f6d697373696f6e7380677061796c6f6164f66a726571756573745f6964657265715f3366737461747573656572726f726776657264696374f6687761726e696e677380",
    ),
    (
        "task.result",
        r#"{"artifacts":[{"handle":"task_die_hard","kind":"task"}],"assurance":{"bounds":{"produced":{"engine":"continuum-engine-reference","summary":"exact finite reachability"}},"fairness":{"unsupported":{"reason":"sequential-consistency-only"}},"faults":{"unsupported":{"reason":"sequential-consistency-only"}},"memory_model":{"unsupported":{"reason":"sequential-consistency-only"}},"observer":{"unsupported":{"reason":"sequential-consistency-only"}},"proof_status":{"unsupported":{"reason":"sequential-consistency-only"}},"schedules":{"unsupported":{"reason":"sequential-consistency-only"}},"unknowns":{"unsupported":{"reason":"sequential-consistency-only"}},"values":{"produced":{"engine":"continuum-engine-reference","summary":"exact finite reachability"}}},"continuation":"cont_die_hard","cost":{"states":16},"epochs":{"corpus":null,"engine":"engine-reference-1","evidence":null,"intent":null,"proof":null,"protocol":"3.3","semantic":"semantic-1"},"next_operations":[],"omissions":[{"reason":"unsupported","subject":"cost.wall_ms"}],"payload":null,"request_id":"req_2","status":"task_suspended","task":"task_die_hard","verdict":{"semantic":{"assurance_class":"observed","inconclusive_reason":"ResourceExhausted","verdict":"inconclusive"}},"warnings":[]}"#,
        "ad6961727469666163747381a26668616e646c656d7461736b5f6469655f68617264646b696e64647461736b696173737572616e6365a966626f756e6473a16870726f6475636564a266656e67696e65781a636f6e74696e75756d2d656e67696e652d7265666572656e63656773756d6d617279781965786163742066696e6974652072656163686162696c69747968666169726e657373a16b756e737570706f72746564a166726561736f6e781b73657175656e7469616c2d636f6e73697374656e63792d6f6e6c79666661756c7473a16b756e737570706f72746564a166726561736f6e781b73657175656e7469616c2d636f6e73697374656e63792d6f6e6c796c6d656d6f72795f6d6f64656ca16b756e737570706f72746564a166726561736f6e781b73657175656e7469616c2d636f6e73697374656e63792d6f6e6c79686f62736572766572a16b756e737570706f72746564a166726561736f6e781b73657175656e7469616c2d636f6e73697374656e63792d6f6e6c796c70726f6f665f737461747573a16b756e737570706f72746564a166726561736f6e781b73657175656e7469616c2d636f6e73697374656e63792d6f6e6c79697363686564756c6573a16b756e737570706f72746564a166726561736f6e781b73657175656e7469616c2d636f6e73697374656e63792d6f6e6c7968756e6b6e6f776e73a16b756e737570706f72746564a166726561736f6e781b73657175656e7469616c2d636f6e73697374656e63792d6f6e6c796676616c756573a16870726f6475636564a266656e67696e65781a636f6e74696e75756d2d656e67696e652d7265666572656e63656773756d6d617279781965786163742066696e6974652072656163686162696c6974796c636f6e74696e756174696f6e6d636f6e745f6469655f6861726464636f7374a166737461746573106665706f636873a766636f72707573f666656e67696e6572656e67696e652d7265666572656e63652d316865766964656e6365f666696e74656e74f66570726f6f66f66870726f746f636f6c63332e336873656d616e7469636a73656d616e7469632d316f6e6578745f6f7065726174696f6e7380696f6d697373696f6e7381a266726561736f6e6b756e737570706f72746564677375626a6563746c636f73742e77616c6c5f6d73677061796c6f6164f66a726571756573745f6964657265715f32667374617475736e7461736b5f73757370656e646564647461736b6d7461736b5f6469655f686172646776657264696374a16873656d616e746963a36f6173737572616e63655f636c617373686f6273657276656473696e636f6e636c75736976655f726561736f6e715265736f7572636545786861757374656467766572646963746c696e636f6e636c7573697665687761726e696e677380",
    ),
    (
        "workspace.create.body",
        r#"{"components":{"cml_modules":["c1"],"configuration":["c1"],"correspondence":["c1"],"dependencies":["c1"],"domain_packs":["c1"],"epochs":{"proof":"e1","semantic":"e1","toolchain":"e1"},"file_components":[{"commitment":"c1","path":"x"}],"files":["c1"],"intent":"in_x","proof_environment":["c1"],"rust_extraction":["c1"]},"overlay":[{"content":"AP8Qg3o","path":"x"}],"seal":true}"#,
        "a36a636f6d706f6e656e7473ab6b636d6c5f6d6f64756c6573816263316d636f6e66696775726174696f6e816263316e636f72726573706f6e64656e6365816263316c646570656e64656e63696573816263316c646f6d61696e5f7061636b73816263316665706f636873a36570726f6f666265316873656d616e74696362653169746f6f6c636861696e6265316f66696c655f636f6d706f6e656e747381a26a636f6d6d69746d656e74626331647061746861786566696c65738162633166696e74656e7464696e5f787170726f6f665f656e7669726f6e6d656e74816263316f727573745f65787472616374696f6e81626331676f7665726c617981a267636f6e74656e744500ff10837a64706174686178647365616cf5",
    ),
    (
        "intent.get.body",
        r#"{"intent":"in_x"}"#,
        "a166696e74656e7464696e5f78",
    ),
    (
        "verification.start.body",
        r#"{"context_policy":{"audience":"agent","budget":{"bytes":"9007199254740993","candidates":"9007199254740993","cpu_ms":"9007199254740993","memory_bytes":"9007199254740993","proof_ms":"9007199254740993","solver_ms":"9007199254740993","states":"9007199254740993","tokens":"9007199254740993","wall_ms":"9007199254740993"},"compile_on_failure":true},"portfolio":"interactive","priority_class":"interactive","target":{"id":"x","kind":"property"}}"#,
        "a46e636f6e746578745f706f6c696379a36861756469656e6365656167656e7466627564676574a96562797465731b00200000000000016a63616e646964617465731b0020000000000001666370755f6d731b00200000000000016c6d656d6f72795f62797465731b00200000000000016870726f6f665f6d731b002000000000000169736f6c7665725f6d731b0020000000000001667374617465731b002000000000000166746f6b656e731b00200000000000016777616c6c5f6d731b002000000000000172636f6d70696c655f6f6e5f6661696c757265f569706f7274666f6c696f6b696e7465726163746976656e7072696f726974795f636c6173736b696e74657261637469766566746172676574a26269646178646b696e646870726f7065727479",
    ),
    (
        "model.check.body",
        r#"{"context_policy":{"audience":"agent","budget":{"bytes":"9007199254740993","candidates":"9007199254740993","cpu_ms":"9007199254740993","memory_bytes":"9007199254740993","proof_ms":"9007199254740993","solver_ms":"9007199254740993","states":"9007199254740993","tokens":"9007199254740993","wall_ms":"9007199254740993"},"compile_on_failure":true},"model":"model_x","target":{"id":"x","kind":"property"}}"#,
        "a36e636f6e746578745f706f6c696379a36861756469656e6365656167656e7466627564676574a96562797465731b00200000000000016a63616e646964617465731b0020000000000001666370755f6d731b00200000000000016c6d656d6f72795f62797465731b00200000000000016870726f6f665f6d731b002000000000000169736f6c7665725f6d731b0020000000000001667374617465731b002000000000000166746f6b656e731b00200000000000016777616c6c5f6d731b002000000000000172636f6d70696c655f6f6e5f6661696c757265f5656d6f64656c676d6f64656c5f7866746172676574a26269646178646b696e646870726f7065727479",
    ),
    (
        "program.extract.body",
        r#"{"roots":["x"]}"#,
        "a165726f6f7473816178",
    ),
    (
        "refinement.check.body",
        r#"{"model":"model_x","observer":"x","program":"model_x"}"#,
        "a3656d6f64656c676d6f64656c5f78686f6273657276657261786770726f6772616d676d6f64656c5f78",
    ),
    (
        "proof.goal.body",
        r#"{"obligation":"x"}"#,
        "a16a6f626c69676174696f6e6178",
    ),
    (
        "correspondence.bind.body",
        r#"{"element":"x","source":{"end_column":7,"end_line":7,"file":"x","start_column":7,"start_line":7}}"#,
        "a267656c656d656e74617866736f75726365a56a656e645f636f6c756d6e0768656e645f6c696e65076466696c6561786c73746172745f636f6c756d6e076a73746172745f6c696e6507",
    ),
    (
        "debug.open.body",
        r#"{"observer":"x","subject":"artifact_x"}"#,
        "a2686f627365727665726178677375626a6563746a61727469666163745f78",
    ),
    (
        "context.compile.body",
        r#"{"audience":"agent","evidence_root":"artifact_x","guarantees":["x"],"question":"x"}"#,
        "a46861756469656e6365656167656e746d65766964656e63655f726f6f746a61727469666163745f786a67756172616e74656573816178687175657374696f6e6178",
    ),
    (
        "failure.explain.body",
        r#"{"audience":"agent","failure":"crash_x","level":"trace"}"#,
        "a36861756469656e6365656167656e74676661696c7572656763726173685f78656c6576656c657472616365",
    ),
    (
        "repair.begin.body",
        r#"{"failure":"crash_x","gate_profile":"phase-b"}"#,
        "a2676661696c7572656763726173685f786c676174655f70726f66696c656770686173652d62",
    ),
    (
        "observe.ingest.body",
        r#"{"instrumentation_profile":"x","trace":"c1"}"#,
        "a277696e737472756d656e746174696f6e5f70726f66696c656178657472616365626331",
    ),
    (
        "forge.create.body",
        r#"{"assurance_target":"observed","diversity_descriptors":["x"],"objectives":["x"],"sketch":{}}"#,
        "a4706173737572616e63655f746172676574686f62736572766564756469766572736974795f64657363726970746f72738161786a6f626a6563746976657381617866736b65746368a0",
    ),
    (
        "benchmark.run.body",
        r#"{"graders":["x"],"task_id":"x"}"#,
        "a26767726164657273816178677461736b5f69646178",
    ),
    (
        "task.status.body",
        r#"{"task":"task_x"}"#,
        "a1647461736b667461736b5f78",
    ),
    (
        "evidence.get.body",
        r#"{"evidence":"ev_x","inline":true}"#,
        "a26865766964656e63656465765f7866696e6c696e65f5",
    ),
    (
        "whiteboard.compile.body",
        r#"{"note":{}}"#,
        "a1646e6f7465a0",
    ),
    (
        "signing.mint.body",
        r#"{"kinds":["receipt"]}"#,
        "a1656b696e6473816772656365697074",
    ),
    (
        "query.explain_reuse.body",
        r#"{"derivation":"artifact_x"}"#,
        "a16a64657269766174696f6e6a61727469666163745f78",
    ),
];

// --- the golden-byte claims ------------------------------------------------------------

#[test]
fn the_golden_vectors_are_the_bytes_this_file_declares() {
    let actual = golden();
    assert_eq!(
        actual.len(),
        GOLDEN.len(),
        "the vector list and the byte table are the same set"
    );
    for ((name, json, cbor), (expected_name, expected_json, expected_cbor)) in
        actual.iter().zip(GOLDEN)
    {
        assert_eq!(name, expected_name);
        assert_eq!(
            String::from_utf8(json.clone()).expect("canonical JSON is UTF-8"),
            *expected_json,
            "JSON golden vector `{name}`"
        );
        assert_eq!(hex(cbor), *expected_cbor, "CBOR golden vector `{name}`");
    }
}

#[test]
fn a_golden_vector_re_encodes_to_itself_byte_for_byte() {
    // "re-encoding either MUST reproduce it byte for byte". Parsing and re-writing is the
    // normalizing operation that makes this a claim about the *writer* rather than about
    // the reader having been lenient.
    for (name, json, cbor) in golden() {
        let parsed = Json::parse(&json).unwrap_or_else(|error| panic!("`{name}` parses: {error}"));
        assert_eq!(
            parsed.to_canonical_bytes(),
            json,
            "`{name}` re-encodes (JSON)"
        );
        let parsed = Cbor::parse(&cbor).unwrap_or_else(|error| panic!("`{name}` parses: {error}"));
        assert_eq!(
            parsed.to_canonical_bytes(),
            cbor,
            "`{name}` re-encodes (CBOR)"
        );
    }
}

#[test]
fn one_message_has_one_canonical_field_sequence_in_both_encodings() {
    // `rule encoding.canonical_form`: "identical canonical field order […] in both
    // encodings and at every nesting depth". Asserted as the *sequence* rather than the
    // set, because a set would be satisfied by the length-first order the rule forbids.
    for (name, json, cbor) in golden() {
        let json = Json::parse(&json).expect("the JSON vector parses");
        let cbor = Cbor::parse(&cbor).expect("the CBOR vector parses");
        assert_eq!(
            field_sequence(&json),
            field_sequence(&cbor),
            "`{name}` has one field sequence in both encodings"
        );
    }
}

#[test]
fn the_cbor_key_order_is_the_idls_and_not_the_cbor_specifications() {
    // The one row where the two specifications disagree, made concrete. `"aa"` and `"b"`
    // sort one way by code point and the other way by the bytewise order of their encoded
    // heads, which is where the CBOR specification's deterministic encoding puts shorter
    // keys first. The IDL wins.
    let mut entries = BTreeMap::new();
    entries.insert("b".to_owned(), Cbor::Unsigned(1));
    entries.insert("aa".to_owned(), Cbor::Unsigned(2));
    let bytes = Cbor::Map(entries).to_canonical_bytes();
    assert_eq!(
        hex(&bytes),
        "a262616102616201",
        "`aa` precedes `b`: code-point order, not length-first"
    );
    // The length-first spelling of the same map is a second byte spelling, and the reader
    // refuses it rather than re-ordering.
    let length_first = unhex("a2616201626161 02".replace(' ', "").as_str());
    assert_eq!(
        Cbor::parse(&length_first).expect_err("length-first is out of canonical order"),
        CborError::KeyOrder { at: 4 }
    );
}

// --- round trip -------------------------------------------------------------------------

/// Assert `decode(encode(v)) == v` and that re-encoding reproduces the same bytes, in `D`.
fn round_trip<D: Document, T: ProtocolValue + PartialEq + core::fmt::Debug>(value: &T) -> Vec<u8> {
    let bytes = write_in::<D, _>(value).expect("the value encodes");
    let decoded: T = read_in::<D, _>(&bytes).expect("the value decodes");
    assert_eq!(
        &decoded,
        value,
        "decode(encode(v)) == v in {:?}",
        D::ENCODING
    );
    let again = write_in::<D, _>(&decoded).expect("the decoded value re-encodes");
    assert_eq!(again, bytes, "encode(decode(b)) == b in {:?}", D::ENCODING);
    bytes
}

#[test]
fn every_representative_message_round_trips_through_both_encodings() {
    round_trip::<Json, _>(&hello());
    round_trip::<Cbor, _>(&hello());
    round_trip::<Json, _>(&reject());
    round_trip::<Cbor, _>(&reject());
    round_trip::<Json, _>(&create_request());
    round_trip::<Cbor, _>(&create_request());
    round_trip::<Json, _>(&link_request());
    round_trip::<Cbor, _>(&link_request());
    round_trip::<Json, _>(&components());
    round_trip::<Cbor, _>(&components());
    round_trip::<Json, _>(&target());
    round_trip::<Cbor, _>(&target());
    round_trip::<Json, _>(&denial());
    round_trip::<Cbor, _>(&denial());
    round_trip::<Json, _>(&task_result());
    round_trip::<Cbor, _>(&task_result());
    round_trip::<Json, _>(&create_envelope::<Json>());
    round_trip::<Cbor, _>(&create_envelope::<Cbor>());
    round_trip::<Json, _>(&link_envelope::<Json>());
    round_trip::<Cbor, _>(&link_envelope::<Cbor>());
}

#[test]
fn a_message_decoded_from_one_encoding_equals_the_message_decoded_from_the_other() {
    // Cross-encoding identity at the level of the *typed value*, for every message that
    // carries no `Opaque`. The two byte strings are as different as two encodings get, and
    // the values they denote are equal.
    macro_rules! agree {
        ($value:expr, $ty:ty) => {{
            let value = $value;
            let json: $ty = read_in::<Json, _>(&write_in::<Json, _>(&value).expect("encodes"))
                .expect("the JSON bytes decode");
            let cbor: $ty = read_in::<Cbor, _>(&write_in::<Cbor, _>(&value).expect("encodes"))
                .expect("the CBOR bytes decode");
            assert_eq!(json, cbor, "one message, two encodings, one value");
            assert_eq!(json, value);
        }};
    }
    agree!(hello(), ClientHello);
    agree!(reject(), ServerReject);
    agree!(create_request(), WorkspaceCreateRequest);
    agree!(link_request(), EvidenceLinkRequest);
    agree!(components(), SnapshotComponents);
    agree!(denial(), ResultEnvelope);
    agree!(task_result(), ResultEnvelope);

    // A message that *does* carry an `Opaque` agrees on everything but that field, and
    // that is the contract rather than a gap: the payload is "a canonical value of the
    // negotiated encoding" (`rule encoding.opaque_payloads`), so its bytes are the
    // encoding's. The field sequence still agrees, which is what the rule fixes.
    let json = create_envelope::<Json>();
    let cbor = create_envelope::<Cbor>();
    assert_ne!(
        json.arguments, cbor.arguments,
        "the payload is in its encoding"
    );
    assert_eq!(
        RequestEnvelope {
            arguments: cbor.arguments.clone(),
            ..json.clone()
        },
        cbor,
        "the two envelopes differ in `arguments` and nowhere else"
    );
    let body_from_json: WorkspaceCreateRequest =
        continuumd::codec::from_opaque_in::<Json, _>(&json.arguments).expect("decodes");
    let body_from_cbor: WorkspaceCreateRequest =
        continuumd::codec::from_opaque_in::<Cbor, _>(&cbor.arguments).expect("decodes");
    assert_eq!(
        body_from_json, body_from_cbor,
        "one payload, two encodings, one value"
    );
}

// --- the registry sweep ------------------------------------------------------------------

#[test]
fn every_registry_struct_round_trips_in_both_encodings() {
    // The conforming-daemon claim, over the whole declared surface: every operation body
    // and every named struct decodes from a document built out of its own declaration and
    // re-encodes to exactly that document, in both encodings, with one field sequence
    // between them.
    let mut swept = 0_usize;
    for (name, spec) in every_declared_struct() {
        let json = synthesize::<Json>(spec, 0);
        let cbor = synthesize::<Cbor>(spec, 0);
        assert_eq!(
            (spec.json_round_trip)(&json).unwrap_or_else(|error| panic!("`{name}`: {error}")),
            json,
            "`{name}` re-encodes to its own document (JSON)"
        );
        assert_eq!(
            (spec.cbor_round_trip)(&cbor).unwrap_or_else(|error| panic!("`{name}`: {error}")),
            cbor,
            "`{name}` re-encodes to its own document (CBOR)"
        );
        assert_eq!(
            field_sequence(&json),
            field_sequence(&cbor),
            "`{name}` has one field sequence in both encodings"
        );
        // And the byte level: the document is its own canonical form in each encoding.
        let json_bytes = json.to_canonical_bytes();
        assert_eq!(
            Json::parse(&json_bytes).expect("the synthesized JSON parses"),
            json,
            "`{name}` is canonical JSON"
        );
        let cbor_bytes = cbor.to_canonical_bytes();
        assert_eq!(
            Cbor::parse(&cbor_bytes).expect("the synthesized CBOR parses"),
            cbor,
            "`{name}` is canonical CBOR"
        );
        swept += 1;
    }
    assert_eq!(OPERATIONS.len(), OPERATION_COUNT, "the registry is 83 rows");
    assert_eq!(
        swept,
        OPERATION_COUNT * 2 + NAMED_STRUCTS.len(),
        "every operation body and every named struct was swept"
    );
}

#[test]
fn every_operation_the_families_serve_round_trips_through_the_codec() {
    // The claim `codec::operations`' own documentation makes: every variant of `Arguments`
    // its table produces encodes and decodes back to itself. Driven from the registry, so
    // an operation that gained a family and not a table arm would show up as a count
    // mismatch rather than as an untested path.
    let mut served = 0_usize;
    let mut unserved = 0_usize;
    for operation in OPERATIONS {
        let json = synthesize::<Json>(&operation.request, 0);
        let cbor = synthesize::<Cbor>(&operation.request, 0);
        let json_opaque = Opaque::from_bytes(json.to_canonical_bytes());
        let cbor_opaque = Opaque::from_bytes(cbor.to_canonical_bytes());
        match decode_arguments_in::<Json>(operation.name, &json_opaque) {
            Ok(arguments) => {
                served += 1;
                assert_eq!(
                    arguments.operation(),
                    operation.name,
                    "the decoded variant names its own operation"
                );
                let re_encoded = continuumd::transport::encode_arguments_in::<Json>(&arguments)
                    .expect("the body re-encodes");
                assert_eq!(
                    re_encoded, json_opaque,
                    "`{}` round-trips (JSON)",
                    operation.name
                );

                // The same operation, decoded from the same declaration in the other
                // encoding. The two typed values are *not* asserted equal: a request body
                // carrying an `Opaque` of its own — `intent.propose_revision`'s
                // `IntentChangeSet.changes`, for one — holds bytes of the encoding it was
                // read in, which is `rule encoding.opaque_payloads`'s contract and not a
                // gap. What must agree is the field sequence, and
                // `every_registry_struct_round_trips_in_both_encodings` asserts that for
                // this very body.
                let from_cbor = decode_arguments_in::<Cbor>(operation.name, &cbor_opaque)
                    .expect("the same operation decodes in CBOR");
                let re_encoded = continuumd::transport::encode_arguments_in::<Cbor>(&from_cbor)
                    .expect("the body re-encodes");
                assert_eq!(
                    re_encoded, cbor_opaque,
                    "`{}` round-trips (CBOR)",
                    operation.name
                );
            }
            Err(CodecError::UnknownOperation) => unserved += 1,
            Err(error) => panic!("`{}` failed to decode: {error}", operation.name),
        }
    }
    // The split `codec::operations::decode_arguments` documents: 29 served, 45 whose
    // families have not landed. It was 26/47 until bn-28jj (PR-11 / IMPL-04) landed the
    // `context` family, which serves `context.expand` and answers `context.compile` with
    // the typed refusal `rule errors.unsupported_surface` requires — both decode, so both
    // are served here, and 28/45 until bn-1as8e added `whiteboard.compile` at protocol
    // 3.5, which grew the registry rather than the landed set alone. bn-3of5h moved it
    // once more and the other way round: `workspace.create_by_reference` grows the
    // registry *and* the landed set in one step, because the `workspace` family serves it
    // the day it is declared — 30/45. bn-3glnv did the same eight times at protocol 3.8:
    // the six `signing` operations and the two bundle operations are served the day they
    // are declared — 38/45. A count that moves when a family lands is the point: it forces
    // the landing to be visible in a file nobody editing a family would otherwise open.
    assert_eq!(served, 38, "the operations the landed families serve");
    assert_eq!(unserved, OPERATION_COUNT - 38);
}

#[test]
fn a_typed_payload_encodes_into_an_opaque_in_either_encoding() {
    // The result half of the same seam. `Payload::None` is the `null` the IDL declares on
    // `status = error`, in both encodings alike.
    assert_eq!(
        encode_payload_in::<Json>(&Payload::None).expect("encodes"),
        None
    );
    assert_eq!(
        encode_payload_in::<Cbor>(&Payload::None).expect("encodes"),
        None
    );
}

// --- presence, unknown fields, and the closed/open enum split -------------------------

#[test]
fn presence_is_three_valued_in_cbor_exactly_as_in_json() {
    // "Absent and null are distinct and MUST NOT be conflated in either direction"
    // (RFC 0026). The same four ways to conflate them, in the second encoding.
    let mut envelope = create_envelope::<Cbor>();
    envelope.idempotency_key = Optional::Absent;
    let document: Cbor = envelope.encode().expect("encodes");
    let entries = document.as_map().expect("an envelope is a map");
    assert!(
        !entries.contains_key("idempotency_key"),
        "an absent `optional` field is omitted, never null"
    );
    assert_eq!(
        entries.get("intent"),
        Some(&Cbor::Null),
        "a `nullable` field is present and null, never omitted"
    );

    // An `optional` field spelled null is malformed.
    let mut widened = entries.clone();
    widened.insert("idempotency_key".to_owned(), Cbor::Null);
    assert_eq!(
        RequestEnvelope::decode(&Cbor::Map(widened)).expect_err("null optional"),
        CodecError::UnexpectedNull {
            declared_by: "RequestEnvelope",
            field: "idempotency_key",
        }
    );

    // A `nullable` field omitted is malformed.
    let mut narrowed = entries.clone();
    narrowed.remove("intent");
    assert_eq!(
        RequestEnvelope::decode(&Cbor::Map(narrowed)).expect_err("omitted nullable"),
        CodecError::MissingField {
            declared_by: "RequestEnvelope",
            field: "intent",
        }
    );

    // A `required` field spelled null is malformed.
    let mut nulled = entries.clone();
    nulled.insert("operation".to_owned(), Cbor::Null);
    assert_eq!(
        RequestEnvelope::decode(&Cbor::Map(nulled)).expect_err("null required"),
        CodecError::UnexpectedNull {
            declared_by: "RequestEnvelope",
            field: "operation",
        }
    );
}

#[test]
fn an_unknown_field_and_an_unknown_enum_member_behave_the_same_in_cbor() {
    // `rule envelope.unknown_fields` is the protocol's only forward-compatibility
    // mechanism and `rule versioning.enums` is where it stops — both encodings alike,
    // because both are the *same* code with a different document model under it.
    let document: Cbor = target().encode().expect("encodes");
    let entries = document.as_map().expect("a target is a map").clone();

    let mut widened = entries.clone();
    widened.insert("a_field_from_a_later_minor".to_owned(), Cbor::Unsigned(7));
    assert_eq!(
        Target::decode(&Cbor::Map(widened)).expect("an unknown field is ignored"),
        target()
    );

    // `TargetKind` is `@open`: the token is surfaced verbatim rather than rejected.
    let mut open = entries.clone();
    open.insert("kind".to_owned(), Cbor::Text("all_conjectures".to_owned()));
    let error = Target::decode(&Cbor::Map(open)).expect_err("an unknown open member");
    assert_eq!(
        error,
        CodecError::UnknownOpenMember {
            enum_name: "TargetKind",
            token: "all_conjectures".into(),
        }
    );
    assert_eq!(error.code(), ErrorCode::UnsupportedSemanticFeature);

    // `Portfolio` is closed: an unrecognized member is malformed.
    let document: Cbor = Portfolio::Interactive.encode().expect("encodes");
    assert_eq!(document, Cbor::Text("interactive".to_owned()));
    assert_eq!(
        Portfolio::decode(&Cbor::Text("whatever".to_owned())).expect_err("a closed enum"),
        CodecError::UnknownMember {
            enum_name: "Portfolio",
        }
    );
}

#[test]
fn a_union_is_one_key_named_by_its_declared_variant_in_cbor_too() {
    // `rule encoding.union_tagging`, in the second encoding. `refuted` is a member of both
    // `SemanticVerdict` and `EvaluationVerdict`, so the tag is load-bearing.
    let verdict = Verdict::Semantic(SemanticVerdictValue {
        verdict: SemanticVerdict::Refuted,
        inconclusive_reason: Optional::Absent,
        assurance_class: AssuranceClass::Validated,
    });
    let bytes = write_in::<Cbor, _>(&verdict).expect("the verdict encodes");
    assert_eq!(
        hex(&bytes),
        "a16873656d616e746963a26f6173737572616e63655f636c6173736976616c69646174656467766572646963746772656675746564"
    );
    assert_eq!(
        read_in::<Cbor, Verdict>(&bytes).expect("it decodes"),
        verdict,
        "the tag round-trips"
    );
    // Zero keys and two keys are each malformed.
    assert_eq!(
        Verdict::decode(&Cbor::Map(BTreeMap::new())).expect_err("arity"),
        CodecError::UnionArity { union: "Verdict" }
    );
    let mut two = BTreeMap::new();
    two.insert("semantic".to_owned(), Cbor::Null);
    two.insert("structural".to_owned(), Cbor::Null);
    assert_eq!(
        Verdict::decode(&Cbor::Map(two)).expect_err("arity"),
        CodecError::UnionArity { union: "Verdict" }
    );
    let mut undeclared = BTreeMap::new();
    undeclared.insert("conjectural".to_owned(), Cbor::Null);
    assert_eq!(
        Verdict::decode(&Cbor::Map(undeclared)).expect_err("an undeclared variant"),
        CodecError::UnknownVariant { union: "Verdict" }
    );
}

// --- the two leaves the encodings spell differently ------------------------------------

#[test]
fn bytes_and_a_large_u64_are_the_only_leaves_the_two_encodings_spell_differently() {
    // IDL §3, both rows, both directions.
    //
    // `Bytes`: base64url text there, a byte string here.
    let value = Bytes::from([0x00, 0xFF, 0x10, 0x83, 0x7A]);
    assert_eq!(
        write_in::<Json, _>(&value).expect("encodes"),
        b"\"AP8Qg3o\"".to_vec()
    );
    assert_eq!(
        hex(&write_in::<Cbor, _>(&value).expect("encodes")),
        "4500ff10837a"
    );
    assert_eq!(
        read_in::<Cbor, Bytes>(&unhex("4500ff10837a")).expect("decodes"),
        value
    );
    // A *text* string offered to a `Bytes` field in CBOR is a second spelling and is
    // refused, where in JSON it is the only spelling.
    assert_eq!(
        Bytes::decode(&Cbor::Text("AP8Qg3o".to_owned())).expect_err("a text `Bytes`"),
        CodecError::TypeMismatch {
            expected: "Bytes",
            found: "string",
        }
    );

    // `U64`: a number, then a decimal string, in JSON; one unsigned integer in CBOR.
    let exact = (1_u64 << 53) - 1;
    let beyond = exact + 1;
    assert_eq!(
        write_in::<Json, _>(&continuumd::protocol::scalar::ByteCount::new(exact)).expect("encodes"),
        exact.to_string().into_bytes()
    );
    assert_eq!(
        write_in::<Json, _>(&continuumd::protocol::scalar::ByteCount::new(beyond))
            .expect("encodes"),
        format!("\"{beyond}\"").into_bytes()
    );
    assert_eq!(
        hex(
            &write_in::<Cbor, _>(&continuumd::protocol::scalar::ByteCount::new(beyond))
                .expect("encodes")
        ),
        "1b0020000000000000"
    );
    // The decimal string is not a `U64` spelling in CBOR.
    assert_eq!(
        <u64 as ProtocolValue>::decode(&Cbor::Text(beyond.to_string())).expect_err("a text `U64`"),
        CodecError::TypeMismatch {
            expected: "U64",
            found: "string",
        }
    );
    // And the whole range round-trips in both, across the JSON boundary that CBOR does not
    // have.
    for magnitude in [
        0,
        1,
        23,
        24,
        255,
        256,
        65_535,
        65_536,
        exact,
        beyond,
        u64::MAX,
    ] {
        round_trip::<Json, _>(&continuumd::protocol::scalar::DurationMs::new(magnitude));
        round_trip::<Cbor, _>(&continuumd::protocol::scalar::DurationMs::new(magnitude));
    }
}

// --- the canonical subset, derived rather than chosen ------------------------------------

#[test]
fn the_canonical_subset_is_derived_from_one_spelling_not_chosen() {
    // Each row of `cbor`'s derivation table, as a pair: a rejected byte string and the
    // accepted one that denotes the same message. That the two denote the same message is
    // what makes the rejection follow from "one message has exactly one byte spelling"
    // rather than from a preference.
    let cases: &[(&str, &str, &str)] = &[
        ("9f01ff", "8101", "an indefinite-length array"),
        ("810101", "8101", "trailing bytes after the document"),
        ("1801", "01", "a non-shortest one-byte head"),
        ("190001", "01", "a non-shortest two-byte head"),
        ("1a00000001", "01", "a non-shortest four-byte head"),
        ("1b0000000000000001", "01", "a non-shortest eight-byte head"),
        ("c06161", "6161", "a tagged text string"),
        (
            "a2616202626161 01",
            "a262616101616201",
            "length-first key order",
        ),
        ("a2616101616102", "a1616101", "a duplicated key"),
    ];
    for (rejected, accepted, why) in cases {
        let rejected_bytes = unhex(&rejected.replace(' ', ""));
        let accepted_bytes = unhex(&accepted.replace(' ', ""));
        assert!(
            Cbor::parse(&rejected_bytes).is_err(),
            "{why} must be rejected: {rejected}"
        );
        assert!(
            Cbor::parse(&accepted_bytes).is_ok(),
            "the accepted spelling parses: {accepted}"
        );
        assert_ne!(rejected_bytes, accepted_bytes);
    }

    // The values the protocol declares no type for: rejected because they denote no
    // message at all, not because they are a second spelling of one.
    for (bytes, why) in [
        ("20", "a negative integer"),
        ("f97e00", "a half-precision float"),
        ("fa47c35000", "a single-precision float"),
        ("fb3ff199999999999a", "a double-precision float"),
        ("f7", "`undefined`"),
        ("e0", "an unassigned simple value"),
        ("a1010101", "an integer map key"),
        ("1c", "a reserved additional-information value"),
        ("bf616101ff", "an indefinite-length map"),
        ("5f42010243030405ff", "an indefinite-length byte string"),
    ] {
        assert!(
            Cbor::parse(&unhex(bytes)).is_err(),
            "{why} must be rejected: {bytes}"
        );
    }
}

#[test]
fn every_cbor_reader_rejection_is_reachable_and_named() {
    // Each variant of `CborError`, with the input that produces it. A rejection nothing can
    // reach is not a rule; this is the file that makes each one load-bearing.
    let cases: &[(&str, CborError)] = &[
        ("81", CborError::Truncated),
        ("0101", CborError::TrailingBytes),
        ("9f01ff", CborError::IndefiniteLength { at: 0 }),
        ("1801", CborError::NonShortestHead { at: 0 }),
        ("20", CborError::NegativeInteger { at: 0 }),
        ("f93c00", CborError::FloatingPoint { at: 0 }),
        ("c001", CborError::Tag { at: 0 }),
        ("f7", CborError::SimpleValue { at: 0 }),
        ("1c", CborError::Reserved { at: 0 }),
        ("a1010101", CborError::NonTextKey { at: 1 }),
        ("a2616202616101", CborError::KeyOrder { at: 4 }),
        ("6180", CborError::NotUtf8 { at: 0 }),
    ];
    for (bytes, expected) in cases {
        assert_eq!(
            Cbor::parse(&unhex(bytes)).expect_err("a rejection"),
            *expected,
            "input {bytes}"
        );
    }

    // `TooDeep`: the bound is checked before recursing, so an adversarial document is a
    // typed error rather than a stack overflow. `MAX_DEPTH + 2` nested arrays.
    let mut deep = vec![0x81_u8; MAX_DEPTH + 2];
    deep.push(0x01);
    assert_eq!(
        Cbor::parse(&deep).expect_err("too deep"),
        CborError::TooDeep
    );
    let mut admitted = vec![0x81_u8; MAX_DEPTH];
    admitted.push(0x01);
    assert!(
        Cbor::parse(&admitted).is_ok(),
        "the bound itself is admitted"
    );

    // `DuplicateKey` is reachable through the value-model builder; through the *reader* a
    // repeated key is caught one step earlier by the order rule, exactly as it is in JSON.
    assert_eq!(
        Cbor::map([
            ("a".to_owned(), Cbor::Unsigned(1)),
            ("a".to_owned(), Cbor::Unsigned(2)),
        ])
        .expect_err("a duplicate key"),
        CborError::DuplicateKey {
            key: "a".to_owned()
        }
    );
}

#[test]
fn an_inflated_length_costs_a_bounds_check_and_not_an_allocation() {
    // A CBOR head declares a length before its content, which JSON never does. Nine bytes
    // claiming `2^64 − 1` items must not reserve anything: the reader answers `Truncated`
    // after reading what is actually there. OOM hygiene, as a test rather than a comment.
    for bytes in [
        "5bffffffffffffffff",       // a byte string of 2^64 − 1 bytes
        "7bffffffffffffffff",       // a text string of the same
        "9bffffffffffffffff",       // an array of 2^64 − 1 items
        "bbffffffffffffffff",       // a map of 2^64 − 1 pairs
        "9bffffffffffffffff010101", // and with a few items actually present
    ] {
        assert_eq!(
            Cbor::parse(&unhex(bytes)).expect_err("an inflated length"),
            CborError::Truncated,
            "input {bytes}"
        );
    }
}

// --- the mutation sweep -------------------------------------------------------------------

#[test]
fn no_single_byte_mutation_of_a_cbor_golden_vector_decodes_to_the_same_document() {
    // Canonicality, operationally, in the second encoding: one message has one spelling, so
    // two distinct byte strings never denote one document. Every mutant either fails to
    // parse or parses to something *different* — and whatever it parses to is its own
    // canonical form, which is the half a permissive CBOR reader fails, because it would
    // read `18 01` and `01` as the same integer.
    //
    // Three replacement bytes per position rather than all 255, the same budget the JSON
    // sweep took: they cover a head byte, a length byte, and a bit flip inside a string.
    // The sweep is linear in the vectors' total length and allocates one mutant at a time.
    let mut mutants = 0_u64;
    let mut rejected = 0_u64;
    for (name, _, bytes) in golden() {
        let original = Cbor::parse(&bytes).unwrap_or_else(|error| panic!("`{name}`: {error}"));
        for index in 0..bytes.len() {
            for replacement in [0x00, 0xFF, bytes[index] ^ 0x01] {
                if replacement == bytes[index] {
                    continue;
                }
                let mut mutant = bytes.clone();
                mutant[index] = replacement;
                mutants += 1;
                match Cbor::parse(&mutant) {
                    Err(_) => rejected += 1,
                    Ok(parsed) => {
                        assert_ne!(
                            parsed, original,
                            "`{name}` byte {index} -> {replacement:#04x} decoded to the \
                             original document, so two byte strings denote one message"
                        );
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
    // CBOR is denser than JSON — every head byte is structural — so the rejected share is
    // above the JSON sweep's quarter. The floor is stated as a floor, not as a target: the
    // load-bearing assertion is the injectivity one above, and this one only says the
    // sweep is reaching structure rather than only string interiors.
    assert!(
        rejected > mutants / 3,
        "the structural mutations must be rejected outright: {rejected} of {mutants}"
    );
}

#[test]
fn a_mutation_inside_a_typed_field_is_rejected_by_the_type_and_not_only_by_the_reader() {
    // The reader accepts a mutated *text string*; the type layer is what rejects it. Each
    // of these is a document that parses cleanly and is still not a message — the CBOR twin
    // of the JSON file's fourth mutation test.
    let document: Cbor = hello().encode().expect("encodes");
    let entries = document.as_map().expect("a hello is a map").clone();
    for (key, value, why) in [
        ("capability", "builder", "a handle without its class prefix"),
        (
            "actor",
            "nobody:builder",
            "an actor outside the four-member scheme",
        ),
    ] {
        let mut mutated = entries.clone();
        mutated.insert(key.to_owned(), Cbor::Text(value.to_owned()));
        let bytes = Cbor::Map(mutated).to_canonical_bytes();
        assert!(
            Cbor::parse(&bytes).is_ok(),
            "the mutant is still canonical CBOR: {why}"
        );
        assert!(
            read_in::<Cbor, ClientHello>(&bytes).is_err(),
            "the type rejects it: {why}"
        );
    }

    // And a mutation with no JSON counterpart at all: CBOR can spell a *byte string* where
    // the declaration says `String`, which JSON has no way to write. It is a well-formed
    // canonical document and still not a message.
    let mut swapped = entries.clone();
    swapped.insert(
        "client".to_owned(),
        Cbor::Bytes(b"continuum-cli/0".to_vec()),
    );
    let bytes = Cbor::Map(swapped).to_canonical_bytes();
    assert!(
        Cbor::parse(&bytes).is_ok(),
        "a byte string is canonical CBOR"
    );
    assert_eq!(
        read_in::<Cbor, ClientHello>(&bytes).expect_err("a byte string is not a `String`"),
        CodecError::TypeMismatch {
            expected: "String",
            found: "byte string",
        }
    );

    // A `String` field, by contrast, takes any text the IDL declares no pattern for —
    // stated so that the rejections above are read as the type layer working rather than
    // as the reader being suspicious of content.
    let mut empty = entries;
    empty.insert("client".to_owned(), Cbor::Text(String::new()));
    assert!(
        read_in::<Cbor, ClientHello>(&Cbor::Map(empty).to_canonical_bytes()).is_ok(),
        "an unconstrained `String` field takes any string"
    );
}

// --- the wire, end to end ------------------------------------------------------------------

#[test]
fn a_cbor_connection_serves_a_whole_exchange_in_cbor() {
    // The negotiated encoding is a property of the connection, and this is what having
    // negotiated one buys: the server reads the request frame and writes the result frame
    // in CBOR, with no per-message choice and nothing sniffing the bytes.
    use continuum_value::epoch::ProtocolWindow;
    use continuumd::daemon::Daemon;
    use continuumd::daemon::identity::Blake3Identity;
    use continuumd::protocol::handshake::negotiate;
    use continuumd::protocol::registry::ENCODINGS;
    use continuumd::protocol::scalar::Timestamp;
    use continuumd::transport::{LocalPair, Server, client_receive_in, client_send_in};

    let implemented = [ProtocolVersion::new(3, 3)];
    let negotiated = negotiate(&implemented, ProtocolWindow::new(3), ENCODINGS, &hello())
        .expect("the handshake succeeds");
    assert_eq!(
        negotiated.encoding(),
        Encoding::CanonicalCbor,
        "the hello offers CBOR first, so CBOR is what was negotiated"
    );

    let daemon = Daemon::builder(
        Blake3Identity,
        negotiated,
        CapabilityHandle::new("cap_builder").expect("a well-formed capability"),
    )
    .epochs(epochs())
    .now(Timestamp::new("2026-08-01T00:00:00.000Z").expect("a timestamp"))
    .build();
    let mut server = Server::new(daemon, negotiated);
    let mut pair = LocalPair::new();

    // `evidence.link` is the 73rd operation and one this daemon's tables decode, so the
    // request is decoded rather than refused at the codec — and the daemon, built here
    // with no families and no capability grants, answers it with a typed error. That is
    // the path worth exercising: the refusal travels exactly the bytes an answer would,
    // so a wrong encoding on either leg would be a decode failure rather than a subtle
    // difference in the body.
    let envelope = request_envelope("evidence.link", Opaque::from_bytes(Vec::new()));
    let arguments = continuumd::daemon::family::Arguments::EvidenceLink(link_request());
    client_send_in::<Cbor>(&mut pair, &envelope, &arguments).expect("the request is framed");

    // The frame on the wire is CBOR, not JSON: its head byte is a map head, and it is not
    // canonical JSON at all.
    let frame = pair
        .to_server
        .take_frame()
        .expect("a frame")
        .expect("a whole frame");
    assert_eq!(frame[0] >> 5, 5, "the request frame is a CBOR map");
    assert!(
        Json::parse(&frame).is_err(),
        "a CBOR frame is not canonical JSON"
    );
    pair.to_server.put_frame(&frame).expect("re-framed");

    assert_eq!(server.serve(&mut pair).expect("the server answers"), 1);
    let result_frame = pair
        .to_client
        .take_frame()
        .expect("a frame")
        .expect("a whole frame");
    assert_eq!(
        result_frame[0] >> 5,
        5,
        "the result frame is a CBOR map too"
    );
    assert!(
        Json::parse(&result_frame).is_err(),
        "a CBOR result frame is not canonical JSON"
    );
    pair.to_client.put_frame(&result_frame).expect("re-framed");

    let (result, payload) = client_receive_in::<Cbor>(&mut pair, "evidence.link")
        .expect("the result decodes")
        .expect("a result frame");
    assert_eq!(
        result.request_id, envelope.request_id,
        "the echo is truthful"
    );
    assert_eq!(
        result.status,
        ResultStatus::Error,
        "a daemon with no capability grants refuses, and refuses in CBOR"
    );
    assert_eq!(payload, Payload::None, "an error carries a null payload");
    assert!(
        matches!(result.payload, Nullable::Null),
        "`payload` is null on `status = error`, which is what the IDL declares"
    );
}
