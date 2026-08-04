//! Evidence for `continuum evidence show` (bn-1g7e4, PR-13 second command group) — the
//! promotion half, and the one command in the group whose backing service has landed.
//!
//! Everything here runs through `continuum_cli::evidence::show` over a real `LocalPair`
//! boundary against a real, provisioned `Daemon` serving
//! `continuumd::daemon::evidence::EvidenceFamily`. No renderer is exercised against a
//! hand-built answer in this file: every assertion is about what a daemon actually said.
//!
//! # Clause → test
//!
//! - **"queries the PR-7 evidence graph by handle"** →
//!   [`show_renders_the_node_a_real_daemon_holds_under_that_handle`], in all three formats,
//!   with the node document embedded verbatim in machine output.
//! - **"bad handle → the daemon's typed refusal rendered faithfully"** →
//!   [`a_handle_the_graph_does_not_hold_is_the_daemons_own_capability_denial`]. RFC 0027 X2:
//!   a read of a node the caller is not authorized for is `CapabilityDenied` *whether or not
//!   it exists*, because a distinct not-found is an existence oracle — and this command adds
//!   no "no such node" gloss of its own.
//! - **a refusal is not an unsupported surface** →
//!   [`a_denial_reports_a_refused_depth_and_never_an_unsupported_one`], the contrast that
//!   makes the `depth` token in `tests/debug_repair_surface.rs` mean something.
//! - **INV-007, non-suppressible, and honest degradation** →
//!   [`asking_for_inline_content_is_admitted_with_a_typed_omission_naming_it`] — the daemon
//!   has no bounded-content channel, so the answer carries the record *plus* the typed
//!   omission `evidence.inline_content` with reason `unsupported`, which this command prints
//!   unabridged.
//! - **withheld is not absent (RFC 0026)** →
//!   [`a_redacted_reference_renders_the_typed_stub_beside_the_record`]: the `Redacted` stub
//!   field by field, *and* the `redaction` omission, *and* the node record itself — three
//!   obligations, none substituting for another.
//! - **the machine contract, pinned** →
//!   [`the_denial_json_document_is_pinned_byte_for_byte`].
//!
//! # The fixture
//!
//! `EvidenceFamily` plus `ObserveFamily`, one staged trace, and one `observe.ingest` that
//! appends the node the tests read back. The ingest is fixture setup — it is not part of
//! this bone's command surface — so it is dispatched directly against the `Daemon`, exactly
//! as `tests/task_lifecycle.rs` dispatches its own setup. Duplicated locally rather than
//! imported, for the reason `continuum-mcp/tests/typed_surface.rs`'s identical comment
//! gives: a `tests/*.rs` file is its own crate.

use continuum_cli::evidence::{self, ShowArgs};
use continuum_cli::format::Format;
use continuum_cli::render::Depth;
use continuum_cli::wire::{Connection, LocalLink, Outcome};
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::evidence::EvidenceFamily;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::daemon::{Daemon, OperationRequest};
use continuumd::protocol::envelope::{Budget, Redacted, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, VersionRange, negotiate,
};
use continuumd::protocol::operations::observe::ObserveIngestRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, EvidenceHandle, Opaque, OperationName, ProtocolVersion,
    RequestId, Timestamp,
};
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, DataGrant, Encoding, ErrorCode, OmissionReason, RedactionReason, ResultStatus,
};
use continuumd::transport::{LocalPair, Server};

/// A production trace, as a captured bundle would arrive.
const TRACE: &str = "{\"events\":[{\"at\":0,\"op\":\"fill\"},{\"at\":1,\"op\":\"pour\"}]}\n";

/// The instrumentation profile the trace was captured under (plan §18.4).
const PROFILE: &str = "otel-1.0/sampled";

const NOW: &str = "2026-08-01T00:00:00.000Z";

/// A well-formed `ev_` handle no ingest in this fixture ever appends.
const ABSENT: &str = "ev_neverappended01";

// --- fixture, duplicated locally (see the module doc) --------------------------------------

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 2)
}

fn actor(name: &str) -> ActorId {
    ActorId::new(name).expect("a well-formed actor identity")
}

fn capability(name: &str) -> CapabilityHandle {
    CapabilityHandle::new(name).expect("a well-formed capability handle")
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

fn traced(grants: &[DataGrant]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: Vec::new(),
        denied_operations: Vec::new(),
        data_grants: grants.to_vec(),
        cross_principal_sharing: true,
    }
}

fn grant(
    handle: &str,
    who: &str,
    level: AuthorityLevel,
    profile: Optional<CapabilityProfile>,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: capability(handle),
        actor: actor(who),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: 3,
        profile,
    }
}

