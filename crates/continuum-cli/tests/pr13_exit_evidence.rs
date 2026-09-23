//! Dedicated exit evidence for `PR-13-EXIT` (`notes/plan/notes/PLAN_REQUIREMENTS.json`,
//! id `PR-13-EXIT`; `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 13's Exit line).
//!
//! > **Exit:** golden tests pin JSON, exit codes, and concise terminal output.
//! >
//! > — `notes/plan/notes/START_HERE_IMPLEMENTATION.md`, PR 13
//!
//! This file asserts that sentence end to end, the way `pr3_exit_evidence.rs` did for PR 3's
//! exit (bn-3tkw) and `pr6_exit_evidence.rs` did for PR 6's (bn-1n6r): one named witness that
//! walks the whole sentence in one place, independent of the four implementation bones it
//! grades (bn-3rqvm, bn-1g7e4, bn-3tz60, bn-ybh1z), touching none of their tests and none of
//! `src/`. Where those bones' own suites assert *properties* of renderings, this file retains
//! the renderings themselves: committed golden artifacts, compared byte for byte.
//!
//! # Evidence map
//!
//! The exit sentence names three subjects; each is carried by named tests over one shared
//! scenario script ([`script`]), which drives **every stable command of plan §13.4's list**
//! through [`continuum_cli::cli::run`] — the outermost seam: real argv, the real `Scan`
//! parser, a real encoded frame over a real `LocalPair` boundary, a real provisioned
//! `Daemon`, and the real rendered answer — exactly as `bin/continuum.rs` would drive it if
//! a process transport existed (see the crate root doc, "What 'over the wire' means here").
//!
//! - **"golden tests pin JSON"** → [`every_pinned_artifact_matches_its_committed_golden_bytes`].
//!   Twenty scenarios; each `--format json` document retained under a stable artifact ID in
//!   `tests/golden/` (naming documented at [`golden_path`]) and compared byte-exactly.
//! - **"… exit codes"** → [`the_exit_code_matrix_reaches_all_five_classes_on_real_scenarios`].
//!   All five classes of `continuum_cli::contract::Exit`, each produced by a scenario that
//!   *genuinely* earns it: a success (`0`); a real typed refusal (`1` — the daemon's own
//!   `StaleSnapshot` after the lineage really moved, its own `IdempotencyKeyReused` after a
//!   real generated-key collision, and its own `UnsupportedSemanticFeature` over a really
//!   unserved surface); a real fault (`2` — `NullTransport`, the transport `bin/continuum.rs`
//!   actually runs today); the engine's own refutation of the Die Hard claims (`3`); and the
//!   engine's own typed `ResourceExhausted` inconclusiveness under a real budget (`4`).
//!   Nothing is mocked: every verdict travelled a frame.
//! - **"… and concise terminal output"** → the same golden comparison pins every scenario's
//!   `--format text` body byte for byte, and
//!   [`the_terminal_body_of_every_pinned_scenario_is_a_pure_projection_of_its_json`] holds
//!   `contract::divergence` empty between the two channels for every pinned scenario — the
//!   text never invents or drops content relative to the JSON (INV-003).
//!   [`pretty_is_text_plus_the_command_label_for_every_pinned_scenario`] pins the third
//!   format as exactly one label line more.
//!
//! # Determinism (INV-005) and independence (INV-004)
//!
//! - [`two_independent_provisionings_render_byte_identical_artifacts`] runs the whole script
//!   twice, from two separately built daemons, and compares every artifact byte for byte.
//!   Deterministic by construction, not by normalization: **no normalization of any kind is
//!   applied to any pinned byte.** The daemon is built with a pinned `now`, handles are
//!   content-addressed (`Blake3Identity`), request identifiers are per-invocation counters,
//!   the daemon reads no clock (`cost.wall_ms` is absent because nothing measures it), and
//!   canonical JSON fixes member order. A timestamp, filesystem path, or map-order artifact
//!   in any golden would be a finding against the output contract; none was found.
//! - The comparison can fail, and the file proves it on its own artifacts:
//!   [`a_single_byte_perturbation_of_any_golden_artifact_is_detected`] flips one byte of
//!   every committed golden and asserts the same comparison the pinning test uses rejects
//!   it, and [`a_deliberately_altered_rendering_fails_the_projection_check`] mutates a real
//!   scenario's terminal body — a changed value, an invented key, a dropped line — and
//!   asserts `contract::divergence` names the drift. Silencing the contract check is
//!   therefore visible: the check distinguishes every pinned scenario from every other
//!   ([`the_projection_check_distinguishes_every_pinned_scenario_from_every_other`]).
//! - [`the_evidence_summary_agrees_with_what_this_file_observed`] holds the retained
//!   machine-readable summary (`tests/evidence/pr13-exit.json`) to what actually ran, so
//!   the summary cannot drift from the suite it summarizes.
//!
//! # Ratified behaviour, pinned as a regression guard (bn-jmx97)
//!
//! The CLI's *generated* idempotency key is a function of the operation name and a
//! per-process call counter, **not** of the request body
//! (`continuum_cli::wire::Connection::envelope`). This suite originally retained that as a
//! content-collision defect record; bn-jmx97 then examined content-derivation and
//! **ratified the counter shape** — no content digest is honestly reachable through the
//! crate's single production edge, and the daemon's own content-addressed handles already
//! answer identical-content resubmission idempotently — with `--idempotency-key` as the
//! documented cross-invocation channel (the full grounds live on
//! `Connection::with_idempotency_key`). Two invocations of the same mutation with
//! different arguments present the same generated key, and the daemon's
//! `rule idempotency.replay` refuses the second with `IdempotencyKeyReused` — a loud,
//! typed refusal naming the flag. Scenario `06-check-start--generated-key-collision` pins
//! that ratified refusal mechanically, as a regression guard: the golden fails visibly if
//! the derivation ever changes without revisiting bn-jmx97's ratification. Every other
//! mutation this script repeats with a different body passes `--idempotency-key`
//! explicitly, which is the documented channel the same module names.
//!
//! # House rules, inherited from `pr3_exit_evidence.rs`
//!
//! - **`src/` is not touched**, and no existing test is touched.
//! - **The fixture is duplicated locally** rather than imported — a `tests/*.rs` file is its
//!   own crate (`continuum-mcp/tests/typed_surface.rs`'s reason, restated by every sibling
//!   file here). It is the union of `tests/check_verdict.rs`'s Die Hard deployment,
//!   `tests/evidence_show.rs`'s observe/evidence pair, and `tests/context_expand.rs`'s
//!   registered Context Pack, so one daemon serves every command the script drives.
//! - **Golden regeneration is explicit and cannot pass.** Setting `PR13_EXIT_BLESS=1`
//!   rewrites `tests/golden/` from the live run and then panics, so a blessing run is
//!   always red and the diff is always reviewed as a contract change (INV-004: the suite
//!   cannot silently certify its own new bytes).

