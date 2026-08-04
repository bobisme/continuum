//! Evidence for `continuum snapshot create|fork|seal` (bn-3rqvm, PR-13 first command group)
//! — the PR-3 workspace family, driven through `continuum_cli::snapshot`'s own command
//! functions over a real `LocalPair` boundary against a real, provisioned `Daemon`.
//!
//! Every assertion here is about what a daemon actually said. `continuumd::daemon::workspace`
//! serves all three operations, so all three green paths are live; nothing in this file
//! renders a hand-built answer.
//!
//! # Clause → test
//!
//! - **the CLI is a client of the same daemon surface agents use** →
//!   [`create_seal_and_fork_run_end_to_end_over_real_frames`]: one snapshot is created,
//!   sealed, and forked entirely through the CLI's command functions, and the second command
//!   consumes the handle the first *printed*. There is no second code path: the handles are
//!   the daemon's own and the CLI holds none between calls.
//! - **the registry mapping** → [`each_verb_names_the_registry_operation_it_drives`].
//! - **INV-001, visible** → the same test asserts a fork's answer carries the preserved
//!   `intent`, which is the whole content of "the fork preserves the intent binding by
//!   identity" at this operation.
//! - **refusal path, X2 preserved** →
//!   [`a_fork_of_a_base_this_daemon_does_not_hold_is_a_typed_denial_and_not_a_not_found`].
//! - **boundary: a served family that does not serve one argument** →
//!   [`a_patched_fork_reports_an_unsupported_depth_from_a_family_that_is_served`] — the same
//!   `Depth` machinery bn-1g7e4 built, here distinguishing "this deployment cannot apply
//!   patches" from "this handle was refused", inside a namespace that *is* served.
//! - **the machine contract, pinned** →
//!   [`the_denial_json_document_is_pinned_byte_for_byte`] and
//!   [`the_seal_text_rendering_is_pinned_line_for_line`].
//! - **INV-002 explicit handles** →
//!   [`no_snapshot_verb_reaches_the_wire_without_its_handle`].
//! - **adversarial input, linear** → [`a_large_overlay_is_still_a_typed_answer`].
//!
//! # The fixture
//!
//! `tests/task_lifecycle.rs`'s deployment, trimmed to the two families this group needs
//! (`WorkspaceFamily` for the operations, `IntentFamily` because `workspace.create` refuses a
//! snapshot whose governing intent is not an *accepted* registry record). Duplicated locally
//! rather than imported, for the reason `continuum-mcp/tests/typed_surface.rs`'s identical
//! comment gives: a `tests/*.rs` file is its own crate.

use continuum_cli::format::Format;
use continuum_cli::render::{Depth, Rendered};
use continuum_cli::snapshot::{self, CreateArgs, ForkArgs, SealArgs};
use continuum_cli::wire::{Connection, LinkError, LocalLink, Transport};
use continuum_cli::{cli, render};
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::publication::ContentIdentifier;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::daemon::family::Arguments;
use continuumd::daemon::identity::{Blake3Identity, intent_to_wire};
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationRequest};
use continuumd::protocol::envelope::{EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, EpochIdentity, IntentHandle, Opaque, OperationName,
    ProtocolVersion, RequestId, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{
    FileComponent, FileOverlay, SnapshotComponents, SnapshotEpochs,
};
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{AuthorityLevel, Encoding, ErrorCode, ResultStatus};
use continuumd::transport::{LocalPair, Server};

const MODULE: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");
const CONTRACT: &str = include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");
const MODULE_PATH: &str = "DieHard.ctm";
const NOW: &str = "2026-08-01T00:00:00.000Z";

/// A well-formed `ws_` handle no deployment in this file ever creates.
const ABSENT: &str = "ws_neverheld0001";

/// Every operation this bone's `snapshot` commands drive, read from the crate's own public
/// constants so a command that changed which frame it sends fails here.
const DRIVEN: [&str; 3] = [
    snapshot::CREATE_OPERATION,
    snapshot::FORK_OPERATION,
    snapshot::SEAL_OPERATION,
];

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

fn grant(handle: &str, who: &str, level: AuthorityLevel) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: capability(handle),
        actor: actor(who),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: 3,
        profile: Optional::Absent,
    }
}

