//! Evidence for `continuum context expand` (bn-3tz60, PR-13 third command group).
//!
//! # Clause → test
//!
//! - **"surfaces the omission manifest (INV-007) alongside the expanded slice"** →
//!   [`the_success_renderer_prints_the_expanded_pack_beside_the_full_omission_manifest`],
//!   unit-level against the real wire types directly (see the crate root doc: no live
//!   daemon can answer this call successfully today, since PR-11/IMPL-04, bn-28jj, is
//!   open).
//! - **"non-suppressible in machine output"** →
//!   [`the_json_envelope_always_carries_the_omissions_key`] — the key is present on both
//!   arms, refused or admitted, and there is no flag anywhere in `cli::run`'s parser that
//!   removes it (`cli.rs` has no `--no-omissions`/`--quiet` flag at all).
//! - **the real, wire-reachable behavior today** →
//!   [`a_real_daemon_refuses_context_expand_with_a_typed_unsupported_semantic_feature`] —
//!   a real frame, a real `Daemon::dispatch`-adjacent `Server::answer`, and the crate's own
//!   `Connection::context_expand` decoding the real refusal.
//!
//! # The fixture
//!
//! Deliberately minimal, unlike `task_lifecycle.rs`'s: `context.expand` refuses at
//! `codec::operations::decode_arguments` — before family lookup, before admission, before
//! any capability is checked (`continuumd::transport::Server::answer`'s own control flow) —
//! so no corpus, no intent, and no operation family is staged here. Duplicated locally
//! rather than imported, for the reason `continuum-mcp/tests/typed_surface.rs`'s identical
//! comment gives: a `tests/*.rs` file is its own crate.

use continuum_cli::format::Format;
use continuum_cli::wire::{Connection, LocalLink, Outcome};
use continuum_cli::{context, render};
use continuum_value::epoch::ProtocolWindow;
use continuumd::daemon::Daemon;
use continuumd::daemon::identity::Blake3Identity;
use continuumd::protocol::envelope::{ArtifactRef, Cost, EpochSet, NextOperation, Omission};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, VersionRange, negotiate,
};
use continuumd::protocol::operations::context::ContextExpandRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, ContextHandle, EpochIdentity, Opaque, ProtocolVersion, RequestId,
    Timestamp,
};
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{Encoding, ErrorCode, ExpansionRelation, ResultStatus};
use continuumd::transport::{LocalPair, Server};

const NOW: &str = "2026-08-01T00:00:00.000Z";

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 2)
}

fn actor(name: &str) -> ActorId {
    ActorId::new(name).expect("a well-formed actor identity")
}

fn capability(name: &str) -> CapabilityHandle {
    CapabilityHandle::new(name).expect("a well-formed capability handle")
}

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
}

fn epochs() -> EpochSet {
    EpochSet {
        protocol: version(),
        semantic: Nullable::Value(epoch("semantic-1")),
        intent: Nullable::Value(epoch("intent-1")),
        evidence: Nullable::Null,
        proof: Nullable::Value(epoch("proof-1")),
        corpus: Nullable::Null,
        engine: Nullable::Value(epoch("engine-reference-1")),
    }
}

fn hello() -> ClientHello {
    ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuum-cli-evidence".to_owned(),
        actor: actor("service:continuumd"),
        capability: capability("cap_root"),
        features: Optional::Absent,
    }
}

fn root_grant() -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: capability("cap_root"),
        actor: actor("service:continuumd"),
        level: continuumd::protocol::vocabulary::AuthorityLevel::Promote,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: 4,
        profile: Optional::Present(CapabilityProfile {
            privileged_operations: Vec::new(),
            denied_operations: Vec::new(),
            data_grants: Vec::new(),
            cross_principal_sharing: true,
        }),
    }
}

/// A minimal deployment: no operation family registered — none is needed, since
/// `context.expand` refuses before family lookup (see this file's module doc).
struct Fixture {
    server: Server,
    pair: LocalPair,
}