use std::collections::BTreeMap;
use std::path::PathBuf;

use continuum_cli::cli;
use continuum_cli::contract::{self, Exit};
use continuum_cli::error::CliError;
use continuum_cli::render::Rendered;
use continuum_cli::wire::{LocalLink, NullTransport};
use continuum_context::expansion::{
    ExpansionPayload, ExpansionQuery, ExpansionRelation as PackRelation,
};
use continuum_context::omission::{OmissionReason as PackReason, OmissionRecord};
use continuum_context::selection::SelectionKind;
use continuum_context::source::{SourceRef, SourceSpan};
use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json as IntentJson;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_value::value::Name;
use continuum_workspace::artifact_path::ArtifactClass;
use continuum_workspace::publication::ContentIdentifier;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::codec;
use continuumd::codec::json::Json;
use continuumd::daemon::context::{ContextFamily, ContextPackRecord};
use continuumd::daemon::evidence::EvidenceFamily;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::{Blake3Identity, intent_to_wire};
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationRequest};
use continuumd::protocol::envelope::{EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, VersionRange, negotiate,
};
use continuumd::protocol::operations::intent::IntentAcceptRequest;
use continuumd::protocol::operations::observe::ObserveIngestRequest;
use continuumd::protocol::registry::ENCODINGS;
use continuumd::protocol::scalar::{
    ActorId, CapabilityHandle, Commitment, ContextHandle, EpochIdentity, EvidenceHandle,
    IntentHandle, Opaque, OperationName, ProtocolVersion, RequestId, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{FileComponent, SnapshotComponents, SnapshotEpochs};
use continuumd::protocol::spec::{Nullable, Optional, ProtocolEnum};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, DataGrant, Encoding, ExpansionRelation, Portfolio, ResultStatus, TargetKind,
};
use continuumd::transport::{LocalPair, Server};

const MODULE: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");
const CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");
const CONTRACT: &str = include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");
const MODULE_PATH: &str = "DieHard.ctm";
const NOW: &str = "2026-08-01T00:00:00.000Z";

/// Below TV-009's frozen sixteen reachable states: forces a park with a continuation.
const PARK_BUDGET: u64 = 4;
/// Above it: closes the campaign outright.
const CLOSING_BUDGET: u64 = 64;

/// A production trace, as a captured bundle would arrive (`tests/evidence_show.rs`).
const TRACE: &str = "{\"events\":[{\"at\":0,\"op\":\"fill\"},{\"at\":1,\"op\":\"pour\"}]}\n";
/// The instrumentation profile the trace was captured under (plan §18.4).
const PROFILE: &str = "otel-1.0/sampled";

/// The registered Context Pack's handle (`tests/context_expand.rs`).
const PARENT: &str = "ctx_abc123def456";

/// A conforming parent pack advertising the one anchor this deployment holds a group for
/// (`tests/context_expand.rs`, verbatim).
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
        instances: Optional::Absent,
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

/// Root holds the union of what the three source fixtures' roots hold: the privileged
/// `intent.accept`, and the production-trace data grant it delegates to the observer.
fn root_grant() -> CapabilityDescriptor {
    CapabilityDescriptor {
        delegation_depth: 4,
        profile: Optional::Present(CapabilityProfile {
            privileged_operations: vec![OperationName::new("intent.accept").expect("a name")],
            denied_operations: Vec::new(),
            data_grants: vec![DataGrant::ProductionTrace],
            cross_principal_sharing: true,
        }),
        ..grant(
            "cap_root",
            "service:continuumd",
            AuthorityLevel::Promote,
            Optional::Absent,
        )
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
        IntentJson::parse(PARENT_PACK.as_bytes()).expect("the fixture is admissible JSON"),
        WorkspaceHandle::new("ws_demo1").expect("a workspace handle"),
        [group],
    )
    .expect("a conforming pack and a well-formed expansion graph")
}