fn root_grant() -> CapabilityDescriptor {
    CapabilityDescriptor {
        delegation_depth: 4,
        profile: Optional::Present(CapabilityProfile {
            privileged_operations: vec![OperationName::new("intent.accept").expect("a name")],
            denied_operations: Vec::new(),
            data_grants: Vec::new(),
            cross_principal_sharing: true,
        }),
        ..grant("cap_root", "service:continuumd", AuthorityLevel::Promote)
    }
}

/// One provisioned deployment behind a byte boundary.
struct Fixture {
    server: Server,
    pair: LocalPair,
    components: SnapshotComponents,
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
            .epochs(epochs())
            .now(Timestamp::new(NOW).expect("a timestamp"))
            .capability(root_grant(), None)
            .capability(
                grant("cap_builder", "agent:builder", AuthorityLevel::Propose),
                root,
            )
            .family(WorkspaceFamily)
            .family(IntentFamily)
            .build();

        let intent = accept_intent(&mut daemon);
        let components = stage(&mut daemon, intent);

        Self {
            server: Server::new(daemon, negotiated),
            pair: LocalPair::new(),
            components,
        }
    }

    fn link(&mut self) -> LocalLink<'_> {
        LocalLink::new(&mut self.server, &mut self.pair)
    }
}

fn accept_intent(daemon: &mut Daemon) -> IntentHandle {
    let contract =
        IntentContract::decode(CONTRACT.trim_end().as_bytes()).expect("the fixture decodes");
    let stored = ContentIdentifier::identify(
        &Blake3Identity,
        ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    let intent = intent_to_wire(&stored).expect("an `in_` handle");
    daemon.state_mut().put_intent(
        intent.clone(),
        IntentRecord {
            contract,
            status: RegistryStatus::Proposed,
            supersedes: None,
            superseded_by: None,
            acceptance: None,
        },
    );

    let mut fields = std::collections::BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "service:continuumd"),
        ("capability", "revise-intent"),
        ("signature", "sig-die-hard-v1"),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", NOW),
    ] {
        fields.insert(key.to_owned(), Json::String(value.to_owned()));
    }
    let outcome = daemon.dispatch(&OperationRequest {
        envelope: RequestEnvelope {
            protocol_version: version(),
            request_id: RequestId::new("req_accept").expect("a well-formed request id"),
            idempotency_key: Optional::Present("idem-accept".to_owned()),
            actor: actor("service:continuumd"),
            capability: capability("cap_root"),
            operation: OperationName::new("intent.accept").expect("a registry name"),
            snapshot: Nullable::Null,
            intent: Nullable::Null,
            arguments: Opaque::from_bytes(Vec::new()),
            budget: Optional::Absent,
            output_policy: Optional::Absent,
            trace: Optional::Absent,
            page: Optional::Absent,
        },
        arguments: Arguments::IntentAccept(IntentAcceptRequest {
            proposal: intent.clone(),
            acceptance: Opaque::from_bytes(Json::Object(fields).to_canonical_bytes()),
            bundle: Optional::Absent,
        }),
    });
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        outcome.envelope.error
    );
    intent
}