impl Fixture {
    fn fresh() -> Self {
        let hello = hello();
        let negotiated = negotiate(
            &[
                ProtocolVersion::new(3, 0),
                ProtocolVersion::new(3, 1),
                version(),
            ],
            ProtocolWindow::new(3),
            ENCODINGS,
            &hello,
        )
        .expect("3.2 is served");
        let daemon = Daemon::builder(Blake3Identity, negotiated, capability("cap_root"))
            .epochs(epochs())
            .now(Timestamp::new(NOW).expect("a timestamp"))
            .capability(root_grant(), None)
            .build();
        Self {
            server: Server::new(daemon, negotiated),
            pair: LocalPair::new(),
        }
    }

    fn link(&mut self) -> LocalLink<'_> {
        LocalLink::new(&mut self.server, &mut self.pair)
    }
}

fn connection() -> Connection {
    Connection::new(
        version(),
        actor("service:continuumd"),
        capability("cap_root"),
    )
}

fn request() -> ContextExpandRequest {
    ContextExpandRequest {
        context: ContextHandle::new("ctx_abc123def456").expect("a well-formed context handle"),
        anchor: "node-42".to_owned(),
        relation: ExpansionRelation::CausalPredecessors,
        depth: Optional::Present(2),
    }
}

// --- the evidence -------------------------------------------------------------------------

#[test]
fn a_real_daemon_refuses_context_expand_with_a_typed_unsupported_semantic_feature() {
    let mut fixture = Fixture::fresh();
    let mut connection = connection();
    let outcome = {
        let mut link = fixture.link();
        connection
            .context_expand(&mut link, &request(), Optional::Present(0))
            .expect("the call reaches a real frame and a real answer")
    };
    let Outcome::Refused(refusal) = outcome else {
        panic!("context.expand has no family yet and must refuse: {outcome:?}");
    };
    assert_eq!(refusal.code, ErrorCode::UnsupportedSemanticFeature);
    assert!(
        !refusal.detail.is_empty(),
        "RFC 0026 requires a non-empty detail"
    );
    // Not asserted: `retryable`. Two different call sites reach this refusal — the
    // dispatcher's own `unsupported_surface` and the codec's `decode_arguments` miss — and
    // nothing in this bone's scope requires them to agree on that flag; only the code does.
}

#[test]
fn the_cli_renders_the_refusal_with_the_typed_reason_in_every_format() {
    let mut fixture = Fixture::fresh();
    let mut connection = connection();
    let outcome = {
        let mut link = fixture.link();
        connection
            .context_expand(&mut link, &request(), Optional::Present(0))
            .expect("the call is answered")
    };
    for format in [Format::Text, Format::Pretty, Format::Json] {
        let rendered = context::render(
            &continuum_cli::context::ExpandArgs {
                context: request().context,
                anchor: request().anchor,
                relation: request().relation,
                depth: request().depth,
                states: Optional::Present(0),
            },
            &outcome,
            format,
        );
        assert_eq!(
            rendered.exit_code, 1,
            "a refusal exits non-zero: {format:?}"
        );
        assert!(
            rendered
                .text
                .contains(ErrorCode::UnsupportedSemanticFeature.as_wire()),
            "the typed reason is the protocol's own wire token, not a synonym, in {format:?}:\n{}",
            rendered.text
        );
    }
}

#[test]
fn the_json_envelope_always_carries_the_omissions_key() {
    let mut fixture = Fixture::fresh();
    let mut connection = connection();
    let outcome = {
        let mut link = fixture.link();
        connection
            .context_expand(&mut link, &request(), Optional::Present(0))
            .expect("the call is answered")
    };
    let rendered = context::render(
        &continuum_cli::context::ExpandArgs {
            context: request().context,
            anchor: request().anchor,
            relation: request().relation,
            depth: request().depth,
            states: Optional::Present(0),
        },
        &outcome,
        Format::Json,
    );
    assert!(
        rendered.text.contains("\"omissions\":[]"),
        "the manifest key is present even when empty, non-suppressibly:\n{}",
        rendered.text
    );
}