/// One provisioned deployment behind a byte boundary, serving **every** namespace the
/// script's commands drive: the Die Hard workspace/verification/task deployment
/// (`tests/check_verdict.rs`), the observe/evidence pair with one ingested node
/// (`tests/evidence_show.rs`), and the `context` family over one registered pack
/// (`tests/context_expand.rs`). `debug.*` and `repair.*` are registered and unserved, which
/// is exactly what their scenarios pin.
struct Fixture {
    server: Server,
    pair: LocalPair,
    components: SnapshotComponents,
    /// The node `observe.ingest` appended — the handle `evidence show` reads.
    node: EvidenceHandle,
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
                grant(
                    "cap_builder",
                    "agent:builder",
                    AuthorityLevel::Propose,
                    Optional::Absent,
                ),
                root.clone(),
            )
            .capability(
                grant(
                    "cap_runner",
                    "agent:runner",
                    AuthorityLevel::Execute,
                    Optional::Absent,
                ),
                root.clone(),
            )
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
            .family(WorkspaceFamily)
            .family(IntentFamily)
            .family(TaskFamily)
            .family(VerificationFamily)
            .family(ContextFamily)
            .family(EvidenceFamily::new())
            .family(ObserveFamily)
            .build();

        let intent = accept_intent(&mut daemon);
        let components = stage(&mut daemon, intent);
        let trace = daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new("traces/die-hard.jsonl").expect("a workspace path"),
                TRACE.as_bytes().to_vec(),
            )
            .expect("staging names its content");
        let node = ingest(&mut daemon, &trace);
        daemon.state_mut().put_context_pack(
            ContextHandle::new(PARENT).expect("a context handle"),
            pack_record(),
        );

        Self {
            server: Server::new(daemon, negotiated),
            pair: LocalPair::new(),
            components,
            node,
        }
    }

    /// The byte boundary every scripted invocation travels over.
    fn link(&mut self) -> LocalLink<'_> {
        LocalLink::new(&mut self.server, &mut self.pair)
    }
}

