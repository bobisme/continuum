//! Acceptance re-derivation for release gate **G2-03** (bone `bn-24ulk`).
//!
//! > explicit handles and resumability
//! >
//! > — `notes/plan/docs/52_RELEASE_GATES_REV3.md:33`, G2 bullet 3
//!
//! # What this file re-derives, and what it deliberately does not touch
//!
//! `notes/plan/notes/PHASE_A_EXIT_PACKAGE.md` re-ran the *delivering* suites from clean
//! state. This file does not do that, and it does not restate the G1 criteria whose
//! acceptance evidence is already merged:
//!
//! | Already proven, elsewhere | Not repeated here |
//! |---|---|
//! | `tests/gate_g1_02_acceptance.rs` (`bn-3j01v`) — every one of the 75 operations names its subjects by handle, 666 leaves adjudicated | this file adjudicates no IDL leaf |
//! | `tests/gate_g1_04_acceptance.rs` (`bn-16v3x`) — `task.resume` validates 13 input classes before any reuse | this file drives no *refused* resume |
//! | `tests/gate_g1_03_acceptance.rs` (`bn-3huh7`) — the idempotency ledger is honest under a *fresh* `request_id` | this file replays the **identical** request, which is the case an interrupted agent is actually in |
//!
//! G1-02 is a property of the protocol *text*: the operations take handles. G1-04 is a
//! property of the *daemon*: a continuation is checked before it is honored. G2-03 sits on
//! the agent's side of the same seam and asks a question neither answers: **can an agent that
//! loses its own memory mid-workflow still finish?** The interruption here is of the
//! *client*, never of the daemon — the daemon keeps running and keeps its state, and what is
//! destroyed is everything the agent knew.
//!
//! # The instrument
//!
//! One eleven-step agent workflow, driven through [`Daemon::dispatch`]:
//!
//! ```text
//! intent.accept → intent.get → workspace.create → workspace.fork → workspace.seal
//!   → verification.start (parks) → task.status → task.resume (closes) → task.status
//!   → verification.result → evidence.query
//! ```
//!
//! Three instruments read it, and each carries its own negative control:
//!
//! 1. **The amnesia probe** ([`amnesia_at_every_interruption_point_lands_in_the_same_world`]).
//!    At every one of the twelve interruption points the agent's [`Session`] — its whole
//!    working memory — is *destroyed* and rebuilt from the wire alone: the responses it
//!    already holds, plus one typed re-orientation read (`task.status`). The rebuilt session
//!    must equal the one the uninterrupted agent held, field for field, and the world the
//!    interrupted run leaves must equal the control run's.
//! 2. **The handle-sufficiency census**
//!    ([`every_handle_a_request_carries_was_returned_or_is_derivable`]). Mechanical, and it
//!    reads *requests* rather than the author's summary of them: every request is encoded to
//!    its canonical wire form, every handle-shaped token in it is extracted with its JSON
//!    path, and each is resolved to the response that returned it, to content the agent can
//!    hash for itself, or to the connection's own capability. A token resolved by none of the
//!    three is a fact that lived only in the agent's memory.
//! 3. **The replay probe**
//!    ([`a_lost_response_is_recovered_by_replaying_the_identical_request`]). The agent that
//!    crashed between send and receive does not know whether its mutation landed. Every one of
//!    the workflow's six mutations is re-sent verbatim and must answer with the same wire
//!    bytes and move nothing.
//!
//! # Verdict
//!
//! **SATISFIED-AT-NARROWER-SCOPE.** Every step of the workflow is amnesia-safe. The narrowing
//! is not about any step; it is about what the agent must still be holding for the probe to
//! start.
//!
//! 1. **Amnesia is survivable; total amnesia is not.** The rebuild needs the agent's
//!    *ledger* — the responses it received. An agent that lost those too is down to what it
//!    can call with no handle at all, and that surface narrows twice: twelve of the 75
//!    operations have a handle-free *request body*
//!    ([`the_protocol_declares_no_enumeration_over_the_workflow_classes`]), of which one is
//!    both served in this build and complete without an envelope subject
//!    ([`re_entry_from_nothing_reaches_one_servable_operation`], which measures both
//!    narrowings by driving a refusal for each). That one — `evidence.query` over the whole
//!    graph — answers empty, because nothing in this build writes an evidence node. No
//!    operation anywhere in the 75 enumerates tasks, workspaces, intents or continuations, so
//!    re-entry from nothing requires a durable client-side ledger of handles: a client
//!    obligation the protocol does not discharge.
//! 2. **The declared discovery surface is empty on every result.** `next_operations` is a
//!    `required` list on every `ResultEnvelope` and RFC 0026 calls it "the safe recovery and
//!    discovery surface"; this daemon emits `[]` on all eleven steps
//!    ([`the_declared_discovery_surface_is_empty_on_every_result`]). `daemon/result.rs` says
//!    so outright, and an empty list is the statement RFC 0026 says it is. It still means the
//!    resumability measured here is carried entirely by *handles the agent kept*, never by the
//!    protocol offering the next move.
//! 3. **The workflow is bounded by the 30 servable operations.** 45 of the registry's 75 have
//!    no constructible request in this build; the eleven steps are drawn from the 30.
//!
//! # Findings (INV-007, INV-008)
//!
//! - **F1 — one typed read re-derives the whole mid-workflow position.** `task.status`
//!   answers, from a `task_` handle alone, with the snapshot, the intent, the continuation,
//!   the status, the budget, the cost, the milestones and the committed evidence. Every
//!   session field the workflow needs after `verification.start` therefore has *two*
//!   independent routes — the response that first returned it and this record — and
//!   [`the_two_rebuild_routes_agree_at_every_interruption_point`] asserts they agree rather
//!   than assuming it.
//! - **F2 — four request inputs are echoed by no response, and all four are content the agent
//!   can hash for itself.** The `in_` intent handle and the three staged `Commitment`s of
//!   `SnapshotComponents.files`/`configuration` never appear in any response of this workflow.
//!   They are recovered by re-deriving them from the agent's own bytes through
//!   `DaemonState::commit_of` and `ContentIdentifier::identify` — the public seam a client
//!   has — which is IDL §4's "content-addressed identities are derivable by anyone holding the
//!   content" used as a *recovery* mechanism rather than as a property of identity.
//!   [`negative_control_without_the_derivable_closure_four_inputs_are_flagged`] is the
//!   measurement: switch the closure off and the census reports exactly those four.
//! - **F3 — the protocol has no staging operation, so the commitments enter out of band.**
//!   `DaemonState::stage` is reached through `Daemon::state_mut`, not through the wire, and no
//!   operation enumerates staged content. F2's re-derivation is what saves this: the agent
//!   does not need the daemon to tell it what it staged.
//! - **F4 — a class prefix does not make a token a handle.** `workspace.seal`'s `root_digest`
//!   is a `Commitment` spelled `ws_…`, and every `ArtifactRef.commitment` this workflow
//!   returns is a `Commitment` spelled `task_…`. The census counts them as handles anyway,
//!   which is the conservative direction — it can only *add* obligations — and
//!   [`a_class_prefix_does_not_make_a_token_a_handle`] records the over-count.
//! - **F5 — six declared enum spellings are handle-shaped.** `task_started` and
//!   `task_suspended` (`ResultStatus`), `proof_goal` and `proof_dependency`, `receipt_generation`
//!   and `defect_mutants` all satisfy `ArtifactHandle`'s pattern under a declared class prefix,
//!   so the protocol's own vocabulary is indistinguishable from a handle by shape alone. The
//!   census therefore resolves declared vocabulary *before* it resolves handles, and
//!   [`the_vocabulary_collision_between_result_status_and_the_task_class_is_real`] pins the
//!   collision so that ordering is a documented necessity rather than an accident. Two of the
//!   six are values this workflow really does return.
//! - **F6 — the capability token appears in every request and in no response.** Confirmed
//!   empirically over eleven responses ([`the_capability_token_reaches_no_response`]). It
//!   follows that an agent cannot recover its own authority from the wire: `cap_*` is
//!   connection configuration, which is why the census resolves it as a constant rather than
//!   as a returned handle.
//! - **F7 — an identical replay is free, and the idempotency key is what makes it free.**
//!   Replaying each of the six mutations verbatim returns byte-identical wire bytes and moves
//!   nothing. Re-sending the *same* `workspace.fork` under a fresh key is not a replay at all:
//!   it is re-evaluated and refused `StaleSnapshot`, because the agent's own first fork
//!   superseded the base in that lineage. So "an agent can always safely resend without knowing
//!   whether the first landed" is true here **only for an agent that can reconstruct its own
//!   key** — deterministic keying is a client obligation the protocol does not enforce, and it
//!   is the one client-side assumption this file's replay probe rests on. See
//!   [`a_re_keyed_resend_is_refused_as_stale_so_the_idempotency_key_is_load_bearing`]. The
//!   refusal itself is clean: it carries no payload and moves nothing.
//!
//! # Absences — what this file does not probe (INV-007)
//!
//! - **No transport, no CLI, no concurrency.** Every request goes through
//!   [`Daemon::dispatch`] in process, one at a time. "The agent crashed" is modelled by
//!   destroying the client value, not by killing a process.
//! - **No daemon restart.** G1-04's `regression_a_continuation_survives_a_restart_with_its_pins`
//!   measures the restart boundary (bn-20142); this file's interruption is on the other side
//!   of the connection and says nothing about it.
//! - **No evidence graph and no Context Pack.** `verification.*` in this build commits no `ev_`
//!   node — the `task.committed_evidence` omission on every task result says so — so
//!   `evidence.get` and `context.expand` have nothing to walk here and are not driven.
//! - **The world fingerprint reads workspaces, lineages, continuations and staged content by
//!   handle**, from the union of every run's harvest, because the daemon exposes total
//!   iteration only over tasks, intents and evidence. A workspace no run ever heard of would
//!   go unread.
//! - **The agent's *plan* is not treated as memory.** Request ids and idempotency keys are
//!   functions of the step, so an amnesiac reconstructs them from its own program. An agent
//!   drawing random keys would not be able to, and the replay probe says so where it rests on
//!   this.