/// Unit-level, against the real wire types directly (see the module doc): the day
/// PR-11/IMPL-04 lands and a `context.expand` answer can be admitted, this is the renderer
/// that will run, and it is proven correct now rather than left untested until then.
#[test]
fn the_success_renderer_prints_the_expanded_pack_beside_the_full_omission_manifest() {
    use continuum_cli::wire::Admitted;
    use continuumd::protocol::operations::context::ContextExpandResponse;

    let pack_bytes = br#"{"selected":[]}"#.to_vec();
    let response = ContextExpandResponse {
        context: ContextHandle::new("ctx_expanded000001").expect("a context handle"),
        parent: ContextHandle::new("ctx_abc123def456").expect("a context handle"),
        pack: Opaque::from_bytes(pack_bytes),
    };
    let omissions = vec![
        Omission {
            reason: continuumd::protocol::vocabulary::OmissionReason::Budget,
            subject: "context.depth".to_owned(),
            recoverable_by: Optional::Present(
                continuumd::protocol::scalar::ArtifactHandle::new("ctx_expanded000001")
                    .expect("a well-formed artifact handle"),
            ),
        },
        Omission {
            reason: continuumd::protocol::vocabulary::OmissionReason::Redaction,
            subject: "node-99.source".to_owned(),
            recoverable_by: Optional::Absent,
        },
    ];
    let outcome: Outcome<ContextExpandResponse> = Outcome::Admitted(Admitted {
        request_id: RequestId::new("req_cli000001").expect("a request id"),
        status: ResultStatus::Ok,
        payload: response,
        task: None,
        continuation: None,
        omissions,
        artifacts: Vec::<ArtifactRef>::new(),
        cost: Cost {
            wall_ms: Optional::Absent,
            cpu_ms: Optional::Absent,
            memory_bytes: Optional::Absent,
            states: Optional::Absent,
            solver_ms: Optional::Absent,
            proof_ms: Optional::Absent,
            tokens: Optional::Absent,
            candidates: Optional::Absent,
            bytes: Optional::Absent,
            tokenizer_id: Optional::Absent,
        },
        next_operations: Vec::<NextOperation>::new(),
    });

    let args = continuum_cli::context::ExpandArgs {
        context: ContextHandle::new("ctx_abc123def456").expect("a context handle"),
        anchor: "node-42".to_owned(),
        relation: ExpansionRelation::CausalPredecessors,
        depth: Optional::Present(2),
        states: Optional::Present(0),
    };

    // Every format prints the expanded slice's own handle and both omissions, unabridged —
    // never a count-and-expand summary (see `render.rs`'s module doc for why this command
    // does not take the lossy shortcut `continuum_benchmark::shell` documents taking).
    for format in [Format::Text, Format::Pretty] {
        let rendered = context::render(&args, &outcome, format);
        assert_eq!(rendered.exit_code, 0);
        assert!(rendered.text.contains("ctx_expanded000001"));
        assert!(rendered.text.contains("context.depth"));
        assert!(rendered.text.contains("node-99.source"));
        assert!(rendered.text.contains("omissions  2"));
    }

    let json = context::render(&args, &outcome, Format::Json);
    assert_eq!(json.exit_code, 0);
    assert!(json.text.contains("\"expanded\":\"ctx_expanded000001\""));
    assert!(
        json.text.contains("\"selected\":[]"),
        "the pack is embedded verbatim, not stringified: {}",
        json.text
    );
    let omissions_only = render::omissions_json(&[]);
    assert_eq!(
        omissions_only,
        continuumd::codec::json::Json::Array(Vec::new())
    );
}