fn setup_envelope(
    who: &str,
    cap: &str,
    operation: &'static str,
    request_id: &str,
) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request_id).expect("a well-formed request id"),
        idempotency_key: Optional::Present(format!("idem-{request_id}")),
        actor: actor(who),
        capability: capability(cap),
        operation: OperationName::new(operation).expect("a registry name"),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: Optional::Absent,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
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
        fields.insert(key.to_owned(), IntentJson::String(value.to_owned()));
    }
    let outcome = daemon.dispatch(&OperationRequest {
        envelope: setup_envelope(
            "service:continuumd",
            "cap_root",
            "intent.accept",
            "req_accept",
        ),
        arguments: Arguments::IntentAccept(IntentAcceptRequest {
            proposal: intent.clone(),
            acceptance: Opaque::from_bytes(IntentJson::Object(fields).to_canonical_bytes()),
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
    let configuration = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("default.model.toml").expect("a workspace path"),
            CONFIG.as_bytes().to_vec(),
        )
        .expect("staging names its content");
    daemon.state_mut().models_mut().register(
        model_source(&Blake3Identity, [(MODULE_PATH, MODULE.as_bytes())])
            .expect("blake3 names the module set"),
        diehard::model().expect("the port builds"),
    );
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
        configuration: vec![configuration],
        file_components: Optional::Present(placements),
    }
}

fn ingest(daemon: &mut Daemon, trace: &Commitment) -> EvidenceHandle {
    // `observe.ingest` is `@task_starting`, so RFC 0026 requires a budget on the envelope;
    // an empty one declares no ceiling in any dimension (`tests/evidence_show.rs`).
    let envelope = RequestEnvelope {
        budget: Optional::Present(continuumd::protocol::envelope::Budget {
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
        ..setup_envelope(
            "agent:observer",
            "cap_observer",
            "observe.ingest",
            "req_ingest",
        )
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

// --- the scenario script -------------------------------------------------------------------

/// One pinned scenario: a stable artifact identifier, and everything the invocation
/// produced in all three formats.
struct Pinned {
    /// The stable artifact ID — also the golden file stem (see [`golden_path`]).
    artifact: &'static str,
    /// The command label `pretty` prepends, e.g. `"snapshot create"`.
    command: &'static str,
    /// The exit code the answer earned (identical across formats, asserted in [`pin`]).
    exit: i32,
    /// The `--format json` body, byte for byte.
    json: String,
    /// The `--format text` body, byte for byte.
    text: String,
    /// The `--format pretty` body, byte for byte.
    pretty: String,
}

/// A scenario that stopped before any answer could be rendered: a [`CliError`], pinned by
/// its display string and exit code. There is deliberately no JSON channel here — no answer
/// exists to be the machine document — which is itself part of the taxonomy being pinned.
struct PinnedError {
    artifact: &'static str,
    command: &'static str,
    exit: i32,
    /// The `Error: …` line `bin/continuum.rs` writes to stderr, byte for byte.
    stderr: String,
}

/// The exit class each pinned artifact is expected to land on. Asserted against the
/// *observed* codes in [`the_exit_code_matrix_reaches_all_five_classes_on_real_scenarios`],
/// so this table is a claim the script has to earn, not a description of it.
const EXPECTED_EXITS: [(&str, i32); 22] = [
    ("pr13-01-snapshot-create--sealed-die-hard-base", 0),
    ("pr13-02-check-start--fresh-submit-task-shaped", 0),
    ("pr13-03-check-result--die-hard-refuted", 3),
    ("pr13-04-snapshot-fork--overlay-on-sealed-base", 0),
    ("pr13-05-snapshot-seal--forked-snapshot", 0),
    ("pr13-06-check-start--generated-key-collision", 1),
    ("pr13-07-check-start--parked-with-continuation", 0),
    ("pr13-08-task-status--suspended-record", 0),
    ("pr13-09-check-await--bounded-wait-inconclusive", 4),
    ("pr13-10-check-result--budget-inconclusive", 4),
    ("pr13-11-task-resume--stale-snapshot-declined", 1),
    ("pr13-12-task-cancel--valid-continuation", 0),
    ("pr13-13-task-resume--valid-continuation-closes", 0),
    ("pr13-14-evidence-show--ingested-node", 0),
    ("pr13-15-context-expand--served-child-pack", 0),
    ("pr13-16-explain-compile--unsupported-surface", 1),
    ("pr13-17-debug-open--unsupported-surface", 1),
    ("pr13-18-debug-state--unsupported-surface", 1),
    ("pr13-19-repair-begin--unsupported-surface", 1),
    ("pr13-20-repair-review--unsupported-surface", 1),
    ("pr13-21-fault--no-transport-configured", 2),
    ("pr13-22-usage--unknown-command", 1),
];

/// Build one argv: the scenario's own words, then the connection flags every invocation
/// carries.
fn argv(words: &[&str], who: &str, cap: &str, format: &str, key: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
    for flag in ["--actor", who, "--capability", cap, "--format", format] {
        args.push(flag.to_owned());
    }
    if let Some(key) = key {
        args.push("--idempotency-key".to_owned());
        args.push(key.to_owned());
    }
    args
}

/// Run one invocation through [`cli::run`] over the fixture's byte boundary.
fn invoke(fixture: &mut Fixture, args: &[String]) -> Rendered {
    let mut link = fixture.link();
    cli::run(args, &mut link, false).expect("the call reaches a real frame and a real answer")
}

/// Pin one scenario: run it in all three formats, assert the exit code is a property of the
/// answer and not of the format, and keep every byte.
///
/// A repeated `@mutation` here is safe *because the protocol makes it so*: the second and
/// third invocations present the same idempotency key with a byte-identical canonical
/// request, which `rule idempotency.replay` answers with the first outcome verbatim. A
/// `@readonly` repeat is a plain re-read. Either way the three formats render one answer.
fn pin(
    pinned: &mut Vec<Pinned>,
    fixture: &mut Fixture,
    artifact: &'static str,
    command: &'static str,
    words: &[&str],
    (who, cap): (&str, &str),
    key: Option<&str>,
) -> Rendered {
    let json = invoke(fixture, &argv(words, who, cap, "json", key));
    let text = invoke(fixture, &argv(words, who, cap, "text", key));
    let pretty = invoke(fixture, &argv(words, who, cap, "pretty", key));
    assert_eq!(json.exit_code, text.exit_code, "{artifact}");
    assert_eq!(json.exit_code, pretty.exit_code, "{artifact}");
    pinned.push(Pinned {
        artifact,
        command,
        exit: text.exit_code,
        json: json.text,
        text: text.text.clone(),
        pretty: pretty.text,
    });
    text
}

/// The value of one `key  value` line a rendering printed.
fn line(rendered: &Rendered, key: &str) -> String {
    rendered
        .text
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key}  ")))
        .unwrap_or_else(|| panic!("no {key:?} line in:\n{}", rendered.text))
        .to_owned()
}

/// The whole scripted session: every stable command, in one causally ordered story over one
/// deployment, returning every pinned artifact.
///
/// The order is load-bearing and documented inline: `workspace.fork` supersedes its base in
/// the lineage (`dx03_falsification.rs`, attack 6), so everything that consumes a snapshot
/// runs before the fork that supersedes it — that is also what makes scenario 11's
/// `StaleSnapshot` a *real* staleness rather than a staged answer.
#[allow(clippy::too_many_lines)]
fn script(fixture: &mut Fixture) -> (Vec<Pinned>, Vec<PinnedError>) {
    let mut pinned = Vec::new();
    let builder = ("agent:builder", "cap_builder");
    let runner = ("agent:runner", "cap_runner");
    let reader = ("agent:reader", "cap_reader");
    let root = ("service:continuumd", "cap_root");

    let components = String::from_utf8(codec::to_bytes(&fixture.components).expect("encodable"))
        .expect("canonical JSON is UTF-8");
    let all_claims = TargetKind::AllClaims.as_wire();
    let interactive = Portfolio::Interactive.as_wire();
    let closing = CLOSING_BUDGET.to_string();
    let park = PARK_BUDGET.to_string();

    // 01 — create and seal the Die Hard base snapshot A.
    let create = pin(
        &mut pinned,
        fixture,
        "pr13-01-snapshot-create--sealed-die-hard-base",
        "snapshot create",
        &["snapshot", "create", "--components", &components, "--seal"],
        builder,
        Some("pr13-create-a"),
    );
    let base = line(&create, "snapshot");

    // 02 — submit a campaign over A that closes outright. No explicit key: this is the
    // generated-key lane, whose collision scenario 06 pins (bn-jmx97).
    let start_a = pin(
        &mut pinned,
        fixture,
        "pr13-02-check-start--fresh-submit-task-shaped",
        "check start",
        &[
            "check",
            "start",
            &base,
            "--target",
            "DieHard",
            "--target-kind",
            all_claims,
            "--portfolio",
            interactive,
            "--states",
            &closing,
        ],
        runner,
        None,
    );
    let task_a = line(&start_a, "task");

    // 03 — read the closed campaign's verdict: the engine's own refutation, exit 3.
    pin(
        &mut pinned,
        fixture,
        "pr13-03-check-result--die-hard-refuted",
        "check result",
        &["check", "result", &task_a],
        runner,
        None,
    );

    // 04 — fork A with an overlay. This supersedes A in its lineage, which is why every
    // A-consuming scenario above ran first.
    let fork = pin(
        &mut pinned,
        fixture,
        "pr13-04-snapshot-fork--overlay-on-sealed-base",
        "snapshot fork",
        &[
            "snapshot",
            "fork",
            &base,
            "--overlay",
            "elsewhere.txt=elsewhere",
        ],
        builder,
        Some("pr13-fork-b"),
    );
    let forked = line(&fork, "snapshot");

    // 05 — seal the fork, making it a valid semantic input.
    let seal = pin(
        &mut pinned,
        fixture,
        "pr13-05-snapshot-seal--forked-snapshot",
        "snapshot seal",
        &["snapshot", "seal", &forked],
        builder,
        Some("pr13-seal-b"),
    );
    let sealed_b = line(&seal, "snapshot");

    // 06 — the bn-jmx97 ratified refusal, pinned as a regression guard: a second
    // `verification.start` with a *different* body under the same *generated* key
    // (scenario 02 recorded it) is the daemon's own `IdempotencyKeyReused` — the loud,
    // typed refusal the ratified counter-key shape deliberately produces (see the header's
    // "Ratified behaviour" section and `wire::Connection::with_idempotency_key`).
    pin(
        &mut pinned,
        fixture,
        "pr13-06-check-start--generated-key-collision",
        "check start",
        &[
            "check",
            "start",
            &sealed_b,
            "--target",
            "DieHard",
            "--target-kind",
            all_claims,
            "--portfolio",
            interactive,
            "--states",
            &park,
        ],
        runner,
        None,
    );

    // 07 — the same submit under an explicit key (the documented remediation): parks with a
    // continuation under the real four-state budget.
    let start_b = pin(
        &mut pinned,
        fixture,
        "pr13-07-check-start--parked-with-continuation",
        "check start",
        &[
            "check",
            "start",
            &sealed_b,
            "--target",
            "DieHard",
            "--target-kind",
            all_claims,
            "--portfolio",
            interactive,
            "--states",
            &park,
        ],
        runner,
        Some("pr13-start-b"),
    );
    let task_b = line(&start_b, "task");
    let cont_b = line(&start_b, "continuation");

    // 08 — the suspended record, cost/budget/epochs and all.
    pin(
        &mut pinned,
        fixture,
        "pr13-08-task-status--suspended-record",
        "task status",
        &["task", "status", &task_b],
        runner,
        None,
    );

    // 09 — the bounded wait reports the same parked verdict within the caller's own bound.
    pin(
        &mut pinned,
        fixture,
        "pr13-09-check-await--bounded-wait-inconclusive",
        "check await",
        &["check", "await", &task_b, "--timeout-ms", "10"],
        runner,
        None,
    );

    // 10 — the parked campaign's verdict: INV-008's typed unknown, exit 4.
    pin(
        &mut pinned,
        fixture,
        "pr13-10-check-result--budget-inconclusive",
        "check result",
        &["check", "result", &task_b],
        runner,
        None,
    );

    // setup — the lineage moves past B for real: a fork from B supersedes it
    // (`dx03_falsification.rs`'s attack-6 recipe). Driven through the same CLI surface,
    // not pinned: the fork *shape* is scenario 04's.
    let fork_past = invoke(
        fixture,
        &argv(
            &[
                "snapshot",
                "fork",
                &sealed_b,
                "--overlay",
                "past.txt=moved-on",
            ],
            builder.0,
            builder.1,
            "text",
            Some("pr13-fork-past"),
        ),
    );
    let past = line(&fork_past, "snapshot");

    // 11 — resuming B's continuation is now a real staleness: the daemon's own typed
    // `StaleSnapshot`, exit 1.
    pin(
        &mut pinned,
        fixture,
        "pr13-11-task-resume--stale-snapshot-declined",
        "task resume",
        &["task", "resume", &cont_b, "--states", &closing],
        runner,
        Some("pr13-resume-stale"),
    );

    // 12 — cancelling the still-suspended campaign leaves a valid continuation, printed in
    // full (`rule task.cancel_correct`).
    pin(
        &mut pinned,
        fixture,
        "pr13-12-task-cancel--valid-continuation",
        "task cancel",
        &["task", "cancel", &task_b],
        runner,
        Some("pr13-cancel-b"),
    );

    // setup — a third campaign whose continuation stays valid: seal the fork-past
    // snapshot and park a campaign over it.
    let seal_c = invoke(
        fixture,
        &argv(
            &["snapshot", "seal", &past],
            builder.0,
            builder.1,
            "text",
            Some("pr13-seal-c"),
        ),
    );
    let sealed_c = line(&seal_c, "snapshot");
    let start_c = invoke(
        fixture,
        &argv(
            &[
                "check",
                "start",
                &sealed_c,
                "--target",
                "DieHard",
                "--target-kind",
                all_claims,
                "--portfolio",
                interactive,
                "--states",
                &park,
            ],
            runner.0,
            runner.1,
            "text",
            Some("pr13-start-c"),
        ),
    );
    let cont_c = line(&start_c, "continuation");

    // 13 — a valid resume spends the continuation and closes the campaign on Die Hard's
    // frozen sixteen states, exit 0.
    pin(
        &mut pinned,
        fixture,
        "pr13-13-task-resume--valid-continuation-closes",
        "task resume",
        &["task", "resume", &cont_c, "--states", &closing],
        runner,
        Some("pr13-resume-c"),
    );

    // 14 — read the evidence node the fixture's real `observe.ingest` appended.
    let node = fixture.node.as_str().to_owned();
    pin(
        &mut pinned,
        fixture,
        "pr13-14-evidence-show--ingested-node",
        "evidence show",
        &["evidence", "show", &node],
        reader,
        None,
    );

    // 15 — follow a real expansion handle: the served `context` family answers a child pack.
    pin(
        &mut pinned,
        fixture,
        "pr13-15-context-expand--served-child-pack",
        "context expand",
        &[
            "context",
            "expand",
            "--context",
            PARENT,
            "--anchor",
            "node-42",
            "--relation",
            ExpansionRelation::SourceSpan.as_wire(),
            "--depth",
            "1",
            "--states",
            "0",
        ],
        root,
        Some("pr13-expand"),
    );

    // 16–20 — the honestly-unsupported surfaces: registered operations this deployment does
    // not serve. Real round trips, the daemon's own `UnsupportedSemanticFeature`, exit 1
    // with the typed `depth  unsupported` beside the exact operation.
    pin(
        &mut pinned,
        fixture,
        "pr13-16-explain-compile--unsupported-surface",
        "explain compile",
        &[
            "explain",
            "compile",
            "--evidence-root",
            "ev_failure1",
            "--question",
            "why did AckImpliesDurable fail?",
            "--guarantee",
            "ReplayPreserving",
            "--bytes",
            "4096",
        ],
        root,
        Some("pr13-explain"),
    );
    pin(
        &mut pinned,
        fixture,
        "pr13-17-debug-open--unsupported-surface",
        "debug open",
        &[
            "debug",
            "open",
            "crash_die_hard_1",
            "--observer",
            "observer-a",
        ],
        root,
        Some("pr13-debug-open"),
    );
    pin(
        &mut pinned,
        fixture,
        "pr13-18-debug-state--unsupported-surface",
        "debug state",
        &["debug", "state", "dbg_branch01"],
        root,
        None,
    );
    pin(
        &mut pinned,
        fixture,
        "pr13-19-repair-begin--unsupported-surface",
        "repair begin",
        &[
            "repair",
            "begin",
            "crash_die_hard_1",
            "--gate-profile",
            "default",
        ],
        root,
        Some("pr13-repair-begin"),
    );
    pin(
        &mut pinned,
        fixture,
        "pr13-20-repair-review--unsupported-surface",
        "repair review",
        &["repair", "review", "rt_transaction1"],
        root,
        None,
    );

    // 21 — the real fault: `NullTransport` is what `bin/continuum.rs` runs today, and a
    // connection that fails below the protocol is exit 2 with no answer to render.
    let mut errors = Vec::new();
    let mut null = NullTransport;
    let fault = cli::run(
        &argv(
            &["task", "status", "task_neverstarted1"],
            runner.0,
            runner.1,
            "text",
            None,
        ),
        &mut null,
        false,
    )
    .expect_err("no transport is configured, honestly");
    assert!(matches!(fault, CliError::Connection(_)), "{fault:?}");
    errors.push(PinnedError {
        artifact: "pr13-21-fault--no-transport-configured",
        command: "task status",
        exit: fault.exit_code(),
        stderr: fault.to_string(),
    });

    // 22 — the parse-refusal half of exit class 1: an unknown command is a usage error that
    // names every noun this build recognizes, and reaches no wire.
    let usage = cli::run(
        &argv(&["promote"], runner.0, runner.1, "text", None),
        &mut null,
        false,
    )
    .expect_err("an unknown command is a usage error");
    assert!(matches!(usage, CliError::Usage(_)), "{usage:?}");
    errors.push(PinnedError {
        artifact: "pr13-22-usage--unknown-command",
        command: "(none)",
        exit: usage.exit_code(),
        stderr: usage.to_string(),
    });

    (pinned, errors)
}

// --- golden storage ------------------------------------------------------------------------

/// Where one golden artifact lives: `tests/golden/<artifact>.<channel>.golden`, where
/// `<artifact>` is `pr13-<nn>-<command>--<scenario>` and `<channel>` is `json`, `text`, or
/// `stderr`. The artifact ID is the stable name the evidence summary
/// (`tests/evidence/pr13-exit.json`) records.
fn golden_path(artifact: &str, channel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{artifact}.{channel}.golden"))
}

fn read_golden(artifact: &str, channel: &str) -> String {
    let path = golden_path(artifact, channel);
    std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "golden artifact {} is not readable ({error}); a missing golden is a failing \
             test, never a silently-passing one. Regenerate deliberately with \
             PR13_EXIT_BLESS=1 and review the diff as a contract change.",
            path.display()
        )
    })
}

/// The one comparison the pinning test uses: exact byte equality, no normalization.
fn matches_golden(observed: &str, golden: &str) -> bool {
    observed == golden
}

/// `PR13_EXIT_BLESS=1` rewrites every golden from the live run **and then panics**, so a
/// blessing run is always red and its diff is always reviewed (INV-004: the suite must not
/// certify its own new bytes by going green in the same run that wrote them).
fn bless(pinned: &[Pinned], errors: &[PinnedError]) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    std::fs::create_dir_all(&root).expect("the golden directory is creatable");
    for scenario in pinned {
        std::fs::write(golden_path(scenario.artifact, "json"), &scenario.json)
            .expect("a golden is writable");
        std::fs::write(golden_path(scenario.artifact, "text"), &scenario.text)
            .expect("a golden is writable");
    }
    for scenario in errors {
        std::fs::write(golden_path(scenario.artifact, "stderr"), &scenario.stderr)
            .expect("a golden is writable");
    }
    panic!(
        "PR13_EXIT_BLESS rewrote {} golden artifacts under {}; rerun without the variable \
         and review the diff as a contract change",
        pinned.len() * 2 + errors.len(),
        root.display()
    );
}