use std::collections::{BTreeMap, BTreeSet};

use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json as ContractJson;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_workspace::lineage::ForkName;
use continuum_workspace::snapshot::WorkspacePath;
use continuumd::codec;
use continuumd::codec::json::{Json, base64url};
use continuumd::daemon::evidence::EvidenceFamily;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::state::{DaemonState, IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest, identity};
use continuumd::protocol::envelope::{
    Budget, EpochSet, RequestEnvelope, SemanticVerdictValue, Verdict,
};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::evidence::EvidenceQueryRequest;
use continuumd::protocol::operations::intent::{IntentAcceptRequest, IntentGetRequest};
use continuumd::protocol::operations::task::{TaskResumeRequest, TaskStatusRequest};
use continuumd::protocol::operations::verification::{
    VerificationResultRequest, VerificationStartRequest,
};
use continuumd::protocol::operations::whiteboard::WhiteboardCompileRequest;
use continuumd::protocol::operations::workspace::{
    WorkspaceCreateRequest, WorkspaceForkRequest, WorkspaceSealRequest,
};
use continuumd::protocol::registry::{self, ENCODINGS, ENUMS, HANDLES, NAMED_STRUCTS, OPERATIONS};
use continuumd::protocol::scalar::{
    ActorId, ArtifactHandle, CapabilityHandle, Commitment, ContinuationHandle, EpochIdentity,
    IntentHandle, Opaque, OperationName, ProtocolVersion, RequestId, TaskHandle, Timestamp,
    WorkspaceHandle,
};
use continuumd::protocol::shared::{
    EvidenceQuery, FileOverlay, SnapshotComponents, SnapshotEpochs, Target,
};
use continuumd::protocol::spec::{
    Annotation, FieldSpec, Nullable, Optional, Presence, ProtocolEnum, StructSpec,
};
use continuumd::protocol::vocabulary::{
    AssuranceClass, AuthorityLevel, Encoding, ErrorCode, Portfolio, ResultStatus, SemanticVerdict,
    TargetKind, TaskStatus,
};
use continuumd::transport;

// =========================================================================================
// 0. The agent's own inputs, and the deployment it talks to
// =========================================================================================

/// The TV-009 port's model — bytes the agent brought with it.
const DIE_HARD_MODEL: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");

/// The TV-009 port's model configuration.
const DIE_HARD_CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");

/// The Die Hard Intent Contract.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// Where the model lives inside the snapshot.
const MODULE_PATH: &str = "DieHard.ctm";

/// The README the base snapshot carries.
const README: &str = "# TV-009\n";

/// The README the fork overlays, which is what makes the fork a *different* snapshot.
const FORKED_README: &str = "# TV-009 (forked)\n";

/// The `states` ceiling that parks the Die Hard campaign short of its sixteen states.
const PARKING_CEILING: u64 = 4;

/// The `states` ceiling the resume offers, large enough to close the campaign.
const CLOSING_CEILING: u64 = 64;

/// 3.5 since bn-7xz8v: `whiteboard.compile` is `@since("3.5")`, and a connection below
/// an operation's date is refused it (`OperationSpec::since`) as undeclared, which is not
/// the declared-but-unserved refusal the cold-start test reads. Before that gate this file
/// ran at 3.1 and was served operations 3.1 does not declare. Nothing else it drives
/// changes between 3.1 and 3.5.
fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 5)
}

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("a well-formed capability handle")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("a well-formed actor identity")
}

fn operation_name(operation: &str) -> OperationName {
    OperationName::new(operation).expect("a well-formed operation name")
}

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
}

fn now() -> Timestamp {
    Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp")
}

/// The epochs the deployment serves: all six, plus engine identity.
fn epochs() -> EpochSet {
    EpochSet {
        protocol: version(),
        semantic: Nullable::Value(epoch("semantic-1")),
        intent: Nullable::Value(epoch("intent-1")),
        evidence: Nullable::Value(epoch("evidence-1")),
        proof: Nullable::Value(epoch("proof-1")),
        corpus: Nullable::Value(epoch("corpus-1")),
        engine: Nullable::Value(epoch("engine-reference-1")),
    }
}

fn grant(
    handle: &str,
    actor: &str,
    level: AuthorityLevel,
    depth: u32,
    profile: Optional<CapabilityProfile>,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        capability: cap(handle),
        actor: who(actor),
        level,
        snapshots: Vec::new(),
        intents: Vec::new(),
        artifact_classes: Vec::new(),
        expires_at: Nullable::Null,
        delegation_depth: depth,
        profile,
        instances: Optional::Absent,
    }
}

fn profile(privileged: &[&str]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged
            .iter()
            .map(|entry| operation_name(entry))
            .collect(),
        denied_operations: Vec::new(),
        data_grants: Vec::new(),
        cross_principal_sharing: true,
    }
}

fn negotiated() -> Negotiated {
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: version(),
            high: version(),
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-gate-g2-03-acceptance".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[version()], ProtocolWindow::new(3), ENCODINGS, &hello)
        .expect("the requested version is served")
}

/// A deployment holding the intent, the staged content and the model the workflow needs.
///
/// The three registrations are the out-of-band door F3 names: the protocol declares no
/// operation that stages content, registers an intent proposal, or installs a model, so a
/// deployment does them through [`Daemon::state_mut`], exactly as `daemon_task_operations.rs`
/// does. Nothing the *agent* does below reaches that door.
fn deployment() -> Daemon {
    let root = Some(cap("cap_root"));
    let mut daemon = Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .epochs(epochs())
        .now(now())
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Present(profile(&["intent.accept", "intent.reject", "intent.lock"])),
            ),
            None,
        )
        .capability(
            grant(
                "cap_agent",
                "agent:worker",
                AuthorityLevel::Promote,
                3,
                Optional::Present(profile(&["intent.accept"])),
            ),
            root,
        )
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(TaskFamily)
        .family(VerificationFamily)
        .family(EvidenceFamily::default())
        .build();

    daemon.state_mut().put_intent(
        intent_handle(),
        IntentRecord {
            contract: contract(),
            status: RegistryStatus::Proposed,
            supersedes: None,
            superseded_by: None,
            acceptance: None,
        },
    );
    for (path, content) in staged_files() {
        daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new(path).expect("a workspace path"),
                content.as_bytes().to_vec(),
            )
            .expect("staging names its content");
    }
    daemon.state_mut().models_mut().register(
        model_source(&Blake3Identity, [(MODULE_PATH, DIE_HARD_MODEL.as_bytes())])
            .expect("blake3 names the module set"),
        diehard::model().expect("the TV-009 port builds"),
    );
    daemon
}

/// The `(path, content)` records the deployment stages, in the order the snapshot names them.
fn staged_files() -> [(&'static str, &'static str); 3] {
    [
        (MODULE_PATH, DIE_HARD_MODEL),
        ("README.md", README),
        ("default.model.toml", DIE_HARD_CONFIG),
    ]
}

fn contract() -> IntentContract {
    IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes()).expect("the fixture decodes")
}

/// The intent handle, computed **client-side** from the contract bytes the agent holds.
///
/// F2's first row. No response in this workflow returns this value as a fact the agent did not
/// already have; it is recovered by hashing, through the same public seam a client has, which
/// is what makes it a *derivable* fact rather than a memory.
fn intent_handle() -> IntentHandle {
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        continuum_workspace::artifact_path::ArtifactClass::IntentContract,
        &contract().identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    identity::intent_to_wire(&stored).expect("an `in_` handle")
}

/// The three staged commitments, computed **client-side** from the same bytes.
///
/// F2's other three rows. [`DaemonState::commit_of`] is the daemon's own naming function and is
/// public precisely so "the same content has the same name" is checkable from outside; here it
/// is used the other way round, as the agent's recovery route to an input nothing echoes back.
fn derived_commitments() -> Vec<Commitment> {
    staged_files()
        .into_iter()
        .map(|(path, content)| {
            DaemonState::commit_of(
                &Blake3Identity,
                &WorkspacePath::new(path).expect("a workspace path"),
                content.as_bytes(),
            )
            .expect("blake3 names the record")
        })
        .collect()
}

/// The acceptance record the first step carries.
fn acceptance_bytes() -> Opaque {
    let mut fields: BTreeMap<String, ContractJson> = BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "agent:worker"),
        ("capability", "revise-intent"),
        ("signature", "sig-die-hard-v1"),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", "2026-08-01T00:00:00.000Z"),
    ] {
        fields.insert(key.to_owned(), ContractJson::String(value.to_owned()));
    }
    Opaque::from_bytes(ContractJson::Object(fields).to_canonical_bytes())
}

fn budget(states: u64) -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(states),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes: Optional::Absent,
    }
}

/// A request envelope for `operation`, keyed exactly where the IDL requires a key.
///
/// The key is a function of the request id and the request id is a function of the step, so
/// both are the agent's *program* — recomputed after amnesia rather than remembered. That is
/// the assumption the replay probe rests on, and section 7 states it where it is used.
fn envelope(operation: &str, request: &str) -> RequestEnvelope {
    let mutation = registry::operation(operation)
        .expect("the operation is declared")
        .has(Annotation::Mutation);
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request).expect("a well-formed request id"),
        // `rule errors.common`: a `@mutation` without a key is `MalformedRequest`, and so is a
        // `@readonly` *with* one. Reading the annotation is how this file avoids a table that
        // could drift from the registry.
        idempotency_key: if mutation {
            Optional::Present(format!("idem-{request}"))
        } else {
            Optional::Absent
        },
        actor: who("agent:worker"),
        capability: cap("cap_agent"),
        operation: operation_name(operation),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: Optional::Absent,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

// =========================================================================================
// 1. The wire view, and the mechanical harvest
// =========================================================================================