fn root_grant() -> CapabilityDescriptor {
    CapabilityDescriptor {
        delegation_depth: 4,
        ..grant(
            "cap_root",
            "service:continuumd",
            AuthorityLevel::Promote,
            Optional::Present(traced(&[DataGrant::ProductionTrace])),
        )
    }
}

/// One provisioned deployment behind a byte boundary, holding one appended evidence node.
struct Fixture {
    server: Server,
    pair: LocalPair,
    /// The node `observe.ingest` appended — the handle every green-path test reads.
    node: EvidenceHandle,
    /// The staged trace's content identity, reused as a redaction's commitment.
    trace: Commitment,
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

        let root = Some(capability("cap_root"));
        let mut daemon = Daemon::builder(Blake3Identity, negotiated, capability("cap_root"))
            .now(Timestamp::new(NOW).expect("a timestamp"))
            .capability(root_grant(), None)
            .capability(
                grant(
                    "cap_observer",
                    "agent:observer",
                    AuthorityLevel::Execute,
                    Optional::Present(traced(&[DataGrant::ProductionTrace])),
                ),
                root.clone(),
            )
            .capability(
                grant(
                    "cap_reader",
                    "agent:reader",
                    AuthorityLevel::Read,
                    Optional::Absent,
                ),
                root,
            )
            .family(EvidenceFamily::new())
            .family(ObserveFamily)
            .build();

        let trace = daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new("traces/die-hard.jsonl").expect("a workspace path"),
                TRACE.as_bytes().to_vec(),
            )
            .expect("staging names its content");
        let node = ingest(&mut daemon, &trace);

        Self {
            server: Server::new(daemon, negotiated),
            pair: LocalPair::new(),
            node,
            trace,
        }
    }

    /// The byte boundary a CLI command's own calls travel over.
    fn link(&mut self) -> LocalLink<'_> {
        LocalLink::new(&mut self.server, &mut self.pair)
    }

    /// Record that the node's referenced content is withheld — the administrative surface a
    /// deployment's retention policy would use.
    fn redact(&mut self) {
        let commitment = self.trace.clone();
        let node = self.node.clone();
        self.server.daemon_mut().state_mut().redact_evidence(
            &node,
            Redacted {
                redacted: true,
                reason: RedactionReason::Purged,
                commitment,
                original_class: "evidence".to_owned(),
            },
        );
    }
}

fn ingest(daemon: &mut Daemon, trace: &Commitment) -> EvidenceHandle {
    let envelope = RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new("req_ingest").expect("a well-formed request id"),
        idempotency_key: Optional::Present("idem-ingest".to_owned()),
        actor: actor("agent:observer"),
        capability: capability("cap_observer"),
        operation: OperationName::new("observe.ingest").expect("a registry name"),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: Optional::Present(Budget {
            wall_ms: Optional::Absent,
            cpu_ms: Optional::Absent,
            memory_bytes: Optional::Absent,
            states: Optional::Absent,
            solver_ms: Optional::Absent,
            proof_ms: Optional::Absent,
            tokens: Optional::Absent,
            candidates: Optional::Absent,
            bytes: Optional::Absent,
        }),
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    };
    let outcome = daemon.dispatch(&OperationRequest {
        envelope,
        arguments: Arguments::ObserveIngest(ObserveIngestRequest {
            trace: trace.clone(),
            instrumentation_profile: PROFILE.to_owned(),
        }),
    });
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        outcome.envelope.error
    );
    let Payload::ObserveIngest(response) = outcome.payload else {
        panic!("an ingest answers an ingest payload");
    };
    response
        .evidence
        .into_iter()
        .next()
        .expect("an ingest appends one node")
}

fn connection() -> Connection {
    Connection::new(version(), actor("agent:reader"), capability("cap_reader"))
}

fn args(handle: &EvidenceHandle, inline: bool) -> ShowArgs {
    ShowArgs {
        evidence: handle.clone(),
        inline,
    }
}

fn absent() -> EvidenceHandle {
    EvidenceHandle::new(ABSENT).expect("a well-formed evidence handle")
}

/// Run `evidence show` once, over the wire, and return what it rendered.
fn show(fixture: &mut Fixture, args: &ShowArgs, format: Format) -> continuum_cli::render::Rendered {
    let mut connection = connection();
    let mut link = fixture.link();
    evidence::show(&mut connection, &mut link, args, format)
        .expect("the call reaches a real frame and a real answer")
}

// --- the evidence -------------------------------------------------------------------------