// --- the evidence --------------------------------------------------------------------------

/// **"Golden tests pin JSON … and concise terminal output."** Every scenario's machine
/// document and terminal body, byte for byte against the committed artifacts, and every
/// fault's stderr line the same way.
#[test]
fn every_pinned_artifact_matches_its_committed_golden_bytes() {
    let mut fixture = Fixture::fresh();
    let (pinned, errors) = script(&mut fixture);
    if std::env::var_os("PR13_EXIT_BLESS").is_some() {
        bless(&pinned, &errors);
    }
    for scenario in &pinned {
        for (channel, observed) in [("json", &scenario.json), ("text", &scenario.text)] {
            let golden = read_golden(scenario.artifact, channel);
            assert!(
                matches_golden(observed, &golden),
                "{} [{channel}] diverged from its committed golden\n--- observed ---\n{observed}\n--- golden ---\n{golden}",
                scenario.artifact
            );
        }
    }
    for scenario in &errors {
        let golden = read_golden(scenario.artifact, "stderr");
        assert!(
            matches_golden(&scenario.stderr, &golden),
            "{} [stderr] diverged\n--- observed ---\n{}\n--- golden ---\n{golden}",
            scenario.artifact,
            scenario.stderr
        );
    }
}

/// **"… exit codes."** The observed exit code of every scenario matches [`EXPECTED_EXITS`],
/// and the five classes are all reached — each by a scenario that genuinely produced it
/// (see this file's module doc for what "genuinely" means per class).
#[test]
fn the_exit_code_matrix_reaches_all_five_classes_on_real_scenarios() {
    let mut fixture = Fixture::fresh();
    let (pinned, errors) = script(&mut fixture);

    let mut observed: BTreeMap<&str, i32> = BTreeMap::new();
    for scenario in &pinned {
        observed.insert(scenario.artifact, scenario.exit);
    }
    for scenario in &errors {
        observed.insert(scenario.artifact, scenario.exit);
    }
    assert_eq!(observed.len(), EXPECTED_EXITS.len(), "every scenario ran");
    for (artifact, expected) in EXPECTED_EXITS {
        assert_eq!(
            observed.get(artifact),
            Some(&expected),
            "{artifact} earns exit {expected}"
        );
    }
    // All five classes, by code, from the same observations the goldens retain.
    for class in Exit::ALL {
        assert!(
            observed.values().any(|&code| code == class.code()),
            "exit class {} ({}) is reached by a real scenario",
            class.code(),
            class.token()
        );
    }
}