/// One response as a client sees it: a single canonical JSON document.
///
/// The daemon layer emits no bytes — `ResultEnvelope.payload` reads null there and the typed
/// response travels beside it — so the wire view is assembled the way
/// `transport::Server::answer` assembles it: encode the envelope, encode the payload, splice
/// the second into the first's `payload` field. Every probe below reads *this*, never the Rust
/// values, so none of them can see a fact the wire does not carry.
fn wire_of_response(outcome: &OperationOutcome) -> Json {
    let bytes = codec::to_bytes(&outcome.envelope).expect("the envelope encodes");
    let mut document = Json::parse(&bytes).expect("canonical JSON");
    let encoded = codec::operations::encode_payload(&outcome.payload).expect("the payload encodes");
    if let (Json::Object(map), Some(opaque)) = (&mut document, encoded) {
        map.insert(
            "payload".to_owned(),
            Json::parse(opaque.as_bytes()).expect("the payload is canonical JSON"),
        );
    }
    document
}

/// The same for a request, assembled the way `transport::encode_request` assembles it: the
/// typed arguments encoded into the envelope's `arguments`, then the whole envelope encoded.
fn wire_of_request(request: &OperationRequest) -> Json {
    let mut envelope = request.envelope.clone();
    envelope.arguments =
        transport::encode_arguments(&request.arguments).expect("the arguments encode");
    let bytes = codec::to_bytes(&envelope).expect("the envelope encodes");
    Json::parse(&bytes).expect("canonical JSON")
}

/// Every string leaf of a document, with its JSON path, in canonical order.
fn string_leaves(document: &Json) -> Vec<(String, String)> {
    fn walk(node: &Json, path: &str, out: &mut Vec<(String, String)>) {
        match node {
            Json::String(text) => out.push((path.to_owned(), text.clone())),
            Json::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    walk(item, &format!("{path}[{index}]"), out);
                }
            }
            Json::Object(fields) => {
                for (key, value) in fields {
                    let child = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    walk(value, &child, out);
                }
            }
            Json::Null | Json::Bool(_) | Json::Integer(_) => {}
        }
    }
    let mut out = Vec::new();
    walk(document, "", &mut out);
    out
}

/// Every wire spelling of every member of every enum the registry declares.
///
/// Read from `registry::ENUMS` rather than listed, so a vocabulary that grows cannot silently
/// start being classified as an agent's memory. F5 is why this set is consulted first.
fn vocabulary() -> BTreeSet<&'static str> {
    ENUMS
        .iter()
        .flat_map(|declared| declared.members.iter().map(|member| member.wire))
        .collect()
}

/// Whether `text` carries a declared artifact-class prefix and satisfies `ArtifactHandle`.
///
/// Both halves come from the protocol: the 19 prefixes from `registry::HANDLES` and the
/// pattern from the shipped constructor. A token that passes is *handle-shaped*; F4 is the
/// reminder that handle-shaped is not the same as dereferenceable.
fn is_handle_shaped(text: &str) -> bool {
    HANDLES
        .iter()
        .any(|declared| text.starts_with(declared.prefix))
        && ArtifactHandle::new(text).is_ok()
}

/// One response the agent received, in wire form.
#[derive(Debug, Clone)]
struct Received {
    /// The step of [`PLAN`] that produced it.
    step: usize,
    /// The operation that was called.
    operation: &'static str,
    /// The response, as one canonical JSON document.
    wire: Json,
}

/// Everything the agent has been told, and nothing else.
///
/// This is the *whole* of what survives an interruption in the amnesia probe: a list of wire
/// documents. No Rust types, no decoded values, no notes.
#[derive(Debug, Clone, Default)]
struct Ledger {
    /// Responses, in arrival order.
    received: Vec<Received>,
}

impl Ledger {
    /// The value at `path` in the response to the *first* call of `operation`.
    fn at(&self, operation: &str, path: &str) -> Option<String> {
        let entry = self
            .received
            .iter()
            .find(|entry| entry.operation == operation)?;
        string_leaves(&entry.wire)
            .into_iter()
            .find(|(leaf, _)| leaf == path)
            .map(|(_, value)| value)
    }

    /// Whether the agent already called `operation`.
    fn called(&self, operation: &str) -> bool {
        self.received
            .iter()
            .any(|entry| entry.operation == operation)
    }

    /// Every handle-shaped token anywhere in any response the agent holds.
    fn handles(&self) -> BTreeSet<String> {
        self.received
            .iter()
            .flat_map(|entry| string_leaves(&entry.wire))
            .map(|(_, value)| value)
            .filter(|value| is_handle_shaped(value))
            .collect()
    }

    /// How many responses the agent holds.
    fn len(&self) -> usize {
        self.received.len()
    }
}

// =========================================================================================
// 2. The agent: its session, its plan, and the rebuild
// =========================================================================================

/// The agent's working memory between calls.
///
/// Every field is a handle. There is deliberately no cursor, no page token, no "which step am
/// I on" counter and no locally computed index: those are the shapes G2-03 exists to forbid,
/// and a field of that kind here would make the probe prove something weaker than it claims.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Session {
    /// The accepted contract.
    intent: Option<IntentHandle>,
    /// The snapshot `workspace.create` minted.
    base: Option<WorkspaceHandle>,
    /// The snapshot `workspace.fork` minted, which is what the campaign runs on.
    fork: Option<WorkspaceHandle>,
    /// The sealed snapshot's root digest.
    root_digest: Option<Commitment>,
    /// The parked campaign.
    task: Option<TaskHandle>,
    /// Where it resumes from.
    continuation: Option<ContinuationHandle>,
}

/// One field of [`Session`], and the single wire location that carries it.
///
/// The rebuild is a table lookup over this list, so "can the agent recover field X" becomes a
/// question about the *protocol's responses* rather than about how cleverly the rebuild is
/// written. [`negative_control_a_route_to_a_field_no_response_carries_is_reported`] runs the
/// same lookup over a route that names a request field, and it fails.
#[derive(Debug, Clone, Copy)]
struct Route {
    /// The session field this route fills.
    field: &'static str,
    /// The operation whose response carries it.
    operation: &'static str,
    /// The JSON path inside that response.
    path: &'static str,
}

/// The six routes, one per session field.
const ROUTES: &[Route] = &[
    Route {
        field: "intent",
        operation: "intent.accept",
        path: "payload.intent",
    },
    Route {
        field: "base",
        operation: "workspace.create",
        path: "payload.snapshot",
    },
    Route {
        field: "fork",
        operation: "workspace.fork",
        path: "payload.snapshot",
    },
    Route {
        field: "root_digest",
        operation: "workspace.seal",
        path: "payload.root_digest",
    },
    Route {
        field: "task",
        operation: "verification.start",
        path: "task",
    },
    Route {
        field: "continuation",
        operation: "verification.start",
        path: "continuation",
    },
];

/// Rebuild a session from a ledger and a route table, reporting the fields no route reached.
///
/// A field whose route names an operation the agent has not called yet is *not* a failure —
/// the uninterrupted agent did not hold it at that point either, and
/// [`the_rebuilt_session_equals_the_one_the_uninterrupted_agent_held`] is what makes that
/// precise. A field whose route names a location the response does not carry *is* a failure,
/// and it is reported by name.
fn rebuild(ledger: &Ledger, routes: &[Route]) -> (Session, Vec<&'static str>) {
    let mut session = Session::default();
    let mut unreachable = Vec::new();
    for route in routes {
        let found = ledger.at(route.operation, route.path);
        if ledger.called(route.operation) && found.is_none() {
            unreachable.push(route.field);
            continue;
        }
        let Some(text) = found else { continue };
        match route.field {
            "intent" => session.intent = Some(IntentHandle::new(&text).expect("an `in_` handle")),
            "base" => session.base = Some(WorkspaceHandle::new(&text).expect("a `ws_` handle")),
            "fork" => session.fork = Some(WorkspaceHandle::new(&text).expect("a `ws_` handle")),
            "root_digest" => session.root_digest = Some(Commitment::new(&text)),
            "task" => session.task = Some(TaskHandle::new(&text).expect("a `task_` handle")),
            "continuation" => {
                session.continuation =
                    Some(ContinuationHandle::new(&text).expect("a `cont_` handle"));
            }
            other => panic!("no session field named {other}"),
        }
    }
    (session, unreachable)
}

/// The second, independent route to the same facts: one `task.status` read.
///
/// F1. Given nothing but a `task_` handle, the record answers with the snapshot, the intent and
/// the continuation. This is what a real agent would do on re-entry, and it is a *typed read* —
/// `@readonly`, no key, no effect — so the world fingerprint must not move.
fn reorient(daemon: &mut Daemon, task: &TaskHandle, request: &str) -> Json {
    let outcome = daemon.dispatch(&OperationRequest {
        envelope: envelope("task.status", request),
        arguments: Arguments::TaskStatus(TaskStatusRequest { task: task.clone() }),
    });
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "a re-orientation read is served: {:?}",
        outcome.envelope.error
    );
    wire_of_response(&outcome)
}

/// One call the agent's plan makes.
#[derive(Debug, Clone, Copy)]
struct Step {
    /// The wire operation.
    operation: &'static str,
    /// The request id, a function of the step and therefore recomputable.
    request: &'static str,
}

/// The eleven-step workflow.
///
/// Steps 7 and 9 are the same operation twice — before and after the resume — which is what
/// makes [`Ledger::at`]'s "first call" rule visible: the route table reads the *first*
/// `task.status`, and the amnesia probe still has to agree with the control at step 9.
const PLAN: &[Step] = &[
    Step {
        operation: "intent.accept",
        request: "req_01_accept",
    },
    Step {
        operation: "intent.get",
        request: "req_02_get",
    },
    Step {
        operation: "workspace.create",
        request: "req_03_create",
    },
    Step {
        operation: "workspace.fork",
        request: "req_04_fork",
    },
    Step {
        operation: "workspace.seal",
        request: "req_05_seal",
    },
    Step {
        operation: "verification.start",
        request: "req_06_start",
    },
    Step {
        operation: "task.status",
        request: "req_07_status",
    },
    Step {
        operation: "task.resume",
        request: "req_08_resume",
    },
    Step {
        operation: "task.status",
        request: "req_09_status",
    },
    Step {
        operation: "verification.result",
        request: "req_10_result",
    },
    Step {
        operation: "evidence.query",
        request: "req_11_query",
    },
];

