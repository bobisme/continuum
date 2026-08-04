//! Evidence for `continuum context expand` (bn-3tz60, PR-13 third command group; flipped to
//! the live path by bn-28jj, PR-11 / IMPL-04).
//!
//! # What changed with bn-28jj, and why these tests moved
//!
//! When bn-3tz60 landed, `continuumd` served no `context` namespace: every call reached
//! `codec::operations::decode_arguments`, found no arm for `context.expand`, and came back
//! `UnsupportedSemanticFeature`. Three tests here asserted exactly that, and the fourth —
//! the success-path renderer — was proven at unit level against the real wire types because
//! "a renderer that only handled the refusal it happens to see today would be untested on
//! the day the family lands".
//!
//! bn-28jj is that day. `continuumd::daemon::context::ContextFamily` serves the namespace,
//! so the refusal those three tests pinned no longer exists to pin: a `context.expand`
//! against a registered pack is *admitted*. They are replaced here by their live
//! counterparts, which assert the same three properties over a real frame and a real
//! answer — the typed reason is still the protocol's own token where a refusal is still
//! reachable, the `omissions` key is still present in machine output on both arms, and the
//! renderer still prints the expanded pack beside the full manifest. The unit-level success
//! test is unchanged and kept: it is the one that proves the renderer against a
//! hand-built answer rather than against whatever this fixture happens to produce.
//!
//! # Clause → test
//!
//! - **"surfaces the omission manifest (INV-007) alongside the expanded slice"** →
//!   [`the_live_success_path_prints_the_expanded_pack_beside_the_full_omission_manifest`]
//!   (over a real daemon) and
//!   [`the_success_renderer_prints_the_expanded_pack_beside_the_full_omission_manifest`]
//!   (unit-level, against the real wire types).
//! - **"non-suppressible in machine output"** →
//!   [`the_json_envelope_always_carries_the_omissions_key`] — the key is present on both
//!   arms, refused or admitted, and there is no flag anywhere in `cli::run`'s parser that
//!   removes it (`cli.rs` has no `--no-omissions`/`--quiet` flag at all).
//! - **a typed refusal is still a rendered answer** →
//!   [`the_cli_renders_a_refusal_with_the_typed_reason_in_every_format`], driven now by an
//!   anchor the pack does not carry rather than by an unserved namespace.
//!
//! # The fixture
//!
//! One in-process deployment: a real `Daemon` with the `context` family registered and one
//! Context Pack registered out of band (`DaemonState::put_context_pack`, the surface a
//! deployment uses because pack *compilation* is not an operation — see
//! `continuumd::daemon::context`). Duplicated locally rather than imported, for the reason
//! `continuum-mcp/tests/typed_surface.rs`'s identical comment gives: a `tests/*.rs` file is
//! its own crate.
//!
//! The pack's expansion payload carries `source` items because those are the items this
//! workspace can build — `SelectedItem`'s typed constructors are `SourceRef`'s and
//! `ModelActionRef`'s (PR-11 / IMPL-03) — so the relation the CLI drives here is
//! `source_span`. bn-3tz60's fixture named `causal_predecessors`, which no landed
//! constructor can supply items for; the request shape it exercises is identical either way.

use continuum_cli::format::Format;
use continuum_cli::wire::{Connection, LocalLink, Outcome};
use continuum_cli::{context, render};
use continuum_context::expansion::{
    ExpansionPayload, ExpansionQuery, ExpansionRelation as PackRelation,
};
use continuum_context::omission::{OmissionReason as PackReason, OmissionRecord};
use continuum_context::selection::SelectionKind;
use continuum_context::source::{SourceRef, SourceSpan};
use continuum_intent::canonical_json::Json;
use continuum_value::epoch::ProtocolWindow;
use continuum_value::value::Name;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::Daemon;
use continuumd::daemon::context::{ContextFamily, ContextPackRecord};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::protocol::envelope::{ArtifactRef, Cost, EpochSet, NextOperation, Omission};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, VersionRange, negotiate,
};
use continuumd::protocol::operations::context::ContextExpandRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, ContextHandle, EpochIdentity, Opaque, ProtocolVersion, RequestId,
    Timestamp, WorkspaceHandle,
};
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{Encoding, ErrorCode, ExpansionRelation, ResultStatus};
use continuumd::transport::{LocalPair, Server};

const NOW: &str = "2026-08-01T00:00:00.000Z";
const PARENT: &str = "ctx_abc123def456";