/// **INV-003, held over the pinned set.** For every pinned scenario, the terminal body is a
/// pure projection of that same scenario's machine document: `contract::divergence` is
/// empty. The text never invents a fact the JSON does not carry and never renders a shared
/// fact differently.
#[test]
fn the_terminal_body_of_every_pinned_scenario_is_a_pure_projection_of_its_json() {
    let mut fixture = Fixture::fresh();
    let (pinned, _) = script(&mut fixture);
    for scenario in &pinned {
        let document = Json::parse(scenario.json.trim_end().as_bytes())
            .unwrap_or_else(|error| panic!("{}: --json parses: {error:?}", scenario.artifact));
        assert_eq!(
            contract::divergence(&scenario.text, &document),
            None,
            "{}\n--- terminal ---\n{}--- machine ---\n{}",
            scenario.artifact,
            scenario.text,
            scenario.json
        );
    }
}

/// The third format, pinned by construction: `pretty` is `text` plus exactly one prepended
/// `command  <name>` label line, for every pinned scenario — nothing is behind colour, a
/// table, or a graph, because there is none (G8-06).
#[test]
fn pretty_is_text_plus_the_command_label_for_every_pinned_scenario() {
    let mut fixture = Fixture::fresh();
    let (pinned, _) = script(&mut fixture);
    for scenario in &pinned {
        assert_eq!(
            scenario.pretty,
            format!("command  {}\n{}", scenario.command, scenario.text),
            "{}",
            scenario.artifact
        );
    }
}