/// Build step `index`'s request from the session and the agent's own inputs.
///
/// Every handle comes from `session`. A field the session does not hold is a panic naming that
/// field, which is exactly what an insufficient rebuild would produce — so the amnesia probe
/// cannot silently paper over a missing handle by reaching somewhere else for it.
fn compose(index: usize, session: &Session) -> OperationRequest {
    let step = PLAN[index];
    let mut envelope = envelope(step.operation, step.request);
    let arguments = match step.operation {
        "intent.accept" => Arguments::IntentAccept(IntentAcceptRequest {
            // Derivable: the agent hashes the contract it holds. No response has arrived yet.
            proposal: intent_handle(),
            acceptance: acceptance_bytes(),
            bundle: Optional::Absent,
        }),
        "intent.get" => Arguments::IntentGet(IntentGetRequest {
            intent: session.intent.clone().expect("session.intent"),
        }),
        "workspace.create" => Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: SnapshotComponents {
                files: derived_commitments()[..2].to_vec(),
                cml_modules: Vec::new(),
                rust_extraction: Vec::new(),
                domain_packs: Vec::new(),
                dependencies: Vec::new(),
                epochs: SnapshotEpochs {
                    semantic: epoch("semantic-1"),
                    proof: epoch("proof-1"),
                    toolchain: Optional::Absent,
                },
                intent: session.intent.clone().expect("session.intent"),
                correspondence: Vec::new(),
                proof_environment: Vec::new(),
                configuration: vec![derived_commitments()[2].clone()],
                file_components: Optional::Absent,
            },
            overlay: Optional::Absent,
            seal: Optional::Present(false),
        }),
        "workspace.fork" => Arguments::WorkspaceFork(WorkspaceForkRequest {
            base: session.base.clone().expect("session.base"),
            overlay: Optional::Present(vec![FileOverlay {
                path: "README.md".to_owned(),
                content: FORKED_README.as_bytes().to_vec(),
            }]),
            patches: Optional::Absent,
        }),
        "workspace.seal" => Arguments::WorkspaceSeal(WorkspaceSealRequest {
            snapshot: session.fork.clone().expect("session.fork"),
        }),
        "verification.start" => {
            envelope.snapshot = Nullable::Value(session.fork.clone().expect("session.fork"));
            envelope.budget = Optional::Present(budget(PARKING_CEILING));
            Arguments::VerificationStart(VerificationStartRequest {
                target: Target {
                    kind: TargetKind::AllClaims,
                    id: "DieHard".to_owned(),
                },
                portfolio: Portfolio::Interactive,
                context_policy: Optional::Absent,
                priority_class: Optional::Absent,
            })
        }
        "task.status" => Arguments::TaskStatus(TaskStatusRequest {
            task: session.task.clone().expect("session.task"),
        }),
        "task.resume" => {
            envelope.budget = Optional::Present(budget(CLOSING_CEILING));
            Arguments::TaskResume(TaskResumeRequest {
                continuation: session.continuation.clone().expect("session.continuation"),
                budget: Optional::Present(budget(CLOSING_CEILING)),
            })
        }
        "verification.result" => Arguments::VerificationResult(VerificationResultRequest {
            task: session.task.clone().expect("session.task"),
        }),
        "evidence.query" => Arguments::EvidenceQuery(EvidenceQueryRequest {
            // The whole graph: no roots, no filters, no handle at all.
            query: EvidenceQuery {
                node_kinds: Optional::Absent,
                edge_kinds: Optional::Absent,
                statuses: Optional::Absent,
                claim_id: Optional::Absent,
                roots: Optional::Absent,
                max_depth: Optional::Absent,
            },
        }),
        other => panic!("the plan names {other}, which `compose` does not build"),
    };
    OperationRequest {
        envelope,
        arguments,
    }
}

/// Absorb a response into the session and the ledger, the way a client would.
fn absorb(session: &mut Session, ledger: &mut Ledger, index: usize, outcome: &OperationOutcome) {
    ledger.received.push(Received {
        step: index,
        operation: PLAN[index].operation,
        wire: wire_of_response(outcome),
    });
    match &outcome.payload {
        Payload::IntentAccept(response) => session.intent = Some(response.intent.clone()),
        Payload::WorkspaceCreate(response) => session.base = Some(response.snapshot.clone()),
        Payload::WorkspaceFork(response) => session.fork = Some(response.snapshot.clone()),
        Payload::WorkspaceSeal(response) => {
            session.root_digest = Some(response.root_digest.clone());
        }
        Payload::VerificationStart(_) => {
            session.task = outcome.envelope.task.value().cloned();
            session.continuation = outcome.envelope.continuation.value().cloned();
        }
        _ => {}
    }
}

/// Drive steps `from..to` of the plan, threading `session` and `ledger`.
fn drive(
    daemon: &mut Daemon,
    session: &mut Session,
    ledger: &mut Ledger,
    from: usize,
    to: usize,
) -> Vec<OperationOutcome> {
    let mut outcomes = Vec::new();
    for (index, step) in PLAN.iter().enumerate().take(to).skip(from) {
        let request = compose(index, session);
        let outcome = daemon.dispatch(&request);
        assert!(
            matches!(
                outcome.envelope.status,
                ResultStatus::Ok | ResultStatus::TaskSuspended
            ),
            "step {index} ({}) must be served, got {:?} {:?}",
            step.operation,
            outcome.envelope.status,
            outcome.envelope.error
        );
        absorb(session, ledger, index, &outcome);
        outcomes.push(outcome);
    }
    outcomes
}

/// One uninterrupted run of the whole plan.
struct Run {
    daemon: Daemon,
    session: Session,
    ledger: Ledger,
    outcomes: Vec<OperationOutcome>,
}

fn control_run() -> Run {
    let mut daemon = deployment();
    let mut session = Session::default();
    let mut ledger = Ledger::default();
    let outcomes = drive(&mut daemon, &mut session, &mut ledger, 0, PLAN.len());
    Run {
        daemon,
        session,
        ledger,
        outcomes,
    }
}

// =========================================================================================
// 3. The world fingerprint
// =========================================================================================

/// Every effect the daemon holds, as one comparable value.
///
/// Modelled on `gate_g1_04_acceptance.rs`'s `Trace` and excluding the same three surfaces for
/// the same reason: the admission log, the store's audit log and the idempotency ledger all
/// move on *every* call including a `@readonly` one. The property under test is that a
/// re-orientation read changes nothing an agent or a later run can observe, not that it is
/// unaudited — and [`the_reorientation_read_is_audited_even_though_it_moves_nothing`] asserts
/// the other direction so "outside the fingerprint" does not become "unchecked".
#[derive(Debug, Clone, PartialEq, Eq)]
struct World {
    store: Vec<(String, Vec<u8>)>,
    tasks: Vec<String>,
    continuations: Vec<String>,
    workspaces: Vec<String>,
    lineages: Vec<String>,
    regions_opened: u32,
    finalizations: Vec<String>,
    region_defects: Vec<String>,
    evidence_nodes: Vec<String>,
    evidence_edges: Vec<String>,
    evidence_events: Vec<String>,
    intents: Vec<String>,
    staged: Vec<String>,
}

/// The handles a fingerprint dereferences, gathered from the runs themselves.
///
/// The daemon iterates tasks, intents and evidence totally; workspaces, lineages,
/// continuations and staged content are read by handle. Taking the *union* of every run's
/// harvest is what stops the instrument being self-serving: a snapshot only the interrupted run
/// minted is still read.
#[derive(Debug, Clone, Default)]
struct Reach {
    workspaces: BTreeSet<String>,
    continuations: BTreeSet<String>,
    commitments: BTreeSet<String>,
}

impl Reach {
    fn absorb(&mut self, ledger: &Ledger) {
        for handle in ledger.handles() {
            if handle.starts_with("ws_") {
                self.workspaces.insert(handle.clone());
            }
            if handle.starts_with("cont_") {
                self.continuations.insert(handle);
            }
        }
        for commitment in derived_commitments() {
            self.commitments.insert(commitment.as_str().to_owned());
        }
    }
}

fn world(daemon: &Daemon, reach: &Reach) -> World {
    let token = identity::capability_to_store(&cap("cap_root")).expect("`cap_root` is a token");
    let mut store: Vec<(String, Vec<u8>)> = daemon
        .store()
        .audit_view(&token)
        .expect("`cap_root` confers audit")
        .identities()
        .into_iter()
        .map(|handle| {
            let bytes = daemon
                .store()
                .read(&handle, &token)
                .expect("`cap_root` confers read");
            (handle.to_string(), bytes)
        })
        .collect();
    store.sort();

    let state = daemon.state();
    let tasks = state
        .tasks()
        .handles()
        .into_iter()
        .map(|handle| format!("{handle:?} => {:?}", state.tasks().get(handle)))
        .collect();
    let continuations = reach
        .continuations
        .iter()
        .map(|text| {
            let handle = ContinuationHandle::new(text).expect("a `cont_` handle");
            format!("{text} => {:?}", state.tasks().continuation(&handle))
        })
        .collect();

    let mut lineage_names: BTreeSet<ForkName> = BTreeSet::new();
    let workspaces = reach
        .workspaces
        .iter()
        .map(|text| {
            let handle = WorkspaceHandle::new(text).expect("a `ws_` handle");
            let record = state.workspace(&handle);
            if let Some(record) = record {
                lineage_names.insert(record.lineage.clone());
            }
            format!("{text} => {record:?}")
        })
        .collect();
    let lineages = lineage_names
        .iter()
        .map(|name| format!("{name:?} => {:?}", state.lineage(name)))
        .collect();

    World {
        store,
        tasks,
        continuations,
        workspaces,
        lineages,
        regions_opened: state.regions().opened(),
        finalizations: state
            .regions()
            .finalizations()
            .iter()
            .map(|entry| format!("{entry:?}"))
            .collect(),
        region_defects: state
            .regions()
            .defects()
            .iter()
            .map(|entry| format!("{entry:?}"))
            .collect(),
        evidence_nodes: state
            .evidence_nodes()
            .map(|(handle, node)| format!("{handle:?} => {node:?}"))
            .collect(),
        evidence_edges: state
            .evidence_edges()
            .map(|(handle, edge)| format!("{handle:?} => {edge:?}"))
            .collect(),
        evidence_events: state
            .evidence_events()
            .iter()
            .map(|event| format!("{event:?}"))
            .collect(),
        intents: state
            .intents()
            .map(|(handle, record)| format!("{handle:?} => {record:?}"))
            .collect(),
        staged: reach
            .commitments
            .iter()
            .map(|text| {
                let commitment = Commitment::new(text);
                format!("{text} => {:?}", state.staged(&commitment))
            })
            .collect(),
    }
}