fn stage(daemon: &mut Daemon, intent: IntentHandle) -> SnapshotComponents {
    let mut files: Vec<Commitment> = Vec::new();
    let mut placements = Vec::new();
    for (path, content) in [(MODULE_PATH, MODULE), ("README.md", "# TV-009\n")] {
        let commitment = daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new(path).expect("a workspace path"),
                content.as_bytes().to_vec(),
            )
            .expect("staging names its content");
        placements.push(FileComponent {
            path: path.to_owned(),
            commitment: commitment.clone(),
        });
        files.push(commitment);
    }
    SnapshotComponents {
        files,
        cml_modules: Vec::new(),
        rust_extraction: Vec::new(),
        domain_packs: Vec::new(),
        dependencies: Vec::new(),
        epochs: SnapshotEpochs {
            semantic: epoch("semantic-1"),
            proof: epoch("proof-1"),
            toolchain: Optional::Absent,
        },
        intent,
        correspondence: Vec::new(),
        proof_environment: Vec::new(),
        configuration: Vec::new(),
        file_components: Optional::Present(placements),
    }
}

fn connection() -> Connection {
    Connection::new(version(), actor("agent:builder"), capability("cap_builder"))
}

fn create_args(fixture: &Fixture, seal: bool) -> CreateArgs {
    CreateArgs {
        components: fixture.components.clone(),
        overlay: Vec::new(),
        seal,
    }
}

fn absent() -> WorkspaceHandle {
    WorkspaceHandle::new(ABSENT).expect("a well-formed snapshot handle")
}

/// Run one command over the wire and return what it rendered, on a fresh connection each
/// time — the CLI holds nothing between invocations, and neither does this helper.
fn run<F>(fixture: &mut Fixture, call: F) -> Rendered
where
    F: FnOnce(
        &mut Connection,
        &mut LocalLink<'_>,
    ) -> Result<Rendered, continuum_cli::error::CliError>,
{
    let mut connection = connection();
    let mut link = fixture.link();
    call(&mut connection, &mut link).expect("the call reaches a real frame and a real answer")
}

/// The `snapshot` line value a rendering printed, so the next command can consume the handle
/// the previous one *printed* rather than one the test kept behind the CLI's back.
fn line(rendered: &Rendered, key: &str) -> String {
    rendered
        .text
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key}  ")))
        .unwrap_or_else(|| panic!("no {key:?} line in:\n{}", rendered.text))
        .to_owned()
}

// --- the evidence -------------------------------------------------------------------------

#[test]
fn each_verb_names_the_registry_operation_it_drives() {
    for operation in DRIVEN {
        assert!(
            registry::operation(operation).is_some(),
            "{operation} is a registered operation"
        );
    }
    assert_eq!(
        DRIVEN,
        ["workspace.create", "workspace.fork", "workspace.seal"],
        "the mapping this group documents is the mapping it sends"
    );
}