#[test]
fn show_renders_the_node_a_real_daemon_holds_under_that_handle() {
    let mut fixture = Fixture::fresh();
    let handle = fixture.node.clone();

    // The typed answer, read directly: the source of truth every rendering below is held to.
    let outcome = {
        let mut connection = connection();
        let mut link = fixture.link();
        connection
            .evidence_get(
                &mut link,
                &continuumd::protocol::operations::evidence::EvidenceGetRequest {
                    evidence: handle.clone(),
                    inline: Optional::Absent,
                },
            )
            .expect("the call is answered")
    };
    let Outcome::Admitted(admitted) = &outcome else {
        panic!("a node this daemon holds is admitted: {outcome:?}");
    };
    assert_eq!(Depth::of(&outcome), Depth::Served);
    let Payload::EvidenceGet(response) = &admitted.payload else {
        panic!("evidence.get answers an evidence.get payload");
    };
    assert!(
        response.node.value().is_some(),
        "the handle names a node, so the node reading is set"
    );
    assert!(
        response.edge.value().is_none(),
        "…and the edge reading is null: the two are exclusive"
    );

    for format in [Format::Text, Format::Pretty] {
        let rendered = show(&mut fixture, &args(&handle, false), format);
        assert_eq!(rendered.exit_code, 0, "{format:?}:\n{}", rendered.text);
        assert!(rendered.text.contains("depth  served"), "{}", rendered.text);
        // The handle is printed in full, never truncated.
        assert!(rendered.text.contains(handle.as_str()), "{}", rendered.text);
        assert!(
            rendered.text.contains("edge_bytes  none"),
            "{}",
            rendered.text
        );
        // `none`, not `false`: the answer carried no stub, and the bare key reads the
        // machine document's `redacted` — `null` there, `none` here. bn-ybh1z flipped this
        // pin: `false` was a claim this crate made about a field the daemon never sent, and
        // it disagreed with the machine channel for the same answer.
        assert!(
            rendered.text.contains("redacted  none"),
            "{}",
            rendered.text
        );
        assert!(
            rendered.text.contains("redacted.redacted  none"),
            "{}",
            rendered.text
        );
        assert!(rendered.text.contains("omissions  0"), "{}", rendered.text);
    }

    let json = show(&mut fixture, &args(&handle, false), Format::Json);
    assert_eq!(json.exit_code, 0);
    assert!(json.text.contains("\"depth\":\"served\""), "{}", json.text);
    // The node document is embedded verbatim, so its schema-normative fields are readable
    // in machine output without a second call.
    assert!(
        json.text
            .contains(&format!("\"node_id\":\"{}\"", handle.as_str())),
        "{}",
        json.text
    );
    assert!(
        json.text
            .contains("\"schema_id\":\"https://continuum.dev/schema/evidence-graph-node.json\""),
        "{}",
        json.text
    );
    assert!(json.text.contains("\"edge\":null"), "{}", json.text);
    assert!(json.text.contains("\"redacted\":null"), "{}", json.text);
    assert!(json.text.contains("\"error\":null"), "{}", json.text);
    // `node` and `node_bytes` are both present, so a body that failed to parse would be
    // visible as `(null, N)` rather than indistinguishable from an absent one.
    assert!(json.text.contains("\"node_bytes\":"), "{}", json.text);
}

#[test]
fn a_handle_the_graph_does_not_hold_is_the_daemons_own_capability_denial() {
    let mut fixture = Fixture::fresh();
    for format in [Format::Text, Format::Pretty, Format::Json] {
        let rendered = show(&mut fixture, &args(&absent(), false), format);
        assert_eq!(
            rendered.exit_code, 1,
            "a refusal exits non-zero in {format:?}:\n{}",
            rendered.text
        );
        assert!(
            rendered
                .text
                .contains(ErrorCode::CapabilityDenied.as_wire()),
            "the typed reason is the protocol's own wire token, not a synonym, in \
             {format:?}:\n{}",
            rendered.text
        );
        // RFC 0027 X2: nothing here says "no such node". The command knows only the code,
        // and the code deliberately does not distinguish absence from denial.
        for oracle in ["not found", "not-found", "no such", "does not exist"] {
            assert!(
                !rendered.text.to_lowercase().contains(oracle),
                "an existence oracle leaked into {format:?}:\n{}",
                rendered.text
            );
        }
    }
}

#[test]
fn a_denial_reports_a_refused_depth_and_never_an_unsupported_one() {
    // The contrast that gives `depth` its content: this daemon *serves* `evidence.get`, so a
    // `no` about one handle is `refused` and not `unsupported`. A CLI that collapsed the two
    // would tell a caller to give up on a surface that works.
    let mut fixture = Fixture::fresh();
    let rendered = show(&mut fixture, &args(&absent(), false), Format::Json);
    assert!(
        rendered.text.contains("\"depth\":\"refused\""),
        "{}",
        rendered.text
    );
    assert!(
        !rendered.text.contains("\"depth\":\"unsupported\""),
        "{}",
        rendered.text
    );
    assert_eq!(Depth::of_code(ErrorCode::CapabilityDenied), Depth::Refused);
}