// =========================================================================================
// 4. The control run
// =========================================================================================

#[test]
fn the_control_run_completes_the_whole_workflow() {
    let run = control_run();

    // The shape everything below measures against, pinned so a change to the workflow cannot
    // quietly weaken every probe at once.
    assert_eq!(run.outcomes.len(), 11);
    assert_eq!(run.ledger.len(), 11);
    for (index, entry) in run.ledger.received.iter().enumerate() {
        assert_eq!(entry.step, index, "the ledger is in plan order");
        assert_eq!(entry.operation, PLAN[index].operation);
    }
    assert_eq!(
        run.outcomes[5].envelope.status,
        ResultStatus::TaskSuspended,
        "a Die Hard campaign bounded at {PARKING_CEILING} states parks"
    );
    assert!(
        run.outcomes[5].envelope.continuation.value().is_some(),
        "a parked campaign names its continuation on the envelope"
    );

    // The session the uninterrupted agent ends with: six handles, all present.
    let session = &run.session;
    assert!(session.intent.is_some(), "intent");
    assert!(session.base.is_some(), "base");
    assert!(session.fork.is_some(), "fork");
    assert!(session.root_digest.is_some(), "root_digest");
    assert!(session.task.is_some(), "task");
    assert!(session.continuation.is_some(), "continuation");
    assert_ne!(
        session.base, session.fork,
        "the overlay makes the fork a different snapshot, so the workflow really does hold two \
         handles of one class and the routes have to tell them apart"
    );

    // The campaign closed, and the verdict is the one the Die Hard contract predicts.
    let Payload::TaskStatus(record) = &run.outcomes[8].payload else {
        panic!("step 9 answers with a task record");
    };
    assert_eq!(record.status, TaskStatus::Completed);
    assert_eq!(
        run.outcomes[9].envelope.verdict,
        Nullable::Value(Verdict::Semantic(SemanticVerdictValue {
            verdict: SemanticVerdict::Refuted,
            inconclusive_reason: Optional::Absent,
            assurance_class: AssuranceClass::Validated,
        })),
        "the campaign refutes `NotSolved`, at `validated`"
    );
}

// =========================================================================================
// 5. The amnesia probe
// =========================================================================================

/// What one interruption produced.
struct Amnesia {
    /// The session the agent held at the moment it was interrupted.
    held: Session,
    /// The session it rebuilt from the wire alone.
    rebuilt: Session,
    /// The ledger after the resumed run finished.
    ledger: Ledger,
    /// The daemon the resumed run left behind.
    daemon: Daemon,
    /// The handles that run reached.
    reach: Reach,
}

/// Interrupt the agent after step `k`, rebuild it from the wire, and finish the plan.
///
/// The interruption is total on the client side: `session` is dropped and a *new* one is built
/// from `ledger` — the wire documents — plus, once a `task_` handle exists, one typed
/// `task.status` read. The daemon is untouched: the same process, with the same state, which is
/// what makes this the agent's interruption and not the daemon's.
fn amnesia_at(k: usize) -> Amnesia {
    let mut daemon = deployment();
    let mut session = Session::default();
    let mut ledger = Ledger::default();
    drive(&mut daemon, &mut session, &mut ledger, 0, k);

    // --- the interruption ------------------------------------------------------------
    let held = session.clone();
    drop(session);

    let (rebuilt, unreachable) = rebuild(&ledger, ROUTES);
    assert!(
        unreachable.is_empty(),
        "interrupted after step {k}: no response carries {unreachable:?}"
    );
    if let Some(task) = rebuilt.task.clone() {
        // F1: the second route. Everything it answers must agree with the first.
        let record = reorient(&mut daemon, &task, &format!("req_reorient_{k}"));
        let leaves: BTreeMap<String, String> = string_leaves(&record).into_iter().collect();
        assert_eq!(
            leaves.get("payload.task").map(String::as_str),
            Some(task.as_str()),
            "the record names the task it was asked about"
        );
        if let Some(text) = leaves.get("payload.snapshot") {
            let snapshot = WorkspaceHandle::new(text).expect("a `ws_` handle");
            assert_eq!(
                rebuilt.fork.as_ref(),
                Some(&snapshot),
                "the record's snapshot and `workspace.fork`'s must be the same handle"
            );
        }
        if let Some(text) = leaves.get("payload.intent") {
            let intent = IntentHandle::new(text).expect("an `in_` handle");
            assert_eq!(
                rebuilt.intent.as_ref(),
                Some(&intent),
                "the record's intent and `intent.accept`'s must be the same handle"
            );
        }
        if let Some(text) = leaves.get("payload.continuation") {
            let continuation = ContinuationHandle::new(text).expect("a `cont_` handle");
            assert_eq!(
                rebuilt.continuation.as_ref(),
                Some(&continuation),
                "the record's continuation and `verification.start`'s must be the same handle"
            );
        }
    }

    let mut reach = Reach::default();
    reach.absorb(&ledger);

    let mut resumed = rebuilt.clone();
    drive(&mut daemon, &mut resumed, &mut ledger, k, PLAN.len());
    reach.absorb(&ledger);

    Amnesia {
        held,
        rebuilt,
        ledger,
        daemon,
        reach,
    }
}

#[test]
fn amnesia_at_every_interruption_point_lands_in_the_same_world() {
    let control = control_run();
    let mut control_reach = Reach::default();
    control_reach.absorb(&control.ledger);

    for k in 0..=PLAN.len() {
        let probe = amnesia_at(k);
        // Both fingerprints read the union of both harvests, so neither instrument can be
        // blind to a handle the other run produced.
        let mut probe_reach = probe.reach.clone();
        probe_reach.absorb(&control.ledger);
        let mut here = control_reach.clone();
        here.absorb(&probe.ledger);

        assert_eq!(
            world(&probe.daemon, &probe_reach),
            world(&control.daemon, &here),
            "an agent interrupted after step {k} of {} leaves a different world than one that \
             was never interrupted",
            PLAN.len()
        );
    }
}

#[test]
fn the_rebuilt_session_equals_the_one_the_uninterrupted_agent_held() {
    // The probe's core claim, stated on its own so a failure names the field.
    for k in 0..=PLAN.len() {
        let probe = amnesia_at(k);
        assert_eq!(
            probe.held.intent, probe.rebuilt.intent,
            "intent, after an interruption at step {k}"
        );
        assert_eq!(probe.held.base, probe.rebuilt.base, "base, at step {k}");
        assert_eq!(probe.held.fork, probe.rebuilt.fork, "fork, at step {k}");
        assert_eq!(
            probe.held.root_digest, probe.rebuilt.root_digest,
            "root_digest, at step {k}"
        );
        assert_eq!(probe.held.task, probe.rebuilt.task, "task, at step {k}");
        assert_eq!(
            probe.held.continuation, probe.rebuilt.continuation,
            "continuation, at step {k}"
        );
        assert_eq!(probe.held, probe.rebuilt, "the whole session, at step {k}");
    }
}

#[test]
fn the_two_rebuild_routes_agree_at_every_interruption_point() {
    // F1. `amnesia_at` asserts the agreement inline, where it can name the field; this states
    // the coverage — the second route exists from step 6 onward and nowhere earlier, because
    // no `task_` handle exists before `verification.start` answers.
    let mut with_second_route = Vec::new();
    for k in 0..=PLAN.len() {
        if amnesia_at(k).rebuilt.task.is_some() {
            with_second_route.push(k);
        }
    }
    assert_eq!(
        with_second_route,
        vec![6, 7, 8, 9, 10, 11],
        "the interruption points at which a `task.status` re-orientation is possible"
    );
}

#[test]
fn the_reorientation_read_is_audited_even_though_it_moves_nothing() {
    // The fingerprint deliberately excludes the admission log, so this is what keeps
    // "excluded" from meaning "unchecked": the read *is* recorded, and that record is what
    // makes a silent re-orientation impossible.
    let mut daemon = deployment();
    let mut session = Session::default();
    let mut ledger = Ledger::default();
    drive(&mut daemon, &mut session, &mut ledger, 0, 6);
    let task = session
        .task
        .clone()
        .expect("a parked campaign names its task");

    let before = daemon.state().admissions().len();
    reorient(&mut daemon, &task, "req_audit_probe");
    assert_eq!(
        daemon.state().admissions().len(),
        before + 1,
        "a re-orientation read appends exactly one admission record"
    );
}

// =========================================================================================
// 6. The handle-sufficiency census
// =========================================================================================

/// How a token in a request is accounted for.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    /// Returned by a response the agent already held.
    Returned,
    /// Re-derived by the agent from content it holds (F2).
    Derivable,
    /// The connection's own capability: configuration, never a response (F6).
    Capability,
    /// A declared enum member spelling that happens to be handle-shaped (F5).
    Vocabulary,
    /// Accounted for by nothing: a fact that lived only in the agent's memory.
    MemoryOnly,
}