#[test]
fn create_seal_and_fork_run_end_to_end_over_real_frames() {
    let mut fixture = Fixture::fresh();

    // 1. create, unsealed — the caller did not ask for a seal, so the daemon's own default
    //    stands and the answer says which it was.
    let args = create_args(&fixture, false);
    let created = run(&mut fixture, |connection, link| {
        snapshot::create(connection, link, &args, Format::Text)
    });
    assert_eq!(created.exit_code, 0, "{}", created.text);
    assert!(created.text.contains("depth  served"), "{}", created.text);
    assert!(created.text.contains("sealed  false"), "{}", created.text);
    assert!(
        created.text.contains("diagnostics  0"),
        "an empty diagnostic list states its own emptiness:\n{}",
        created.text
    );
    let snapshot = line(&created, "snapshot");
    assert!(snapshot.starts_with("ws_"), "{snapshot}");

    // 2. seal, by the handle the previous command printed. This is the INV-002 property as a
    //    test: the second invocation knows the snapshot only because the first *printed* it.
    let seal_args = SealArgs {
        snapshot: WorkspaceHandle::new(&snapshot).expect("the printed handle is well formed"),
    };
    let sealed = run(&mut fixture, |connection, link| {
        snapshot::seal(connection, link, &seal_args, Format::Text)
    });
    assert_eq!(sealed.exit_code, 0, "{}", sealed.text);
    assert_eq!(line(&sealed, "sealed"), snapshot);
    assert!(
        !line(&sealed, "root_digest").is_empty(),
        "a sealed snapshot is named by its root digest:\n{}",
        sealed.text
    );

    // 3. fork, with an overlay — and the preserved intent binding beside the new handle
    //    (INV-001).
    let fork_args = ForkArgs {
        base: seal_args.snapshot.clone(),
        overlay: vec![FileOverlay {
            path: "elsewhere.txt".to_owned(),
            content: b"elsewhere".to_vec(),
        }],
        patches: Vec::new(),
    };
    let forked = run(&mut fixture, |connection, link| {
        snapshot::fork(connection, link, &fork_args, Format::Text)
    });
    assert_eq!(forked.exit_code, 0, "{}", forked.text);
    let child = line(&forked, "snapshot");
    assert!(child.starts_with("ws_"), "{child}");
    assert_ne!(child, snapshot, "a fork names a different snapshot");
    assert_eq!(
        line(&forked, "intent"),
        fixture.components.intent.as_str(),
        "the fork preserves the intent binding by identity (INV-001)"
    );
    assert_eq!(
        line(&forked, "pre_diff"),
        "none",
        "an absent optional field is named, never left to silence"
    );

    // The machine channel carries the same facts, structurally.
    let json = run(&mut fixture, |connection, link| {
        snapshot::seal(connection, link, &seal_args, Format::Json)
    });
    assert!(json.text.contains("\"depth\":\"served\""), "{}", json.text);
    assert!(
        json.text.contains(&format!("\"sealed\":\"{snapshot}\"")),
        "{}",
        json.text
    );
    assert!(json.text.contains("\"error\":null"), "{}", json.text);
    assert!(json.text.contains("\"omissions\":["), "{}", json.text);
}

#[test]
fn creating_with_the_seal_flag_answers_sealed_true_in_every_format() {
    // The boundary between the two ways to seal: `--seal` on create, and the `seal` verb.
    // Both are real, and the create answer says which happened rather than leaving a caller
    // to run a second command to find out.
    let mut fixture = Fixture::fresh();
    let args = create_args(&fixture, true);
    for format in [Format::Text, Format::Pretty] {
        let mut connection = connection();
        let mut link = fixture.link();
        let rendered = snapshot::create(&mut connection, &mut link, &args, format)
            .expect("the call is answered");
        assert_eq!(rendered.exit_code, 0, "{format:?}:\n{}", rendered.text);
        assert!(
            rendered.text.contains("sealed  true"),
            "{format:?}:\n{}",
            rendered.text
        );
        assert!(rendered.text.contains("seal  true"), "{}", rendered.text);
    }
    let mut connection = connection();
    let mut link = fixture.link();
    let json = snapshot::create(&mut connection, &mut link, &args, Format::Json)
        .expect("the call is answered");
    assert!(json.text.contains("\"sealed\":true"), "{}", json.text);
    assert!(json.text.contains("\"diagnostics\":[]"), "{}", json.text);
}