/// **Determinism (INV-005), mechanically.** The whole script, run twice from two
/// independently provisioned daemons, renders byte-identical artifacts — with no
/// normalization anywhere between the renderer and the comparison.
#[test]
fn two_independent_provisionings_render_byte_identical_artifacts() {
    let mut first_fixture = Fixture::fresh();
    let (first, first_errors) = script(&mut first_fixture);
    let mut second_fixture = Fixture::fresh();
    let (second, second_errors) = script(&mut second_fixture);

    assert_eq!(first.len(), second.len());
    for (a, b) in first.iter().zip(second.iter()) {
        assert_eq!(a.artifact, b.artifact);
        assert_eq!(a.exit, b.exit, "{}", a.artifact);
        assert_eq!(a.json, b.json, "{}: json is deterministic", a.artifact);
        assert_eq!(a.text, b.text, "{}: text is deterministic", a.artifact);
        assert_eq!(
            a.pretty, b.pretty,
            "{}: pretty is deterministic",
            a.artifact
        );
    }
    assert_eq!(first_errors.len(), second_errors.len());
    for (a, b) in first_errors.iter().zip(second_errors.iter()) {
        assert_eq!(a.artifact, b.artifact);
        assert_eq!(a.stderr, b.stderr, "{}", a.artifact);
        assert_eq!(a.exit, b.exit, "{}", a.artifact);
    }
}