/// One handle-shaped token in one request, and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Accounted {
    /// The step that sent it.
    step: usize,
    /// The JSON path inside the request.
    path: String,
    /// The token.
    token: String,
    /// Its account.
    source: Source,
}

/// The tokens an agent recomputes rather than remembers: F2's four rows.
fn derivable_closure() -> BTreeSet<String> {
    let mut closure = BTreeSet::new();
    closure.insert(intent_handle().as_str().to_owned());
    for commitment in derived_commitments() {
        closure.insert(commitment.as_str().to_owned());
    }
    closure
}

/// Classify one token against a ledger of returned tokens and a derivable closure.
fn account(
    token: &str,
    returned: &BTreeSet<String>,
    closure: &BTreeSet<String>,
    vocabulary: &BTreeSet<&'static str>,
) -> Option<Source> {
    // F5: declared vocabulary is resolved first, because two `ResultStatus` members are
    // handle-shaped and a census that read the prefix first would report the protocol's own
    // words as an agent's memory.
    if vocabulary.contains(token) {
        return is_handle_shaped(token).then_some(Source::Vocabulary);
    }
    if !is_handle_shaped(token) {
        return None;
    }
    Some(if token.starts_with("cap_") {
        Source::Capability
    } else if returned.contains(token) {
        Source::Returned
    } else if closure.contains(token) {
        Source::Derivable
    } else {
        Source::MemoryOnly
    })
}

/// Census the handle-shaped tokens of every request in a run.
///
/// `closure` is a parameter so that
/// [`negative_control_without_the_derivable_closure_four_inputs_are_flagged`] can run the same
/// census with it emptied. `returned` accumulates as the run proceeds, so a token counts as
/// `Returned` only if a response that arrived *before* the request carried it.
fn census(closure: &BTreeSet<String>) -> Vec<Accounted> {
    let vocabulary = vocabulary();
    let mut daemon = deployment();
    let mut session = Session::default();
    let mut ledger = Ledger::default();
    let mut returned: BTreeSet<String> = BTreeSet::new();
    let mut accounted = Vec::new();

    for index in 0..PLAN.len() {
        let request = compose(index, &session);
        for (path, token) in string_leaves(&wire_of_request(&request)) {
            if let Some(source) = account(&token, &returned, closure, &vocabulary) {
                accounted.push(Accounted {
                    step: index,
                    path,
                    token,
                    source,
                });
            }
        }
        let outcome = daemon.dispatch(&request);
        absorb(&mut session, &mut ledger, index, &outcome);
        for (_, value) in string_leaves(&wire_of_response(&outcome)) {
            if is_handle_shaped(&value) {
                returned.insert(value);
            }
        }
    }
    accounted
}

#[test]
fn every_handle_a_request_carries_was_returned_or_is_derivable() {
    let accounted = census(&derivable_closure());
    let unaccounted: Vec<&Accounted> = accounted
        .iter()
        .filter(|entry| entry.source == Source::MemoryOnly)
        .collect();
    assert!(
        unaccounted.is_empty(),
        "these request tokens are accounted for by no response, no derivation and no constant, \
         so the workflow depends on something the agent could only have remembered: \
         {unaccounted:#?}"
    );

    // The census has to have looked at something, so its shape is pinned too.
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in &accounted {
        let key = match entry.source {
            Source::Returned => "returned",
            Source::Derivable => "derivable",
            Source::Capability => "capability",
            Source::Vocabulary => "vocabulary",
            Source::MemoryOnly => "memory-only",
        };
        *counts.entry(key).or_default() += 1;
    }
    assert_eq!(counts.get("memory-only"), None);
    assert_eq!(
        counts["capability"], 11,
        "every one of the eleven requests presents the connection's capability"
    );
    assert_eq!(
        counts["derivable"], 4,
        "the intent handle at `intent.accept`, before any response exists, plus the three \
         staged commitments; the *second* time the intent handle is carried — inside \
         `workspace.create`'s components — it has already been returned, so it resolves \
         `Returned` and the two accounts do not double-count it"
    );
    assert!(
        counts["returned"] >= 6,
        "the workflow really does carry returned handles forward: {counts:?}"
    );
}

#[test]
fn negative_control_the_census_flags_a_handle_no_response_ever_returned() {
    // The method's teeth. A well-formed handle of a real class, naming a snapshot this
    // deployment never minted, must be reported memory-only — and the same token must be
    // reported `Returned` once a response has carried it, so the classification is a function
    // of the ledger rather than of the token's spelling.
    let vocabulary = vocabulary();
    let closure = derivable_closure();
    let forged = "ws_0000000000000000000000000000000000000000000000000000000000000000";
    assert!(
        is_handle_shaped(forged),
        "the forgery is a well-formed `ws_` handle"
    );

    assert_eq!(
        account(forged, &BTreeSet::new(), &closure, &vocabulary),
        Some(Source::MemoryOnly),
        "a handle no response returned and no content derives is memory-only"
    );
    let returned: BTreeSet<String> = [forged.to_owned()].into_iter().collect();
    assert_eq!(
        account(forged, &returned, &closure, &vocabulary),
        Some(Source::Returned),
        "and the very same token, once returned, is accounted for"
    );

    // The real run's own tokens are never memory-only, which is the contrast that makes the
    // control informative rather than decorative.
    let real = census(&closure);
    assert!(real.iter().all(|entry| entry.source != Source::MemoryOnly));
}

#[test]
fn negative_control_without_the_derivable_closure_four_inputs_are_flagged() {
    // F2, measured. Switch the derivation off — model an agent that cannot hash its own
    // content — and the census reports exactly the inputs no response echoes.
    let accounted = census(&BTreeSet::new());
    let flagged: BTreeSet<String> = accounted
        .iter()
        .filter(|entry| entry.source == Source::MemoryOnly)
        .map(|entry| entry.token.clone())
        .collect();
    assert_eq!(
        flagged.len(),
        4,
        "the intent handle and the three staged commitments: {flagged:#?}"
    );
    assert_eq!(
        flagged,
        derivable_closure(),
        "and they are exactly the derivable closure, which is what makes that closure a \
         measurement rather than a list"
    );

    // Where they sit: the intent handle is carried before any response exists at all, and the
    // commitments are carried by `workspace.create`.
    let sites: Vec<(usize, String)> = accounted
        .iter()
        .filter(|entry| entry.source == Source::MemoryOnly)
        .map(|entry| (entry.step, entry.path.clone()))
        .collect();
    assert_eq!(
        sites,
        vec![
            (0, "arguments.proposal".to_owned()),
            (2, "arguments.components.configuration[0]".to_owned()),
            (2, "arguments.components.files[0]".to_owned()),
            (2, "arguments.components.files[1]".to_owned()),
        ],
        "the four sites at which an underivable input would stop the workflow. \
         `arguments.components.intent` is *not* among them: by step 2 `intent.accept` has \
         already returned that handle, so an agent without the hash still holds it"
    );
}

#[test]
fn negative_control_a_route_to_a_field_no_response_carries_is_reported() {
    // The rebuild's own control. `idempotency_key` is on every mutation *request* and on no
    // response, so a route naming it must be reported unreachable — which is how the rebuild
    // would report a session field the protocol genuinely cannot return.
    let control = control_run();
    let broken: Vec<Route> = ROUTES
        .iter()
        .copied()
        .chain([Route {
            field: "intent",
            operation: "workspace.create",
            path: "idempotency_key",
        }])
        .collect();
    let (_, unreachable) = rebuild(&control.ledger, &broken);
    assert_eq!(
        unreachable,
        vec!["intent"],
        "a route into a request field is not a route"
    );
    let (_, none) = rebuild(&control.ledger, ROUTES);
    assert!(none.is_empty(), "and the real table reaches every field");
}

#[test]
fn negative_control_the_world_fingerprint_separates_two_runs_that_differ_by_one_call() {
    // The comparison instrument's own control. Without it, "the interrupted run leaves the same
    // world" could be true of an instrument that reads nothing.
    let control = control_run();
    let mut reach = Reach::default();
    reach.absorb(&control.ledger);
    let before = world(&control.daemon, &reach);

    let mut divergent = deployment();
    let mut session = Session::default();
    let mut ledger = Ledger::default();
    drive(&mut divergent, &mut session, &mut ledger, 0, PLAN.len());
    // One extra mutation: a further fork off the lineage's *head*, with a different overlay.
    // (Forking the original base again is refused `StaleSnapshot` — see
    // `a_re_keyed_resend_is_refused_as_stale_so_the_idempotency_key_is_load_bearing`.)
    let outcome = divergent.dispatch(&OperationRequest {
        envelope: envelope("workspace.fork", "req_divergent_fork"),
        arguments: Arguments::WorkspaceFork(WorkspaceForkRequest {
            base: session.fork.clone().expect("session.fork"),
            overlay: Optional::Present(vec![FileOverlay {
                path: "README.md".to_owned(),
                content: b"# TV-009 (divergent)\n".to_vec(),
            }]),
            patches: Optional::Absent,
        }),
    });
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        outcome.envelope.error
    );
    ledger.received.push(Received {
        step: PLAN.len(),
        operation: "workspace.fork",
        wire: wire_of_response(&outcome),
    });
    let mut divergent_reach = reach.clone();
    divergent_reach.absorb(&ledger);

    assert_ne!(
        world(&divergent, &divergent_reach),
        before,
        "the fingerprint must separate a run that made one more mutation"
    );
}

// =========================================================================================
// 7. Interrupted mid-request: the agent that does not know whether its call landed
// =========================================================================================

/// The steps of the plan the IDL annotates `@mutation`, read from the registry.
fn mutation_steps() -> Vec<usize> {
    (0..PLAN.len())
        .filter(|index| {
            registry::operation(PLAN[*index].operation)
                .expect("declared")
                .has(Annotation::Mutation)
        })
        .collect()
}