#[test]
fn a_fork_of_a_base_this_daemon_does_not_hold_is_a_typed_denial_and_not_a_not_found() {
    // RFC 0027 X2: a read of a snapshot the caller is not authorized for is
    // `CapabilityDenied` *whether or not it exists*, because a distinct not-found is an
    // existence oracle. This command renders the code the daemon typed and adds no gloss —
    // there is no branch here that could, because the only thing it knows is the code.
    let mut fixture = Fixture::fresh();
    let args = ForkArgs {
        base: absent(),
        overlay: Vec::new(),
        patches: Vec::new(),
    };
    for format in [Format::Text, Format::Pretty, Format::Json] {
        let mut connection = connection();
        let mut link = fixture.link();
        let rendered = snapshot::fork(&mut connection, &mut link, &args, format)
            .expect("the call is answered");
        assert_eq!(
            rendered.exit_code, 1,
            "a refusal exits non-zero in {format:?}:\n{}",
            rendered.text
        );
        assert!(
            rendered
                .text
                .contains(ErrorCode::CapabilityDenied.as_wire()),
            "the typed reason is the protocol's own wire token in {format:?}:\n{}",
            rendered.text
        );
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
fn a_patched_fork_reports_an_unsupported_depth_from_a_family_that_is_served() {
    // The contrast that gives `depth` its content inside a *served* namespace: this daemon
    // serves `workspace.fork`, and refuses one argument of it. A CLI that collapsed the two
    // `no`s would tell a caller either to give up on a surface that works, or to keep
    // retrying an argument no deployment here can honour.
    let mut fixture = Fixture::fresh();
    let args = ForkArgs {
        base: absent(),
        overlay: Vec::new(),
        patches: vec![Commitment::new("blake3-256:patchplaceholder")],
    };
    let mut patched_connection = connection();
    let mut link = fixture.link();
    let rendered = snapshot::fork(&mut patched_connection, &mut link, &args, Format::Json)
        .expect("the call is answered");
    assert!(
        rendered.text.contains("\"depth\":\"unsupported\""),
        "{}",
        rendered.text
    );
    assert!(
        rendered
            .text
            .contains(ErrorCode::UnsupportedSemanticFeature.as_wire()),
        "{}",
        rendered.text
    );
    assert!(
        rendered.text.contains("\"patches\":1"),
        "the request is echoed so the answer says what was unsupported:\n{}",
        rendered.text
    );
    assert_eq!(rendered.exit_code, 1);
    assert_eq!(
        Depth::of_code(ErrorCode::UnsupportedSemanticFeature),
        Depth::Unsupported
    );
    // …and the *same* command with the same base and no patches is a plain refusal, so the
    // difference is the argument and not the handle.
    let plain = ForkArgs {
        patches: Vec::new(),
        ..args
    };
    let mut plain_connection = connection();
    let mut link = fixture.link();
    let rendered = snapshot::fork(&mut plain_connection, &mut link, &plain, Format::Json)
        .expect("the call is answered");
    assert!(
        rendered.text.contains("\"depth\":\"refused\""),
        "{}",
        rendered.text
    );
}

/// The whole `snapshot fork --json` document for a denial, byte for byte.
///
/// The denial arm is the one that can be pinned exactly: every admitted arm names a snapshot
/// whose handle is a content identity of the fixture's own bytes.
#[test]
fn the_denial_json_document_is_pinned_byte_for_byte() {
    let mut fixture = Fixture::fresh();
    let args = ForkArgs {
        base: absent(),
        overlay: Vec::new(),
        patches: Vec::new(),
    };
    let mut connection = connection();
    let mut link = fixture.link();
    let rendered = snapshot::fork(&mut connection, &mut link, &args, Format::Json)
        .expect("the call is answered");
    assert_eq!(
        rendered.text,
        concat!(
            r#"{"advice":[],"artifacts":[],"base":"ws_neverheld0001","cost":null,"#,
            r#""depth":"refused","diagnostics":null,"#,
            r#""error":{"code":"CapabilityDenied","continuation":null,"#,
            r#""detail":"the presented capability does not admit this operation","#,
            r#""non_resumable_reason":null,"recovery":[],"retryable":false},"#,
            r#""intent":null,"next_operations":[],"omissions":[],"#,
            r#""operation":"workspace.fork","overlay":0,"#,
            r#""patches":0,"pre_diff":null,"request_id":"req_cli000001","snapshot":null}"#,
            "\n"
        ),
        "the machine envelope is canonical JSON with a fixed key set"
    );
    assert_eq!(rendered.exit_code, 1);
}

/// The whole `snapshot seal` text rendering for a denial, line for line — the arm whose
/// every value is either the caller's own or the daemon's fixed sentence.
#[test]
fn the_seal_text_rendering_is_pinned_line_for_line() {
    let mut fixture = Fixture::fresh();
    let args = SealArgs { snapshot: absent() };
    let mut text_connection = connection();
    let mut link = fixture.link();
    let rendered = snapshot::seal(&mut text_connection, &mut link, &args, Format::Text)
        .expect("the call is answered");
    assert_eq!(
        rendered.text,
        concat!(
            "operation  workspace.seal\n",
            "depth  refused\n",
            "snapshot  ws_neverheld0001\n",
            "error.code  CapabilityDenied\n",
            "error.detail  the presented capability does not admit this operation\n",
            "error.retryable  false\n",
            "error.recovery  0\n",
            "error.continuation  none\n",
            "error.non_resumable_reason  none\n",
            "omissions  0\n",
            "next_operations  0\n",
        )
    );

    // `pretty` differs by exactly one line — the command's own name — and by nothing else.
    let mut pretty_connection = connection();
    let mut link = fixture.link();
    let pretty = snapshot::seal(&mut pretty_connection, &mut link, &args, Format::Pretty)
        .expect("the call is answered");
    assert_eq!(
        pretty.text,
        format!("command  snapshot seal\n{}", rendered.text)
    );
}

/// The idempotency channel, and the collision behaviour bn-jmx97 ratified for a generated
/// key (this pin was a defect record until that bone declined content-derivation; see
/// `wire::Connection::with_idempotency_key` for the ratification's grounds).
///
/// `rule idempotency.replay` gives one key one meaning, and which two invocations are "the
/// same request" is a statement only a caller can make. This test is both halves of that:
/// two *different* requests presenting the same generated key earn the daemon's typed
/// `IdempotencyKeyReused` — a rendered refusal naming the channel, never a silent wrong
/// answer — and the same two under distinct `--idempotency-key` values are both admitted.
#[test]
fn two_different_creates_need_two_idempotency_keys_and_say_so_when_they_share_one() {
    let mut fixture = Fixture::fresh();
    let unsealed = create_args(&fixture, false);
    let sealed = create_args(&fixture, true);

    // Same generated key (fresh connection, first call, same operation), different canonical
    // request: the daemon says exactly which of the two it is.
    let first = run(&mut fixture, |connection, link| {
        snapshot::create(connection, link, &unsealed, Format::Text)
    });
    assert_eq!(first.exit_code, 0, "{}", first.text);
    let collided = run(&mut fixture, |connection, link| {
        snapshot::create(connection, link, &sealed, Format::Text)
    });
    assert_eq!(collided.exit_code, 1, "{}", collided.text);
    assert!(
        collided
            .text
            .contains(ErrorCode::IdempotencyKeyReused.as_wire()),
        "the typed reason names the key, not the request:\n{}",
        collided.text
    );

    // The caller's own keys: two statements that these are two requests, and both are
    // admitted.
    let mut fixture = Fixture::fresh();
    for (key, args) in [("mine-1", &unsealed), ("mine-2", &sealed)] {
        let mut connection = connection().with_idempotency_key(Some(key.to_owned()));
        let mut link = fixture.link();
        let rendered = snapshot::create(&mut connection, &mut link, args, Format::Text)
            .expect("the call is answered");
        assert_eq!(rendered.exit_code, 0, "{key}:\n{}", rendered.text);
    }
}

// --- adversarial input ----------------------------------------------------------------------

/// A 64 KiB overlay — one `repeat`, so the fixture is linear in the size it names.
#[test]
fn a_large_overlay_is_still_a_typed_answer() {
    const OVERLAY_BYTES: usize = 64 * 1024;
    let content = "o".repeat(OVERLAY_BYTES);
    assert_eq!(content.len(), OVERLAY_BYTES);

    let mut fixture = Fixture::fresh();
    let args = ForkArgs {
        base: absent(),
        overlay: vec![FileOverlay {
            path: "big.txt".to_owned(),
            content: content.into_bytes(),
        }],
        patches: Vec::new(),
    };
    let mut connection = connection();
    let mut link = fixture.link();
    let rendered = snapshot::fork(&mut connection, &mut link, &args, Format::Json)
        .expect("a large argument is encoded, sent, and answered like any other");
    assert_eq!(rendered.exit_code, 1);
    assert!(rendered.text.contains("\"overlay\":1"), "{}", rendered.text);
    assert!(
        rendered
            .text
            .contains(ErrorCode::CapabilityDenied.as_wire()),
        "{}",
        rendered.text
    );
}

// --- INV-002: no verb reaches the wire without its handle -------------------------------------

/// A transport that fails every exchange, so a test asserting a usage error also asserts that
/// no frame was written: reaching the wire would be exit `2`, not exit `1`.
struct Deaf;

impl Transport for Deaf {
    fn exchange(&mut self, _frame: &[u8]) -> Result<Vec<u8>, LinkError> {
        Err(LinkError::NoAnswer)
    }
}

fn args(words: &[&str]) -> Vec<String> {
    let mut line: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
    line.extend(
        ["--actor", "agent:builder", "--capability", "cap_builder"]
            .iter()
            .map(|word| (*word).to_owned()),
    );
    line
}

#[test]
fn no_snapshot_verb_reaches_the_wire_without_its_handle() {
    let lines: [&[&str]; 4] = [
        &["snapshot", "fork"],
        &["snapshot", "seal"],
        // `create` has no positional handle; its declaration is `--components`, and an
        // absent or malformed one is caught before an envelope exists.
        &["snapshot", "create"],
        &["snapshot", "create", "--components", "not a document"],
    ];
    for line in lines {
        let mut transport = Deaf;
        let error = cli::run(&args(line), &mut transport, false)
            .expect_err("a command with no declaration never reaches the wire");
        assert_eq!(
            error.exit_code(),
            1,
            "{line:?} is a usage error, not a wire failure"
        );
    }
}

#[test]
fn a_handle_of_the_wrong_class_is_a_usage_error_and_never_reaches_the_wire() {
    for line in [
        ["snapshot", "seal", "task_abc123"],
        ["snapshot", "fork", "ev_abc123"],
    ] {
        let mut transport = Deaf;
        let error = cli::run(&args(&line), &mut transport, false)
            .expect_err("a handle of the wrong class never reaches the wire");
        assert_eq!(error.exit_code(), 1, "{line:?}");
    }
}

#[test]
fn an_overlay_without_a_path_separator_is_a_usage_error() {
    // `--overlay` is `<path>=<content>`; a token with no `=` names no path, and inventing one
    // would be this crate choosing where a caller's bytes land.
    let mut transport = Deaf;
    let error = cli::run(
        &args(&["snapshot", "fork", ABSENT, "--overlay", "no-separator"]),
        &mut transport,
        false,
    )
    .expect_err("a malformed overlay never reaches the wire");
    assert_eq!(error.exit_code(), 1);
}

#[test]
fn the_omission_manifest_key_is_present_on_every_arm() {
    let mut fixture = Fixture::fresh();
    let create = create_args(&fixture, false);
    let admitted = run(&mut fixture, |connection, link| {
        snapshot::create(connection, link, &create, Format::Json)
    });
    let refused = run(&mut fixture, |connection, link| {
        snapshot::seal(
            connection,
            link,
            &SealArgs { snapshot: absent() },
            Format::Json,
        )
    });
    for rendered in [&admitted, &refused] {
        assert!(
            rendered.text.contains("\"omissions\":["),
            "the INV-007 manifest key is present on every arm:\n{}",
            rendered.text
        );
        assert!(rendered.text.contains("\"advice\":[]"), "{}", rendered.text);
    }
    assert_eq!(render::omissions_json(&[]).to_canonical_bytes(), b"[]");
}