/// A conforming parent pack advertising the one anchor this deployment holds a group for.
const PARENT_PACK: &str = r#"{
  "assurance": {"class": "bounded", "envelope": {}},
  "content_budget": {"bytes": 16384},
  "content_hash": "blake3-256:parentplaceholder",
  "context_id": "ctx_abc123def456",
  "evidence": ["ev_failure1"],
  "expansions": [{"anchor": "node-42", "relation": "source_span"}],
  "guarantees": ["ReplayPreserving"],
  "intent": "in_ack_v1",
  "omissions": [{"count": 2, "expandable": true,
                 "expansion": {"anchor": "node-42", "relation": "source_span"},
                 "kind": "source", "reason": "budget"}],
  "parent": null,
  "question": "why did AckImpliesDurable fail?",
  "replay": "crash_demo1",
  "schema_epoch": 1,
  "schema_id": "https://continuum.dev/schema/context-pack.json",
  "selected": [{"artifact": "ev_ack1", "id": "node-42", "kind": "event",
                "summary": "reply published before stable write"}],
  "semantic_epoch": "sem3-r3-demo",
  "snapshot": "ws_demo1",
  "verdict": "refuted"
}"#;

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

fn name(text: &str) -> Name {
    Name::new(text).expect("a canonical identifier")
}

fn source_item(id: &str, line: u32) -> continuum_context::selection::SelectedItem {
    let span = SourceSpan::new(
        WorkspacePath::new("src/ack.rs").expect("a repo-relative path"),
        line,
        1,
        line,
        40,
    )
    .expect("a well-formed span");
    SourceRef::new(span).into_selected_item(name(id))
}

fn pack_record() -> ContextPackRecord {
    let group = ExpansionPayload::new(
        OmissionRecord::expandable(
            SelectionKind::Source,
            2,
            PackReason::Budget,
            ExpansionQuery::new(PackRelation::SourceSpan, name("node-42")),
        ),
        vec![source_item("span_1", 10), source_item("span_2", 20)],
    )
    .expect("two items for a count of two");
    ContextPackRecord::new(
        Json::parse(PARENT_PACK.as_bytes()).expect("the fixture is admissible JSON"),
        WorkspaceHandle::new("ws_demo1").expect("a workspace handle"),
        [group],
    )
    .expect("a conforming pack and a well-formed expansion graph")
}

/// A deployment serving the `context` namespace over one registered pack.
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
        let mut daemon = Daemon::builder(Blake3Identity, negotiated, capability("cap_root"))
            .epochs(epochs())
            .now(Timestamp::new(NOW).expect("a timestamp"))
            .capability(root_grant(), None)
            .family(ContextFamily)
            .build();
        daemon.state_mut().put_context_pack(
            ContextHandle::new(PARENT).expect("a context handle"),
            pack_record(),
        );
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

fn request(anchor: &str) -> ContextExpandRequest {
    ContextExpandRequest {
        context: ContextHandle::new(PARENT).expect("a well-formed context handle"),
        anchor: anchor.to_owned(),
        relation: ExpansionRelation::SourceSpan,
        depth: Optional::Present(1),
    }
}

fn args(anchor: &str) -> context::ExpandArgs {
    context::ExpandArgs {
        context: request(anchor).context,
        anchor: request(anchor).anchor,
        relation: request(anchor).relation,
        depth: request(anchor).depth,
        states: Optional::Present(0),
    }
}

fn call(
    fixture: &mut Fixture,
    anchor: &str,
) -> Outcome<continuumd::protocol::operations::context::ContextExpandResponse> {
    let mut connection = connection();
    let mut link = fixture.link();
    connection
        .context_expand(&mut link, &request(anchor), Optional::Present(0))
        .expect("the call reaches a real frame and a real answer")
}

// --- the evidence -------------------------------------------------------------------------

#[test]
fn a_real_daemon_admits_context_expand_and_answers_with_a_child_pack() {
    // The flip. Before bn-28jj this call could only be refused
    // `UnsupportedSemanticFeature`, because `codec::operations::decode_arguments` had no arm
    // for the operation; the family has landed, so the same frame is admitted and the answer
    // carries a pack whose parent is the one the request named.
    let mut fixture = Fixture::fresh();
    let outcome = call(&mut fixture, "node-42");
    let Outcome::Admitted(admitted) = outcome else {
        panic!("context.expand is served now and must be admitted: {outcome:?}");
    };
    assert_eq!(admitted.status, ResultStatus::Ok);
    assert_eq!(admitted.payload.parent.as_str(), PARENT);
    assert!(admitted.payload.context.as_str().starts_with("ctx_"));
    assert_ne!(admitted.payload.context.as_str(), PARENT);
    assert!(
        !admitted.payload.pack.as_bytes().is_empty(),
        "the answer carries the child pack itself, not a reference to one"
    );
}