#[test]
fn a_lost_response_is_recovered_by_replaying_the_identical_request() {
    // The agent crashed between send and receive. It does not know whether the mutation landed,
    // and no operation tells it. What it *can* do is send the same bytes again — which it can
    // construct, because the request id and the idempotency key are functions of the step in
    // its plan rather than values it drew at random. An agent that keyed randomly could not,
    // and that is the one client-side obligation this probe rests on.
    //
    // `gate_g1_03_acceptance.rs` proved the ledger honest under a *fresh* `request_id`. The
    // case an interrupted agent is actually in is the identical resend, and this is it.
    let control = control_run();
    let mut base_reach = Reach::default();
    base_reach.absorb(&control.ledger);
    let steps = mutation_steps();
    assert_eq!(
        steps,
        vec![0, 2, 3, 4, 5, 7],
        "the six mutations of the eleven-step plan"
    );

    for index in steps {
        let mut daemon = deployment();
        let mut session = Session::default();
        let mut ledger = Ledger::default();
        drive(&mut daemon, &mut session, &mut ledger, 0, PLAN.len());
        let mut reach = base_reach.clone();
        reach.absorb(&ledger);
        let before = world(&daemon, &reach);

        // The agent rebuilds the same request from its ledger and its plan, and resends it.
        let (rebuilt, unreachable) = rebuild(&ledger, ROUTES);
        assert!(unreachable.is_empty());
        let replayed = daemon.dispatch(&compose(index, &rebuilt));

        assert_eq!(
            wire_of_response(&replayed).to_canonical_bytes(),
            wire_of_response(&control.outcomes[index]).to_canonical_bytes(),
            "resending step {index} ({}) verbatim must answer with the same bytes, or an agent \
             that does not know whether the first landed cannot safely try again",
            PLAN[index].operation
        );

        let mut after = reach.clone();
        after.absorb(&Ledger {
            received: vec![Received {
                step: index,
                operation: PLAN[index].operation,
                wire: wire_of_response(&replayed),
            }],
        });
        assert_eq!(world(&daemon, &after), before, "and it must move nothing");
    }
}

#[test]
fn a_re_keyed_resend_is_refused_as_stale_so_the_idempotency_key_is_load_bearing() {
    // F7's second half, and the honest boundary of the first. The replay above works because
    // the idempotency ledger returns the recorded outcome *without re-evaluating the request*.
    // Take the key away — model an agent that drew a random one and lost it — and the very same
    // `workspace.fork` is refused `StaleSnapshot`, because its own first call already superseded
    // the base in that lineage.
    //
    // So the key is not a convenience here. "Resend safely without knowing whether the first
    // landed" holds exactly to the extent that the agent can *reconstruct its key*, which makes
    // deterministic keying a client obligation this criterion depends on and the protocol does
    // not enforce. That is stated in the header as the one client-side assumption of section 7.
    let control = control_run();
    let mut reach = Reach::default();
    reach.absorb(&control.ledger);

    let mut daemon = deployment();
    let mut session = Session::default();
    let mut ledger = Ledger::default();
    drive(&mut daemon, &mut session, &mut ledger, 0, PLAN.len());
    reach.absorb(&ledger);
    let before = world(&daemon, &reach);

    // The same call, re-keyed and re-identified.
    let mut request = compose(3, &session);
    request.envelope.request_id = RequestId::new("req_04_fork_again").expect("a request id");
    request.envelope.idempotency_key = Optional::Present("idem-req_04_fork_again".to_owned());
    let outcome = daemon.dispatch(&request);
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Error,
        "a re-keyed resend is a fresh call, and this one no longer applies"
    );
    let error = outcome
        .envelope
        .error
        .value()
        .expect("an error result carries its error");
    assert_eq!(
        error.code,
        ErrorCode::StaleSnapshot,
        "the base was superseded by this agent's own first fork"
    );
    assert!(
        !error.retryable,
        "and an identical retry cannot fix it: the lineage has moved"
    );
    assert_eq!(
        outcome.payload,
        Payload::None,
        "a refusal carries no payload"
    );

    // The refusal leaves no effect, which is what makes the failed resend survivable.
    let mut after = reach.clone();
    after.absorb(&Ledger {
        received: vec![Received {
            step: 3,
            operation: "workspace.fork",
            wire: wire_of_response(&outcome),
        }],
    });
    assert_eq!(
        world(&daemon, &after),
        before,
        "a refused resend moves nothing"
    );

    // And the contrast, in the same world: the *keyed* resend of that same call is served from
    // the ledger and answers with the original bytes.
    let keyed = daemon.dispatch(&compose(3, &session));
    assert_eq!(
        wire_of_response(&keyed).to_canonical_bytes(),
        wire_of_response(&control.outcomes[3]).to_canonical_bytes(),
        "the key is what turns a resend into a replay"
    );
}

// =========================================================================================
// 8. Re-entry from nothing: the scope statement, measured
// =========================================================================================

/// Resolve an IDL type spelling to the struct it names, if it names one.
fn named_struct(ty: &str) -> Option<&'static StructSpec> {
    NAMED_STRUCTS.iter().find(|declared| declared.name == ty)
}

/// Strip the IDL's container spellings from a type, leaving the element type.
fn element(ty: &str) -> &str {
    if let Some(inner) = ty.strip_prefix("list<").and_then(|t| t.strip_suffix('>')) {
        return inner.trim();
    }
    if let Some(inner) = ty
        .strip_prefix("map<String,")
        .and_then(|t| t.strip_suffix('>'))
    {
        return inner.trim();
    }
    ty
}

/// Whether a type spelling is a handle class or the class-agnostic `ArtifactHandle`.
fn is_handle_type(ty: &str) -> bool {
    ty == "ArtifactHandle" || HANDLES.iter().any(|declared| declared.name == ty)
}

/// Whether a request body has a `required` field that resolves to a handle, transitively.
fn requires_a_handle(fields: &[FieldSpec], depth: u32) -> bool {
    if depth == 0 {
        return false;
    }
    fields.iter().any(|field| {
        if field.presence != Presence::Required {
            return false;
        }
        let ty = element(field.ty);
        is_handle_type(ty)
            || named_struct(ty).is_some_and(|nested| requires_a_handle(nested.fields, depth - 1))
    })
}

#[test]
fn the_protocol_declares_no_enumeration_over_the_workflow_classes() {
    // The scope statement, computed rather than asserted. An operation whose request body has
    // no required handle is one a cold-started agent could call; every other operation needs a
    // handle it no longer has.
    let handle_free: Vec<&str> = OPERATIONS
        .iter()
        .filter(|spec| !requires_a_handle(spec.request.fields, 8))
        .map(|spec| spec.name)
        .collect();
    assert_eq!(
        handle_free,
        [
            "intent.import_bundle",
            "verification.start",
            "program.extract",
            "program.run",
            "proof.goal",
            "correspondence.bind",
            "correspondence.status",
            "observe.ingest",
            "forge.create",
            "benchmark.run",
            "evidence.query",
            "evidence.subscribe",
            "whiteboard.compile",
            "signing.mint",
            "signing.rotate",
            "signing.revoke",
            "signing.registry",
            "signing.verify",
            "signing.sign_pack",
        ],
        "the cold-start surface at the *request-body* level, out of 83 operations"
    );
    assert_eq!(OPERATIONS.len(), 83);
    // Protocol 3.8 (bn-3glnv) added seven handle-free requests. None enumerates a workflow
    // class: `intent.import_bundle` supplies its own bundle, five `signing` operations name
    // a signer, a kind, a pack, or an artifact the caller supplies, and `signing.registry`
    // lists signers — deployment identities, not tasks, workspaces, intents, or
    // continuations. All seven need an unscoped grant (`rule signing.identities`).

    // None of the twelve *enumerates* the classes a mid-workflow agent needs to find again.
    // Eleven of them create or supply their own subject — `verification.start` takes the
    // snapshot from the envelope, `program.extract`, `proof.goal`, `correspondence.bind`,
    // `observe.ingest`, `forge.create`, `benchmark.run` and `whiteboard.compile` all take a
    // body the caller wrote — and the twelfth, `evidence.query`, walks the evidence graph and
    // nothing else. There is no task, workspace, intent or continuation enumeration anywhere in
    // the 75, so no operation exists that would answer "which campaigns am I running".
    let names: BTreeSet<&str> = OPERATIONS.iter().map(|spec| spec.name).collect();
    for absent in [
        "task.list",
        "workspace.list",
        "intent.list",
        "continuation.list",
        "task.query",
        "workspace.query",
    ] {
        assert!(!names.contains(absent), "{absent} is not declared");
    }
}