#[test]
fn asking_for_inline_content_is_admitted_with_a_typed_omission_naming_it() {
    // The honest degradation: this daemon has no bounded-content channel in the response
    // body, so `--inline` is *admitted* and the shortfall is a typed INV-007 omission naming
    // exactly what was not delivered — never a silent success and never a refusal of a
    // well-formed read.
    let mut fixture = Fixture::fresh();
    let handle = fixture.node.clone();

    let without = show(&mut fixture, &args(&handle, false), Format::Text);
    assert!(without.text.contains("inline  false"), "{}", without.text);
    assert!(without.text.contains("omissions  0"), "{}", without.text);

    let with = show(&mut fixture, &args(&handle, true), Format::Text);
    assert_eq!(with.exit_code, 0, "{}", with.text);
    assert!(with.text.contains("inline  true"), "{}", with.text);
    assert!(with.text.contains("omissions  1"), "{}", with.text);
    assert!(
        with.text.contains("evidence.inline_content"),
        "the omission names exactly what was not delivered:\n{}",
        with.text
    );
    assert!(
        with.text.contains(OmissionReason::Unsupported.as_wire()),
        "…with the protocol's own reason token:\n{}",
        with.text
    );

    let json = show(&mut fixture, &args(&handle, true), Format::Json);
    assert!(json.text.contains("\"inline\":true"), "{}", json.text);
    assert!(
        json.text
            .contains("\"subject\":\"evidence.inline_content\""),
        "{}",
        json.text
    );
    assert!(json.text.contains("\"depth\":\"served\""), "{}", json.text);
}

#[test]
fn a_redacted_reference_renders_the_typed_stub_beside_the_record() {
    let mut fixture = Fixture::fresh();
    fixture.redact();
    let handle = fixture.node.clone();

    let text = show(&mut fixture, &args(&handle, false), Format::Text);
    assert_eq!(
        text.exit_code, 0,
        "a redaction is not a failure:\n{}",
        text.text
    );
    // 1. The typed stub, field by field — "withheld" distinguishable from "absent"
    //    structurally, without inference (RFC 0026).
    assert!(
        text.text.contains("redacted.redacted  true"),
        "{}",
        text.text
    );
    assert!(
        text.text.contains(&format!(
            "redacted.reason  {}",
            RedactionReason::Purged.as_wire()
        )),
        "{}",
        text.text
    );
    assert!(
        text.text.contains("redacted.original_class  evidence"),
        "{}",
        text.text
    );
    // 2. The INV-007 manifest names it too — a different obligation, not a duplicate.
    assert!(text.text.contains("omissions  1"), "{}", text.text);
    assert!(
        text.text.contains(OmissionReason::Redaction.as_wire()),
        "{}",
        text.text
    );
    // 3. The record itself is still returned: an error carries no record, and this is not
    //    an error.
    assert!(
        !text.text.contains("node_bytes  none"),
        "the node record travels beside the redaction:\n{}",
        text.text
    );

    let json = show(&mut fixture, &args(&handle, false), Format::Json);
    assert!(
        json.text.contains("\"redacted\":{\"commitment\":"),
        "the stub is an object, not a boolean:\n{}",
        json.text
    );
    assert!(
        json.text.contains(&format!(
            "\"reason\":\"{}\"",
            RedactionReason::Purged.as_wire()
        )),
        "{}",
        json.text
    );
    assert!(json.text.contains("\"depth\":\"served\""), "{}", json.text);
}

/// The whole `evidence show --json` document for a denial, byte for byte.
///
/// Pinned in full because the acceptance criterion is the *shape* of the machine answer:
/// a `contains` assertion cannot see a key that was added, a key that was dropped, or an
/// ordering that stopped being canonical. The denial arm is the one that can be pinned
/// exactly — the admitted arm embeds a node record whose identity is a content hash.
#[test]
fn the_denial_json_document_is_pinned_byte_for_byte() {
    let mut fixture = Fixture::fresh();
    let rendered = show(&mut fixture, &args(&absent(), false), Format::Json);
    assert_eq!(
        rendered.text,
        concat!(
            r#"{"advice":[],"artifacts":[],"cost":null,"depth":"refused","#,
            r#""edge":null,"edge_bytes":null,"#,
            r#""error":{"code":"CapabilityDenied","continuation":null,"#,
            r#""detail":"the presented capability does not admit this operation","#,
            r#""non_resumable_reason":null,"recovery":[],"retryable":false},"#,
            r#""evidence":"ev_neverappended01","inline":false,"next_operations":[],"#,
            r#""node":null,"node_bytes":null,"#,
            r#""omissions":[],"operation":"evidence.get","redacted":null,"#,
            r#""request_id":"req_cli000001"}"#,
            "\n"
        ),
        "the machine envelope is canonical JSON with a fixed key set"
    );
    assert_eq!(rendered.exit_code, 1);
}