#[test]
fn the_live_success_path_prints_the_expanded_pack_beside_the_full_omission_manifest() {
    let mut fixture = Fixture::fresh();
    let outcome = call(&mut fixture, "node-42");
    let expanded = match &outcome {
        Outcome::Admitted(admitted) => admitted.payload.context.as_str().to_owned(),
        other => panic!("expected an admitted answer: {other:?}"),
    };

    for format in [Format::Text, Format::Pretty] {
        let rendered = context::render(&args("node-42"), &outcome, format);
        assert_eq!(rendered.exit_code, 0, "an admitted answer exits zero");
        assert!(rendered.text.contains(&expanded));
        assert!(
            rendered.text.contains("omissions"),
            "the manifest line is present in {format:?}:\n{}",
            rendered.text
        );
        // The shared output contract, which this command joined in bn-ybh1z: the registry
        // operation and the typed depth on every arm, the request's expansion distance under
        // `expand_depth` (because `depth` is the contract's own key), and the child pack's
        // length beside the document the machine channel embeds.
        assert!(
            rendered.text.contains("operation  context.expand"),
            "{format:?}:\n{}",
            rendered.text
        );
        assert!(
            rendered.text.contains("depth  served"),
            "{format:?}:\n{}",
            rendered.text
        );
        assert!(
            rendered.text.contains("expand_depth  "),
            "{format:?}:\n{}",
            rendered.text
        );
        assert!(
            rendered.text.contains("pack_bytes  "),
            "{format:?}:\n{}",
            rendered.text
        );
        // The CLI-authored `status: ok|refused` token this command carried before bn-ybh1z
        // is gone: `depth` says the same thing, typed, and finer.
        assert!(
            !rendered.text.contains("status  ok"),
            "{format:?}:\n{}",
            rendered.text
        );
    }

    let json = context::render(&args("node-42"), &outcome, Format::Json);
    assert_eq!(json.exit_code, 0);
    assert!(json.text.contains(&format!("\"expanded\":\"{expanded}\"")));
    assert!(json.text.contains("\"pack_bytes\":"), "{}", json.text);
    // The pack is embedded verbatim, so the child's own two selected spans are readable in
    // the machine output without a second call.
    assert!(json.text.contains("\"span_1\""), "{}", json.text);
    assert!(json.text.contains("\"span_2\""), "{}", json.text);
    // …and so is the child's own manifest, which here asserts completeness: the group the
    // parent named is exactly what came back, and nothing further is outstanding.
    assert!(json.text.contains("\"omissions\":[]"), "{}", json.text);
}

#[test]
fn the_cli_renders_a_refusal_with_the_typed_reason_in_every_format() {
    // A refusal is still a rendered answer rather than an error, and the reason is still the
    // protocol's own wire token. What produces it has changed: an anchor the pack does not
    // carry, rather than a namespace nothing served.
    let mut fixture = Fixture::fresh();
    let outcome = call(&mut fixture, "node-99");
    let Outcome::Refused(ref refusal) = outcome else {
        panic!("an anchor the pack does not carry must be refused: {outcome:?}");
    };
    assert_eq!(refusal.code, ErrorCode::MalformedRequest);
    assert!(
        !refusal.detail.is_empty(),
        "RFC 0026 requires a non-empty detail"
    );
    for format in [Format::Text, Format::Pretty, Format::Json] {
        let rendered = context::render(&args("node-99"), &outcome, format);
        assert_eq!(
            rendered.exit_code, 1,
            "a refusal exits non-zero: {format:?}"
        );
        assert!(
            rendered
                .text
                .contains(ErrorCode::MalformedRequest.as_wire()),
            "the typed reason is the protocol's own wire token, not a synonym, in {format:?}:\n{}",
            rendered.text
        );
    }

    // The machine envelope keeps the whole response key set on the refusal arm, explicitly
    // `null` — bn-ybh1z: before it, a refused `context expand` answered `{error, omissions,
    // advice}` and a parser had to branch on which keys existed before it could read one.
    let json = context::render(&args("node-99"), &outcome, Format::Json);
    for key in ["expanded", "parent", "pack", "pack_bytes"] {
        assert!(
            json.text.contains(&format!("\"{key}\":null")),
            "{key} is present and null on the refusal arm:\n{}",
            json.text
        );
    }
}

#[test]
fn the_json_envelope_always_carries_the_omissions_key() {
    // Both arms, non-suppressibly: there is no flag anywhere in `cli.rs`'s parser that
    // removes it.
    let mut fixture = Fixture::fresh();
    for anchor in ["node-42", "node-99"] {
        let outcome = call(&mut fixture, anchor);
        let rendered = context::render(&args(anchor), &outcome, Format::Json);
        assert!(
            rendered.text.contains("\"omissions\":["),
            "the manifest key is present on every arm:\n{}",
            rendered.text
        );
    }
}

/// Unit-level, against the real wire types directly. Kept unchanged from bn-3tz60: the live
/// test above renders whatever this deployment's pack happens to hold, and this one renders
/// an answer built to exercise both an expandable and an irretrievable omission — including
/// the `recoverable_by` handle the live fixture's completed expansion has no occasion to
/// carry.
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
        verdict: None,
        assurance: None,
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