#[test]
fn re_entry_from_nothing_reaches_one_servable_operation() {
    // The measurement behind the narrowing. An agent that lost its ledger as well as its
    // session keeps only its capability. Of the twelve operations whose request body needs no
    // handle, exactly one both is served here and completes without an envelope subject — and
    // against this workflow's world it answers empty, so a total amnesiac learns nothing about
    // the campaign it was running.
    let run = control_run();
    let mut daemon = run.daemon;

    let outcome = daemon.dispatch(&OperationRequest {
        envelope: envelope("evidence.query", "req_cold_start"),
        arguments: Arguments::EvidenceQuery(EvidenceQueryRequest {
            query: EvidenceQuery {
                node_kinds: Optional::Absent,
                edge_kinds: Optional::Absent,
                statuses: Optional::Absent,
                claim_id: Optional::Absent,
                roots: Optional::Absent,
                max_depth: Optional::Absent,
            },
        }),
    });
    assert_eq!(outcome.envelope.status, ResultStatus::Ok);
    let Payload::EvidenceQuery(response) = &outcome.payload else {
        panic!("evidence.query answers with its own payload");
    };
    assert!(
        response.nodes.is_empty() && response.edges.is_empty(),
        "nothing in this build writes an evidence node, so the one cold-start read there is \
         returns an empty graph: {response:?}"
    );

    // And the workflow's own results say so in the typed way, rather than by being silent.
    let omissions: BTreeSet<String> = run
        .outcomes
        .iter()
        .flat_map(|outcome| outcome.envelope.omissions.iter())
        .map(|omission| omission.subject.clone())
        .collect();
    assert!(
        omissions.contains("task.committed_evidence"),
        "every task result carries the omission that explains the empty graph: {omissions:?}"
    );

    // The body-level census of `the_protocol_declares_no_enumeration_over_the_workflow_classes`
    // names twelve operations whose *request body* requires no handle. It over-counts the
    // cold-start surface twice over, and both over-counts are measured here rather than
    // argued.
    //
    // First, an operation can require a handle in the *envelope* rather than in its body:
    // `verification.start` is the workflow's own step 6 and it is in the twelve, yet with a
    // null `snapshot` it does not run.
    let without_a_snapshot = daemon.dispatch(&OperationRequest {
        envelope: envelope("verification.start", "req_cold_start_verify"),
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target: Target {
                kind: TargetKind::AllClaims,
                id: "DieHard".to_owned(),
            },
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    });
    assert_eq!(
        without_a_snapshot.envelope.status,
        ResultStatus::Error,
        "a handle-free request body is not a handle-free call"
    );
    assert_eq!(
        without_a_snapshot
            .envelope
            .error
            .value()
            .expect("an error result carries its error")
            .code,
        ErrorCode::MalformedRequest,
        "and the refusal is typed rather than a silent no-op"
    );

    // Second, an operation can be declared and unserved. `whiteboard.compile` is the other
    // handle-free *read-shaped* door and no `WhiteboardFamily` is registered here, so it
    // answers `UnsupportedSemanticFeature` — which leaves `evidence.query` as the single
    // operation a total amnesiac can actually complete against this deployment.
    let unserved = daemon.dispatch(&OperationRequest {
        envelope: envelope("whiteboard.compile", "req_cold_start_whiteboard"),
        arguments: Arguments::WhiteboardCompile(WhiteboardCompileRequest {
            note: Opaque::from_bytes(b"{}".to_vec()),
        }),
    });
    assert_eq!(unserved.envelope.status, ResultStatus::Error);
    assert_eq!(
        unserved
            .envelope
            .error
            .value()
            .expect("an error result carries its error")
            .code,
        ErrorCode::UnsupportedSemanticFeature,
        "an unserved operation says so in the typed way (INV-008)"
    );
}

#[test]
fn the_declared_discovery_surface_is_empty_on_every_result() {
    // `next_operations` is `required` on every `ResultEnvelope`, and RFC 0026 calls it "the safe
    // recovery and discovery surface". `daemon/result.rs` emits `[]` and says why. This measures
    // it across the whole workflow, so the narrowing in the header is a count and not an
    // impression.
    let run = control_run();
    for (index, outcome) in run.outcomes.iter().enumerate() {
        assert!(
            outcome.envelope.next_operations.is_empty(),
            "step {index} ({}) offers a next operation; the header's narrowing is stale",
            PLAN[index].operation
        );
        assert!(
            outcome.envelope.next_page_token.is_absent(),
            "step {index} mints a page token; a server-side cursor the agent must keep is \
             exactly what this criterion forbids"
        );
    }

    // `evidence.query` is `@paginated`, so the absence above is a fact about this daemon rather
    // than about the operation's declaration.
    assert!(
        registry::operation("evidence.query")
            .expect("declared")
            .has(Annotation::Paginated),
        "evidence.query is the workflow's paginated step"
    );
}

// =========================================================================================
// 9. What the measurement had to be careful about
// =========================================================================================

#[test]
fn a_class_prefix_does_not_make_a_token_a_handle() {
    // F4. Two families of `Commitment`-typed values in this workflow are spelled with an
    // artifact class prefix, so a census matching on prefix alone would treat a content digest
    // as a dereferenceable handle. The census does exactly that, deliberately: counting a
    // non-handle as a handle can only *add* an obligation, never drop one. This records which
    // tokens they are, so the over-count is a known quantity.
    let run = control_run();
    let seal: BTreeMap<String, String> = string_leaves(&run.ledger.received[4].wire)
        .into_iter()
        .collect();
    let root_digest = seal
        .get("payload.root_digest")
        .expect("workspace.seal names a root digest");
    assert!(
        root_digest.starts_with("ws_") && is_handle_shaped(root_digest),
        "the root digest is spelled like a workspace handle: {root_digest}"
    );
    assert_ne!(
        Some(root_digest),
        seal.get("payload.snapshot"),
        "and it is not the snapshot handle beside it"
    );

    // The other family: every `ArtifactRef.commitment` the task steps return.
    let commitments: BTreeSet<String> = run
        .ledger
        .received
        .iter()
        .flat_map(|entry| string_leaves(&entry.wire))
        .filter(|(path, _)| path.contains("artifacts[") && path.ends_with(".commitment"))
        .map(|(_, value)| value)
        .collect();
    assert_eq!(commitments.len(), 2, "two published campaign records");
    for commitment in &commitments {
        assert!(
            commitment.starts_with("task_") && is_handle_shaped(commitment),
            "a publication commitment is spelled like a task handle: {commitment}"
        );
    }
}

#[test]
fn the_vocabulary_collision_between_result_status_and_the_task_class_is_real() {
    // F5, pinned. If this ever stops being true, the census's resolution order becomes an
    // unexplained implementation detail rather than a documented necessity.
    let colliding: Vec<&str> = vocabulary()
        .into_iter()
        .filter(|word| is_handle_shaped(word))
        .collect();
    assert_eq!(
        colliding,
        [
            "defect_mutants",
            "proof_dependency",
            "proof_goal",
            "receipt_generation",
            "task_started",
            "task_suspended",
        ],
        "the declared enum spellings that satisfy the artifact-handle pattern under a declared \
         class prefix"
    );
    assert!(
        ResultStatus::MEMBERS
            .iter()
            .any(|member| member.wire == "task_suspended"),
        "and they are `ResultStatus` members, which this workflow really does return"
    );
}

#[test]
fn the_capability_token_reaches_no_response() {
    // F6. RFC 0026 forbids a `cap_*` from results, error text and `next_operations` arguments.
    // Over eleven responses, measured on the wire rather than trusted.
    let run = control_run();
    for (index, entry) in run.ledger.received.iter().enumerate() {
        for (path, value) in string_leaves(&entry.wire) {
            assert!(
                !value.starts_with("cap_"),
                "step {index} ({}) returns a capability at {path}",
                entry.operation
            );
        }
    }
    // Which is why the census resolves `cap_agent` as configuration: every request carries one
    // and no response could ever return it.
    let leaves: BTreeMap<String, String> =
        string_leaves(&wire_of_request(&compose(0, &Session::default())))
            .into_iter()
            .collect();
    assert_eq!(
        leaves.get("capability").map(String::as_str),
        Some("cap_agent"),
        "the request carries it in the clear"
    );
}

/// The non-handle strings the agent's *program* supplies, and why each is program text.
///
/// The census in section 6 reads handle-shaped tokens. This table accounts for everything else
/// a request carries, so the claim "no step needs a fact the agent could only have remembered"
/// covers the whole request rather than only its handles. Every row is a literal in this file
/// or a spelling the protocol fixes.
const PROGRAM_CONSTANTS: &[(&str, &str)] = &[
    (
        "3.5",
        "the negotiated protocol version, fixed by the connection",
    ),
    ("agent:worker", "the actor the connection was opened as"),
    ("DieHard", "the target the agent was asked to verify"),
    ("README.md", "a path in the agent's own source tree"),
    (
        "semantic-1",
        "the epoch the agent declares its snapshot under",
    ),
    ("proof-1", "likewise"),
    (
        "revise-intent",
        "the capability requirement the contract itself names",
    ),
    (
        "sig-die-hard-v1",
        "the signature the agent attaches to its acceptance record",
    ),
    (
        "supplied-by-the-caller-and-overwritten",
        "the acceptance record's audit placeholder, which the daemon overwrites",
    ),
    (
        "2026-08-01T00:00:00.000Z",
        "the timestamp the agent stamps its acceptance with",
    ),
];

#[test]
fn every_non_handle_string_in_every_request_is_program_text_or_protocol_vocabulary() {
    // The wider census. A string in a request that is neither a handle, nor a declared enum
    // spelling, nor a value this file's own program produces would be "you had to be there"
    // context — the shape G2-03 exists to exclude.
    let vocabulary = vocabulary();
    let closure = derivable_closure();
    let mut program: BTreeSet<String> = PROGRAM_CONSTANTS
        .iter()
        .map(|(text, _)| (*text).to_owned())
        .collect();
    // The rest of the program: the plan's own request ids and their derived keys, the operation
    // names, and the byte blobs the agent authored.
    for step in PLAN {
        program.insert(step.operation.to_owned());
        program.insert(step.request.to_owned());
        program.insert(format!("idem-{}", step.request));
    }
    // `FileOverlay.content` is `Bytes`, which JSON encodes base64url; the acceptance record is
    // `Opaque`, which JSON carries verbatim, so its own strings are leaves and are in the table.
    program.insert(base64url(FORKED_README.as_bytes()));

    let mut daemon = deployment();
    let mut session = Session::default();
    let mut ledger = Ledger::default();
    let mut returned: BTreeSet<String> = BTreeSet::new();
    let mut foreign = Vec::new();

    for index in 0..PLAN.len() {
        let request = compose(index, &session);
        for (path, token) in string_leaves(&wire_of_request(&request)) {
            let known = vocabulary.contains(token.as_str())
                || program.contains(&token)
                || closure.contains(&token)
                || returned.contains(&token)
                || token.starts_with("cap_");
            if !known {
                foreign.push((index, path, token));
            }
        }
        let outcome = daemon.dispatch(&request);
        absorb(&mut session, &mut ledger, index, &outcome);
        for (_, value) in string_leaves(&wire_of_response(&outcome)) {
            returned.insert(value);
        }
    }
    assert!(
        foreign.is_empty(),
        "these request strings came from neither the protocol's vocabulary, the agent's program, \
         its derivable content, nor a response it held: {foreign:#?}"
    );
    assert_eq!(
        PROGRAM_CONSTANTS.len(),
        10,
        "the declared table, which is small on purpose"
    );
}