/// **Anti-vacuity, half one: the byte comparison can fail.** For every committed golden, a
/// copy with one byte flipped is rejected by the same comparison
/// [`every_pinned_artifact_matches_its_committed_golden_bytes`] uses. A pinning test whose
/// comparison could not tell two artifacts apart would prove nothing about either.
#[test]
fn a_single_byte_perturbation_of_any_golden_artifact_is_detected() {
    let mut checked = 0usize;
    for (artifact, _) in EXPECTED_EXITS {
        let channels: &[&str] = if artifact.contains("-fault--") || artifact.contains("-usage--") {
            &["stderr"]
        } else {
            &["json", "text"]
        };
        for channel in channels {
            let golden = read_golden(artifact, channel);
            assert!(
                !golden.is_empty(),
                "{artifact} [{channel}] pins actual bytes"
            );
            let mut bytes = golden.clone().into_bytes();
            let index = bytes.len() / 2;
            bytes[index] ^= 0x01;
            let perturbed = String::from_utf8_lossy(&bytes).into_owned();
            assert!(
                !matches_golden(&perturbed, &golden),
                "{artifact} [{channel}]: a single flipped byte must be detected"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 20 * 2 + 2, "every golden was perturbed");
}

/// **Anti-vacuity, half two: the projection check can fail, on this file's own artifacts.**
/// Three mutations of a real scenario's real terminal body — a changed value, an invented
/// key, and a dropped line — and `contract::divergence` reports each one. This is the
/// mutation check on the suite itself: if `divergence` were silenced or weakened, these
/// assertions are the ones that would go green on drift, so they demonstrate it has teeth
/// on exactly the artifacts the suite pins.
#[test]
fn a_deliberately_altered_rendering_fails_the_projection_check() {
    let mut fixture = Fixture::fresh();
    let (pinned, _) = script(&mut fixture);
    let scenario = pinned
        .iter()
        .find(|scenario| scenario.artifact == "pr13-08-task-status--suspended-record")
        .expect("the suspended-record scenario is pinned");
    let document = Json::parse(scenario.json.trim_end().as_bytes()).expect("--json parses");
    assert_eq!(contract::divergence(&scenario.text, &document), None);

    // A changed value: the terminal claiming a different budget than the machine channel.
    let changed = scenario.text.replace(
        &format!("record.budget.states  {PARK_BUDGET}"),
        "record.budget.states  9",
    );
    assert_ne!(changed, scenario.text, "the perturbation changed a line");
    let reported = contract::divergence(&changed, &document).expect("a changed value is reported");
    assert!(reported.contains("record.budget.states"), "{reported}");

    // An invented key: a fact the machine document does not carry — the smallest prose-only
    // interface INV-003 forbids.
    let invented = format!("{}invented.fact  yes\n", scenario.text);
    assert!(contract::divergence(&invented, &document).is_some());

    // A dropped line: the text no longer re-renders from the document under its own keys.
    let dropped: String = scenario
        .text
        .lines()
        .filter(|line| !line.starts_with("record.status  "))
        .map(|line| format!("{line}\n"))
        .collect();
    assert_ne!(dropped, scenario.text);
    // Dropping a whole line leaves a projection of *fewer* keys, which still re-renders —
    // so the byte-exact golden is what catches a dropped line, and this asserts exactly
    // that division of labour.
    assert!(
        !matches_golden(&dropped, &scenario.text),
        "a dropped line is caught by the byte comparison"
    );
}

/// The projection check distinguishes every pinned scenario from every other: scenario X's
/// terminal body diverges from scenario Y's machine document whenever X ≠ Y. A check that
/// answered "no divergence" across different scenarios would be too weak to mean anything
/// within one.
#[test]
fn the_projection_check_distinguishes_every_pinned_scenario_from_every_other() {
    let mut fixture = Fixture::fresh();
    let (pinned, _) = script(&mut fixture);
    let documents: Vec<Json> = pinned
        .iter()
        .map(|scenario| Json::parse(scenario.json.trim_end().as_bytes()).expect("--json parses"))
        .collect();
    let mut distinguished = 0usize;
    for (i, scenario) in pinned.iter().enumerate() {
        for (j, document) in documents.iter().enumerate() {
            if i == j {
                continue;
            }
            if contract::divergence(&scenario.text, document).is_some() {
                distinguished += 1;
            }
        }
    }
    let pairs = pinned.len() * (pinned.len() - 1);
    assert_eq!(
        distinguished, pairs,
        "every cross-scenario pair diverges ({distinguished}/{pairs})"
    );
}

/// The retained machine-readable summary (`tests/evidence/pr13-exit.json`) agrees with what
/// this file observed: every scenario row's artifact, command, exit code and exit class
/// match the live run, the exit-code matrix rows are exactly the observed partition, and
/// every golden file the summary names exists. The summary is therefore itself under test,
/// not a hand-maintained claim.
#[test]
fn the_evidence_summary_agrees_with_what_this_file_observed() {
    let mut fixture = Fixture::fresh();
    let (pinned, errors) = script(&mut fixture);

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/evidence/pr13-exit.json");
    let raw = std::fs::read(&path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()));
    let summary = Json::parse(&raw).expect("the evidence summary is well-formed JSON");

    let Json::Array(rows) = contract::value_at(&summary, "scenarios") else {
        panic!("the summary carries a scenarios array");
    };
    assert_eq!(
        rows.len(),
        pinned.len() + errors.len(),
        "one summary row per scenario"
    );

    let mut observed: BTreeMap<&str, (&str, i32)> = BTreeMap::new();
    for scenario in &pinned {
        observed.insert(scenario.artifact, (scenario.command, scenario.exit));
    }
    for scenario in &errors {
        observed.insert(scenario.artifact, (scenario.command, scenario.exit));
    }

    for (index, row) in rows.iter().enumerate() {
        let artifact = contract::render_value(contract::value_at(row, "artifact"));
        let command = contract::render_value(contract::value_at(row, "command"));
        let exit: i32 = contract::render_value(contract::value_at(row, "exit_code"))
            .parse()
            .expect("exit_code is a number");
        let class = contract::render_value(contract::value_at(row, "exit_class"));
        let (expected_command, expected_exit) = observed
            .get(artifact.as_str())
            .unwrap_or_else(|| panic!("summary row {index} names an unknown artifact {artifact}"));
        assert_eq!(&command, expected_command, "{artifact}");
        assert_eq!(exit, *expected_exit, "{artifact}");
        let expected_class = Exit::ALL
            .iter()
            .find(|member| member.code() == exit)
            .expect("a documented code")
            .token();
        assert_eq!(class, expected_class, "{artifact}");

        let Json::Array(channels) = contract::value_at(row, "channels") else {
            panic!("{artifact}: the summary names its golden channels");
        };
        for channel in channels {
            let channel = contract::render_value(channel);
            assert!(
                golden_path(&artifact, &channel).is_file(),
                "{artifact}: the {channel} golden the summary names exists"
            );
        }
    }

    // The matrix is the same observation, partitioned by code.
    for class in Exit::ALL {
        let key = format!("exit_code_matrix.{}", class.code());
        let Json::Array(members) = contract::value_at(&summary, &key) else {
            panic!("the summary carries matrix row {key}");
        };
        let expected: Vec<&str> = EXPECTED_EXITS
            .iter()
            .filter(|(_, code)| *code == class.code())
            .map(|(artifact, _)| *artifact)
            .collect();
        let named: Vec<String> = members.iter().map(contract::render_value).collect();
        assert_eq!(named, expected, "matrix row {key}");
    }

    // The generated-key behaviour is annotated, by bone, against the scenario that pins it
    // (bn-jmx97 — originally a retained defect record, since ratified; see the header).
    let residual = contract::render_value(contract::value_at(&summary, "residuals[0].bone"));
    assert_eq!(residual, "bn-jmx97");
    let pinned_by = contract::render_value(contract::value_at(&summary, "residuals[0].pinned_by"));
    assert_eq!(pinned_by, "pr13-06-check-start--generated-key-collision");
}
