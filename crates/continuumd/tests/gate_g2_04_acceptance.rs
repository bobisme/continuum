//! Acceptance re-derivation for release gate **G2-04** (bone `bn-3r10g`).
//!
//! > stale state rejected
//! >
//! > — `notes/plan/docs/52_RELEASE_GATES_REV3.md:33`, G2 bullet 4
//!
//! # Why this file exists beside evidence that already passes
//!
//! `notes/plan/notes/PHASE_A_EXIT_PACKAGE.md:786` records G2-04 as "evidenced —
//! `dx03_falsification.rs`'s stale-snapshot axis over **every** snapshot-consuming
//! operation". That claim rests on one test,
//! `attack_staleness_every_snapshot_consuming_operation_refuses_a_superseded_handle`, and
//! the exit package states its own limit: it re-ran the *delivering* suites rather than
//! re-deriving the criterion. Three gaps follow from that, and this file attacks each.
//!
//! **First, "every snapshot-consuming operation" is a hand-written list of five.** The
//! delivering test drives `verification.start`, `task.resume`, `workspace.fork`,
//! `workspace.seal` and `workspace.diff`. Nothing in it fails if a sixth carrier of a
//! world-view exists and consults nothing — the list is prose, checked by the author.
//! This file derives the carrier set **from the registry**, over all thirty servable
//! operations, and asserts the derivation ([`census_the_request_side_carrier_table_is_exactly_what_the_registry_declares`]).
//!
//! **Second, "no path silently runs" is checked by sampling two observables.** The
//! delivering assertion is `status == Suspended` and `cost.states == 3`. A refusal that
//! advanced a lineage, sealed a snapshot, minted a workspace record, published an
//! artifact or opened a region satisfies both.
//! [`negative_control_a_weak_two_field_predicate_misses_what_the_fingerprint_catches`]
//! constructs exactly that miss and shows the weak predicate calls it clean.
//!
//! **Third, the refusal is never read.** G2 is the *agent-computer interface* gate. A
//! refusal an agent cannot act on without out-of-band knowledge fails the half of the
//! criterion that makes it a G2 bullet rather than a G1 one. This file reads what each
//! refusal actually carries and reports, per carrier, whether it is enough to recover
//! from ([`recovery_the_stale_snapshot_refusal_names_neither_the_current_head_nor_a_way_to_find_it`]).
//!
//! | Delivering evidence | This file |
//! |---|---|
//! | five operations, named by hand | the carrier table **derived from `registry::OPERATIONS`** and asserted equal to the declared one, over all thirty servable operations |
//! | one carrier (a superseded snapshot identity) | **six carriers**, K1–K6 below, each probed on every operation that consults it |
//! | asserts an error code | asserts the code **and** that the daemon's whole effect surface is byte-identical across the refusal ([`Fingerprint`]) |
//! | no control that the no-effect check can fail | rewinds the lineage through a real state seam so the refused write **lands**, and shows the fingerprint catches it ([`negative_control_rewinding_the_lineage_lets_the_refused_write_land_and_the_fingerprint_sees_it`]); and drives the *admitted* form of the heaviest probe so every equality is a measurement ([`control_a_resume_that_is_admitted_moves_the_fingerprint`]) |
//! | no complement | probes the operations that legitimately do **not** consult a view, so the criterion is not met by refusing everything ([`complement_reads_on_explicit_handles_do_not_refuse_after_the_world_moves`], [`complement_expanding_a_pack_pinned_to_a_superseded_snapshot_is_not_stale`]) |
//! | no recovery reading | reads `Error.recovery`, `Error.data`, `Error.detail`, `next_operations` and `epochs` on every refusal and states, per carrier, whether recovery needs out-of-band knowledge |
//!
//! # The carriers, enumerated
//!
//! A **staleness carrier** is a request-side value whose admissibility depends on the world
//! not having moved since the caller learned it. Six exist in this build:
//!
//! | Key | Carrier | Where it is declared | Consulted by | Refusal |
//! |---|---|---|---|---|
//! | K1 | the envelope's snapshot pin | `RequestEnvelope.snapshot: WorkspaceHandle nullable` | `verification.start`, `task.resume`, `context.expand` (and `context.compile`, not driven here) | `StaleSnapshot` |
//! | K2 | a snapshot identity in a request body | `WorkspaceHandle` fields | `workspace.fork` (`base`), `workspace.seal` (`snapshot`), `workspace.diff` (`before`/`after`) | `StaleSnapshot` |
//! | K3 | a continuation's pinned world | `ContinuationHandle` fields | `task.resume` | `StaleSnapshot`, `ContinuationEpochMismatch`, `EpochUnsupported` |
//! | K4 | declared snapshot epochs | `SnapshotEpochs`, inline and nested in `SnapshotComponents` | `workspace.create`, `workspace.create_by_reference` | `EpochUnsupported` |
//! | K5 | a remembered claim status | `expected_status: EvidenceStatus optional` | `evidence.verify` | `StatusConflict` |
//! | K6 | the sealedness of a named snapshot | not a request field — a property of the artifact K1/K2/K3 name | `verification.start`, `task.resume` | `StaleSnapshot` |
//!
//! K1–K5 are derived mechanically. K6 is **not** — it is a property of the referenced
//! record rather than of the request — and that asymmetry is stated rather than hidden: see
//! the absences below.
//!
//! # The verdict this file reaches
//!
//! **SATISFIED-AT-NARROWER-SCOPE.** The rejection half of the criterion holds on every
//! carrier this build implements, at a far stronger no-effect reading than the delivering
//! evidence used. The narrowing has four parts, each an executable assertion:
//!
//! 1. **The criterion is satisfied for K1, K2, K3-at-the-snapshot-dimension, K4, K5 and
//!    K6.** Every one is refused with the code its clause declares, and every refusal
//!    leaves the effect surface byte-identical.
//! 2. **`workspace.diff` cannot be probed at all.** It declares two K2 carriers and answers
//!    `UnsupportedSemanticFeature` uniformly, superseded or current, because the RFC 0031
//!    lane has not shipped. Its staleness behaviour is therefore *undetermined*, not
//!    satisfied — [`scope_workspace_diff_answers_uniformly_and_so_cannot_be_probed`].
//! 3. **K3's epoch dimension is delegated, not re-derived.** `gate_g1_04_acceptance.rs`
//!    closed it over twelve probes; repeating it would be the thing this bone is not for.
//!    What is asserted here is only that the carrier declares the dimension
//!    ([`census_the_continuation_carrier_declares_an_epoch_dimension_probed_under_g1_04`]).
//! 4. **Recovery from a K1/K2/K3 refusal is not possible from the refusal.** This is the
//!    finding, below.
//!
//! # Findings this file states rather than papers over (INV-007, INV-008)
//!
//! - **F1 — a `StaleSnapshot` refusal is a bare refusal, and RFC 0027 H8 says it must not
//!   be.** H8: "a stale-handle failure is recoverable and says so. Every row above carries
//!   a `recovery` list computed under N2, so 're-seal', 're-base' […] is executable rather
//!   than described." What the daemon returns is `recovery: []`, `data: absent`, and a
//!   `&'static str` `detail` that cannot name an identity. The current head appears nowhere
//!   in the refusal. See [`recovery_the_stale_snapshot_refusal_names_neither_the_current_head_nor_a_way_to_find_it`].
//! - **F2 — the value layer computes the identity the wire drops.**
//!   `continuum_workspace::staleness::check_current` returns `LineageError::Stale`, whose
//!   `current()` "names exactly the identity `Fork::advance`/`advance_to` need to redo the
//!   caller's step against". `daemon::workspace::lineage_fault` maps that value to
//!   `Fault::new(StaleSnapshot, <constant>)` and discards it. This is not a missing
//!   computation; it is a discarded one, one function call from the wire. See
//!   [`recovery_the_value_layer_computes_the_current_head_that_the_wire_refusal_drops`].
//! - **F3 — and no servable operation reports it either.** Recovery is therefore not merely
//!   absent from the refusal but absent from the protocol surface a refused agent can
//!   reach: no `@readonly` servable operation answers "what is this lineage's head now".
//!   `task.status` reports the task's *pinned* snapshot, which is the stale one. See
//!   [`recovery_no_servable_operation_reports_a_lineages_current_head`].
//! - **F4 — the epoch carrier is recovery-sufficient, and it is the only one that is.**
//!   `rule envelope.epochs_named` puts all six current epochs on *every* result including
//!   an error, so an `EpochUnsupported` refusal hands back exactly what the caller has to
//!   re-declare. The mechanism that would fix F1 for snapshots already exists for epochs.
//!   See [`recovery_an_epoch_refusal_names_the_daemons_current_epochs_in_a_typed_field`].
//! - **F5 — the status carrier is recoverable by a declared re-read.** `StatusConflict`'s
//!   own text is "Re-read and retry", `evidence.get` is servable, `read`-authority, and
//!   takes the handle the caller already presented. So K5 needs no new wire field: the
//!   handle *is* the recovery channel. That is the shape K1–K3 lack, and it shows the gap
//!   is a design omission rather than a protocol impossibility. See
//!   [`recovery_a_lost_status_guard_is_recoverable_by_a_declared_re_read`].
//! - **F6 — staleness is two different predicates, and only one is currency.** The
//!   `workspace`/`verification`/`task` families ask "is this identity still the lineage's
//!   head" (currency). The `context` family asks "does the envelope's pin equal the
//!   artifact's pin" (agreement), and deliberately serves a pack whose snapshot has been
//!   superseded, because RFC 0028 requires history to stay readable. Both answer
//!   `StaleSnapshot`. An agent cannot tell from the code which predicate refused it. See
//!   [`complement_expanding_a_pack_pinned_to_a_superseded_snapshot_is_not_stale`].
//! - **F7 — one snapshot has two identities, and the lineage holds the one the caller does
//!   not speak.** A lineage advances over `descriptor.source().identity()`; the wire names a
//!   snapshot by the content identity of its whole *descriptor*, which is what a `ws_*`
//!   handle is. So the value F2 says is discarded is not even directly presentable: publishing
//!   it would need a source-identity-to-`ws_*` mapping no operation in this protocol
//!   exposes. That sharpens F1 from "a field was dropped" to "the recovery channel is not
//!   one field away". Asserted at
//!   [`recovery_the_value_layer_computes_the_current_head_that_the_wire_refusal_drops`]
//!   (`assert_ne!` between the two spellings) and again at
//!   [`recovery_no_servable_operation_reports_a_lineages_current_head`].
//!
//! # Absences — what this file does not probe (INV-007)
//!
//! - **No transport, no codec, no CLI.** Every request goes through [`Daemon::dispatch`]
//!   in process, so nothing here says anything about the wire framing of these refusals.
//! - **No concurrency.** `Daemon::dispatch` takes `&mut self`; the TOCTOU probe is a
//!   *sequential* read-then-write across a world change at the API grain, which is the
//!   shape the protocol can express. A genuine interleaving is out of reach of this
//!   entry point.
//! - **K6 is not in the mechanical census.** Sealedness is a property of the record a
//!   handle resolves to, not a field of any request, so no scan over request shapes can
//!   find it. It is declared by hand and probed by hand
//!   ([`carrier_k6_verification_start_refuses_an_unsealed_snapshot`]), and this paragraph
//!   is the statement that the census is incomplete by exactly that one row.
//! - **K3's epoch dimension** is `gate_g1_04_acceptance.rs`'s, not this file's.
//! - **The two carrier-bearing operations this build cannot serve.** Nine of the
//!   seventy-five registry operations declare a K2–K5 carrier; seven of the nine are
//!   servable here, and `correspondence.drift` and `repair.resume` are not. Both the list
//!   and the exclusion are derived and asserted rather than described
//!   ([`census_the_servable_surface_is_thirty_operations_in_eight_families`]).
//! - **`context.compile`'s K1 consult is not driven.** It reaches the same
//!   `stale_snapshot_check` as `context.expand`, from the same envelope field, over a
//!   registered compile source this file does not build. `context.expand` is the probe.
//! - **`workspace.create_by_reference`'s K4 carrier** is declared, counted, and not driven:
//!   it needs a registered components commitment, and `workspace.create`'s nested K4
//!   carrier reaches the same `check_epochs` guard through the same code path.
//! - **The audit surface is outside the fingerprint**, for the reason
//!   `gate_g1_04_acceptance.rs` gives: RFC 0027 P5 requires an admission record whatever
//!   the answer was, so a fingerprint including it could never be equal across a refusal.
//!   [`a_refused_stale_write_is_still_audited`] asserts the opposite direction so that
//!   "outside the fingerprint" does not become "unchecked".

use std::collections::{BTreeMap, BTreeSet};

use continuum_context::expansion::{
    ExpansionPayload, ExpansionQuery, ExpansionRelation as PackRelation,
};
use continuum_context::omission::{OmissionReason as PackReason, OmissionRecord};
use continuum_context::selection::SelectionKind;
use continuum_context::source::{SourceRef, SourceSpan};
use continuum_engine_reference::diehard;
use continuum_intent::canonical_json::Json;
use continuum_intent::contract::IntentContract;
use continuum_value::epoch::ProtocolWindow;
use continuum_value::value::Name;
use continuum_workspace::artifact_path::ArtifactHandle as StoreHandle;
use continuum_workspace::lineage::ForkName;
use continuum_workspace::snapshot::WorkspacePath;
use continuum_workspace::staleness::{LineageError, check_current};
use continuumd::daemon::context::{ContextFamily, ContextPackRecord};
use continuumd::daemon::evidence::EvidenceFamily;
use continuumd::daemon::family::{Arguments, Payload};
use continuumd::daemon::identity::Blake3Identity;
use continuumd::daemon::intent::IntentFamily;
use continuumd::daemon::observe::ObserveFamily;
use continuumd::daemon::state::{IntentRecord, RegistryStatus};
use continuumd::daemon::task::TaskFamily;
use continuumd::daemon::verification::{VerificationFamily, model_source};
use continuumd::daemon::workspace::WorkspaceFamily;
use continuumd::daemon::{Daemon, OperationOutcome, OperationRequest, errors, identity};
use continuumd::protocol::envelope::{Budget, EpochSet, RequestEnvelope};
use continuumd::protocol::handshake::{
    CapabilityDescriptor, CapabilityProfile, ClientHello, Negotiated, VersionRange, negotiate,
};
use continuumd::protocol::operations::context::ContextExpandRequest;
use continuumd::protocol::operations::evidence::{EvidenceGetRequest, EvidenceVerifyRequest};
use continuumd::protocol::operations::intent::{IntentAcceptRequest, IntentGetRequest};
use continuumd::protocol::operations::observe::ObserveIngestRequest;
use continuumd::protocol::operations::task::{TaskResumeRequest, TaskStatusRequest};
use continuumd::protocol::operations::verification::VerificationStartRequest;
use continuumd::protocol::operations::workspace::{
    WorkspaceCreateRequest, WorkspaceDiffRequest, WorkspaceForkRequest, WorkspaceSealRequest,
    WorkspaceSealResponse,
};
use continuumd::protocol::registry::{self, ENCODINGS};
use continuumd::protocol::scalar::{
    ActorId, ByteCount, CapabilityHandle, Commitment, ContextHandle, ContinuationHandle,
    EpochIdentity, EvidenceHandle, IntentHandle, Opaque, OperationName, ProtocolVersion, RequestId,
    TaskHandle, Timestamp, WorkspaceHandle,
};
use continuumd::protocol::shared::{FileOverlay, SnapshotComponents, SnapshotEpochs, Target};
use continuumd::protocol::spec::{Nullable, OperationSpec, Optional, StructSpec};
use continuumd::protocol::vocabulary::{
    AuthorityLevel, DataGrant, DiffLayer, Encoding, ErrorCode, EvidenceStatus, Portfolio,
    ResultStatus, TargetKind, TaskStatus,
};

/// The TV-009 port's model, verbatim — the bytes that go into the snapshot.
const DIE_HARD_MODEL: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/DieHard.ctm");

/// The TV-009 port's default model configuration.
const DIE_HARD_CONFIG: &str =
    include_str!("../../../notes/plan/corpus/tla-examples/ports/TV-009/default.model.toml");

/// The Die Hard Intent Contract, as `continuum-intent`'s own suites use it.
const DIE_HARD_CONTRACT: &str =
    include_str!("../../continuum-intent/tests/fixtures/die-hard-contract.json");

/// Where the model lives inside the workspace.
const MODULE_PATH: &str = "DieHard.ctm";

/// A `states` ceiling that parks a Die Hard campaign short of its sixteen states.
const PARKING_CEILING: u64 = 4;

/// A `states` ceiling large enough to close the campaign, so a resume has work to leak.
const CLOSING_CEILING: u64 = 64;

/// A production trace, as a captured bundle would arrive.
const TRACE: &str = "{\"events\":[{\"at\":0,\"op\":\"fill\"},{\"at\":1,\"op\":\"pour\"}]}\n";

/// The instrumentation profile the trace above was captured under (plan §18.4).
const PROFILE: &str = "otel-1.0/sampled";

/// The context handle the pack fixture is registered under.
const PACK: &str = "ctx_parent1";

/// A conforming parent pack, as a document. Registered against a *real* snapshot handle;
/// the document's own `snapshot` member is not what `ContextPackRecord` pins by, which is
/// why a literal is admissible here.
const PARENT_PACK: &str = r#"{
  "assurance": {"class": "bounded", "envelope": {}},
  "content_budget": {"bytes": 16384},
  "content_hash": "blake3-256:parentplaceholder",
  "context_id": "ctx_parent1",
  "evidence": ["ev_failure1"],
  "expansions": [{"anchor": "e_ack", "relation": "source_span"}],
  "guarantees": ["ReplayPreserving", "CausallyClosed"],
  "intent": "in_ack_v1",
  "omissions": [
    {"count": 2, "expandable": true,
     "expansion": {"anchor": "e_ack", "relation": "source_span"},
     "kind": "source", "reason": "budget"}
  ],
  "parent": null,
  "question": "why did AckImpliesDurable fail?",
  "redactions": [],
  "replay": "crash_demo1",
  "schema_epoch": 1,
  "schema_id": "https://continuum.dev/schema/context-pack.json",
  "selected": [{"artifact": "ev_ack1", "id": "e_ack", "kind": "event",
                "summary": "reply published before stable write"}],
  "semantic_epoch": "sem3-r3-demo",
  "snapshot": "ws_demo1",
  "verdict": "refuted"
}"#;

// =========================================================================================
// A — fixtures
// =========================================================================================

fn version() -> ProtocolVersion {
    ProtocolVersion::new(3, 1)
}

fn cap(handle: &str) -> CapabilityHandle {
    CapabilityHandle::new(handle).expect("a well-formed capability handle")
}

fn who(actor: &str) -> ActorId {
    ActorId::new(actor).expect("a well-formed actor identity")
}

fn name(operation: &str) -> OperationName {
    OperationName::new(operation).expect("a well-formed operation name")
}

fn epoch(token: &str) -> EpochIdentity {
    EpochIdentity::new(token).expect("a well-formed epoch identity")
}

fn now() -> Timestamp {
    Timestamp::new("2026-08-01T00:00:00.000Z").expect("a well-formed timestamp")
}

/// The deployment every probe here serves unless it says otherwise.
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

fn profile(privileged: &[&str], grants: &[DataGrant]) -> CapabilityProfile {
    CapabilityProfile {
        privileged_operations: privileged.iter().map(|entry| name(entry)).collect(),
        denied_operations: Vec::new(),
        data_grants: grants.to_vec(),
        cross_principal_sharing: true,
    }
}

fn grant(
    handle: &str,
    actor: &str,
    level: AuthorityLevel,
    depth: u32,
    descriptor_profile: Optional<CapabilityProfile>,
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
        profile: descriptor_profile,
    }
}

fn negotiated() -> Negotiated {
    let wanted = version();
    let hello = ClientHello {
        protocol_versions: VersionRange {
            low: wanted,
            high: wanted,
        },
        encodings: vec![Encoding::CanonicalJson],
        client: "continuumd-gate-g2-04-acceptance".to_owned(),
        actor: who("service:continuumd"),
        capability: cap("cap_root"),
        features: Optional::Absent,
    };
    negotiate(&[wanted], ProtocolWindow::new(3), ENCODINGS, &hello)
        .expect("the requested version is served")
}

/// A daemon serving `served`, with all eight landed families registered.
///
/// Every family this build ships is registered on purpose: the census below is over the
/// *servable* surface, and a deployment missing a family would make an operation answer
/// `UnsupportedSemanticFeature` for a reason that has nothing to do with staleness.
fn daemon_serving(served: &EpochSet) -> Daemon {
    let root = Some(cap("cap_root"));
    Daemon::builder(Blake3Identity, negotiated(), cap("cap_root"))
        .epochs(served.clone())
        .now(now())
        .capability(
            grant(
                "cap_root",
                "service:continuumd",
                AuthorityLevel::Promote,
                4,
                Optional::Present(profile(
                    &["intent.accept", "intent.reject", "intent.lock"],
                    &[DataGrant::ProductionTrace],
                )),
            ),
            None,
        )
        .capability(
            grant(
                "cap_builder",
                "agent:builder",
                AuthorityLevel::Propose,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_runner",
                "agent:runner",
                AuthorityLevel::Execute,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_reader",
                "agent:reader",
                AuthorityLevel::Read,
                3,
                Optional::Absent,
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_observer",
                "agent:observer",
                AuthorityLevel::Execute,
                3,
                Optional::Present(profile(&[], &[DataGrant::ProductionTrace])),
            ),
            root.clone(),
        )
        .capability(
            grant(
                "cap_steward",
                "human:steward",
                AuthorityLevel::ReviseIntent,
                3,
                Optional::Present(profile(
                    &["intent.accept", "intent.reject", "intent.lock"],
                    &[],
                )),
            ),
            root,
        )
        .family(WorkspaceFamily)
        .family(IntentFamily)
        .family(TaskFamily)
        .family(VerificationFamily)
        .family(ContextFamily)
        .family(EvidenceFamily::default())
        .family(ObserveFamily)
        .build()
}

fn envelope(operation: &str, actor: &str, capability: &str, request: &str) -> RequestEnvelope {
    RequestEnvelope {
        protocol_version: version(),
        request_id: RequestId::new(request).expect("a well-formed request id"),
        idempotency_key: Optional::Absent,
        actor: who(actor),
        capability: cap(capability),
        operation: name(operation),
        snapshot: Nullable::Null,
        intent: Nullable::Null,
        arguments: Opaque::from_bytes(Vec::new()),
        budget: Optional::Absent,
        output_policy: Optional::Absent,
        trace: Optional::Absent,
        page: Optional::Absent,
    }
}

fn keyed(mut request_envelope: RequestEnvelope, key: &str) -> RequestEnvelope {
    request_envelope.idempotency_key = Optional::Present(key.to_owned());
    request_envelope
}

fn on(mut request_envelope: RequestEnvelope, snapshot: &WorkspaceHandle) -> RequestEnvelope {
    request_envelope.snapshot = Nullable::Value(snapshot.clone());
    request_envelope
}

fn budget(states: u64, bytes: Optional<ByteCount>) -> Budget {
    Budget {
        wall_ms: Optional::Absent,
        cpu_ms: Optional::Absent,
        memory_bytes: Optional::Absent,
        states: Optional::Present(states),
        solver_ms: Optional::Absent,
        proof_ms: Optional::Absent,
        tokens: Optional::Absent,
        candidates: Optional::Absent,
        bytes,
    }
}

fn budgeted(mut request_envelope: RequestEnvelope, states: u64) -> RequestEnvelope {
    request_envelope.budget = Optional::Present(budget(states, Optional::Absent));
    request_envelope
}

fn die_hard_contract() -> IntentContract {
    IntentContract::decode(DIE_HARD_CONTRACT.trim_end().as_bytes()).expect("the fixture decodes")
}

fn intent_handle(contract: &IntentContract) -> IntentHandle {
    let stored = continuum_workspace::publication::ContentIdentifier::identify(
        &Blake3Identity,
        continuum_workspace::artifact_path::ArtifactClass::IntentContract,
        &contract.identity_preimage_bytes(),
    )
    .expect("blake3 names every input");
    identity::intent_to_wire(&stored).expect("an `in_` handle")
}

fn acceptance_bytes() -> Opaque {
    let mut fields: BTreeMap<String, Json> = BTreeMap::new();
    for (key, value) in [
        ("accepted_by", "human:steward"),
        ("capability", "revise-intent"),
        ("signature", "sig-die-hard-v1"),
        ("audit_record", "supplied-by-the-caller-and-overwritten"),
        ("timestamp", "2026-08-01T00:00:00.000Z"),
    ] {
        fields.insert(key.to_owned(), Json::String(value.to_owned()));
    }
    Opaque::from_bytes(Json::Object(fields).to_canonical_bytes())
}

fn target(kind: TargetKind, id: &str) -> Target {
    Target {
        kind,
        id: id.to_owned(),
    }
}

/// The `SnapshotEpochs` a create request must declare against a deployment serving `served`.
fn declared_epochs(served: &EpochSet) -> SnapshotEpochs {
    SnapshotEpochs {
        semantic: served
            .semantic
            .value()
            .cloned()
            .unwrap_or_else(|| epoch("semantic-1")),
        proof: served
            .proof
            .value()
            .cloned()
            .unwrap_or_else(|| epoch("proof-1")),
        toolchain: Optional::Absent,
    }
}

/// The content identity of a snapshot carrying only `DieHard.ctm` as a model source.
fn die_hard_source() -> Commitment {
    model_source(&Blake3Identity, [(MODULE_PATH, DIE_HARD_MODEL.as_bytes())])
        .expect("blake3 names the module set")
}

fn pack_record(snapshot: &WorkspaceHandle) -> ContextPackRecord {
    let span = SourceSpan::new(
        WorkspacePath::new("src/ack.rs").expect("a repo-relative path"),
        10,
        1,
        10,
        40,
    )
    .expect("a well-formed span");
    let items = vec![
        SourceRef::new(span.clone()).into_selected_item(item_name("s_1")),
        SourceRef::new(span).into_selected_item(item_name("s_2")),
    ];
    let payload = ExpansionPayload::new(
        OmissionRecord::expandable(
            SelectionKind::Source,
            2,
            PackReason::Budget,
            ExpansionQuery::new(PackRelation::SourceSpan, item_name("e_ack")),
        ),
        items,
    )
    .expect("two items for a count of two");
    ContextPackRecord::new(
        Json::parse(PARENT_PACK.as_bytes()).expect("the fixture is admissible canonical JSON"),
        snapshot.clone(),
        [payload],
    )
    .expect("a conforming pack and a well-formed expansion graph")
}

fn item_name(text: &str) -> Name {
    Name::new(text).expect("a canonical identifier")
}

/// The source-tree identity of the snapshot `handle` names — the value a lineage is
/// advanced over, which is *not* the `ws_*` handle itself.
fn source_identity_of(daemon: &Daemon, handle: &WorkspaceHandle) -> StoreHandle {
    daemon
        .state()
        .workspace(handle)
        .expect("the daemon holds the snapshot")
        .descriptor
        .source()
        .identity()
        .clone()
}

// =========================================================================================
// B — the world, and the one way it moves
// =========================================================================================

/// Every handle this file's [`Fingerprint`] reads by name.
///
/// The daemon exposes total iteration over tasks, intents, evidence and the store, but not
/// over workspaces, lineages, continuations or staged content. Every handle any probe here
/// mints is registered, so the only gap is a handle no code path in this file produces.
#[derive(Debug, Clone, Default)]
struct Known {
    workspaces: BTreeSet<WorkspaceHandle>,
    continuations: BTreeSet<ContinuationHandle>,
    lineages: BTreeSet<ForkName>,
    commitments: BTreeSet<Commitment>,
}

/// One daemon, its world, and the parked campaign it holds.
struct World {
    daemon: Daemon,
    served: EpochSet,
    intent: IntentHandle,
    /// The snapshot the world was created at — the head *before* anything supersedes it.
    origin: WorkspaceHandle,
    lineage: ForkName,
    task: TaskHandle,
    continuation: ContinuationHandle,
    evidence: EvidenceHandle,
    known: Known,
}

fn world() -> World {
    world_serving(&epochs())
}

/// Build the whole world through `Daemon::dispatch`, except the four registrations the
/// protocol has no operation for: the intent registry, staged content, the model catalog,
/// and the Context Pack. Those are `DaemonState`'s own documented out-of-band door.
fn world_serving(served: &EpochSet) -> World {
    let mut daemon = daemon_serving(served);
    let contract = die_hard_contract();
    let intent = intent_handle(&contract);
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

    let mut files = Vec::new();
    for (path, content) in [(MODULE_PATH, DIE_HARD_MODEL), ("README.md", "# TV-009\n")] {
        files.push(
            daemon
                .state_mut()
                .stage(
                    &Blake3Identity,
                    WorkspacePath::new(path).expect("a workspace path"),
                    content.as_bytes().to_vec(),
                )
                .expect("staging names its content"),
        );
    }
    let configuration = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("default.model.toml").expect("a workspace path"),
            DIE_HARD_CONFIG.as_bytes().to_vec(),
        )
        .expect("staging names its content");
    let trace = daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("traces/die-hard.jsonl").expect("a workspace path"),
            TRACE.as_bytes().to_vec(),
        )
        .expect("staging names its content");

    daemon.state_mut().models_mut().register(
        die_hard_source(),
        diehard::model().expect("the port builds"),
    );

    ok(&daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "intent.accept",
                "human:steward",
                "cap_steward",
                "req_accept",
            ),
            "idem-accept",
        ),
        arguments: Arguments::IntentAccept(IntentAcceptRequest {
            proposal: intent.clone(),
            acceptance: acceptance_bytes(),
            bundle: Optional::Absent,
        }),
    }));

    let created = daemon.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "workspace.create",
                "agent:builder",
                "cap_builder",
                "req_create",
            ),
            "idem-create",
        ),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: components(&files, &configuration, &intent, served),
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    });
    ok(&created);
    let origin = match &created.payload {
        Payload::WorkspaceCreate(response) => response.snapshot.clone(),
        other => panic!("expected a workspace.create payload, got {other:?}"),
    };
    let lineage = daemon
        .state()
        .workspace(&origin)
        .expect("the created snapshot is held")
        .lineage
        .clone();

    let started = daemon.dispatch(&OperationRequest {
        envelope: on(
            budgeted(
                keyed(
                    envelope(
                        "verification.start",
                        "agent:runner",
                        "cap_runner",
                        "req_start",
                    ),
                    "idem-start",
                ),
                PARKING_CEILING,
            ),
            &origin,
        ),
        arguments: Arguments::VerificationStart(VerificationStartRequest {
            target: target(TargetKind::AllClaims, "DieHard"),
            portfolio: Portfolio::Interactive,
            context_policy: Optional::Absent,
            priority_class: Optional::Absent,
        }),
    });
    assert_eq!(
        started.envelope.status,
        ResultStatus::TaskSuspended,
        "a Die Hard campaign bounded at {PARKING_CEILING} states parks: {:?}",
        started.envelope.error
    );
    let task = match &started.payload {
        Payload::VerificationStart(response) => response
            .task
            .value()
            .cloned()
            .expect("a fresh start names a task"),
        other => panic!("expected a verification.start payload, got {other:?}"),
    };
    let continuation = daemon
        .state()
        .tasks()
        .get(&task)
        .expect("the task is held")
        .continuation
        .clone()
        .expect("a suspended task has a continuation");

    let ingested = daemon.dispatch(&OperationRequest {
        envelope: {
            let mut request_envelope = keyed(
                envelope(
                    "observe.ingest",
                    "agent:observer",
                    "cap_observer",
                    "req_obs",
                ),
                "idem-obs",
            );
            request_envelope.budget = Optional::Present(budget(0, Optional::Absent));
            request_envelope
        },
        arguments: Arguments::ObserveIngest(ObserveIngestRequest {
            trace: trace.clone(),
            instrumentation_profile: PROFILE.to_owned(),
        }),
    });
    ok(&ingested);
    let evidence = match &ingested.payload {
        Payload::ObserveIngest(response) => response
            .evidence
            .first()
            .cloned()
            .expect("an ingest names the node it appended"),
        other => panic!("expected an observe.ingest payload, got {other:?}"),
    };

    // The Context Pack is registered against the snapshot the world was created at, so a
    // later fork supersedes exactly the identity the pack is stated against.
    daemon.state_mut().put_context_pack(
        ContextHandle::new(PACK).expect("a context handle"),
        pack_record(&origin),
    );

    let mut known = Known::default();
    known.workspaces.insert(origin.clone());
    known.continuations.insert(continuation.clone());
    known.lineages.insert(lineage.clone());
    known.commitments.extend(files.iter().cloned());
    known.commitments.insert(configuration);
    known.commitments.insert(trace);

    World {
        daemon,
        served: served.clone(),
        intent,
        origin,
        lineage,
        task,
        continuation,
        evidence,
        known,
    }
}

fn components(
    files: &[Commitment],
    configuration: &Commitment,
    intent: &IntentHandle,
    served: &EpochSet,
) -> SnapshotComponents {
    SnapshotComponents {
        files: files.to_vec(),
        cml_modules: Vec::new(),
        rust_extraction: Vec::new(),
        domain_packs: Vec::new(),
        dependencies: Vec::new(),
        epochs: declared_epochs(served),
        intent: intent.clone(),
        correspondence: Vec::new(),
        proof_environment: Vec::new(),
        configuration: vec![configuration.clone()],
        file_components: Optional::Absent,
    }
}

fn ok(outcome: &OperationOutcome) {
    assert_eq!(
        outcome.envelope.status,
        ResultStatus::Ok,
        "{:?}",
        outcome.envelope.error
    );
}

fn code(outcome: &OperationOutcome) -> Option<ErrorCode> {
    outcome.error_code()
}

impl World {
    fn dispatch(&mut self, request: &OperationRequest) -> OperationOutcome {
        self.daemon.dispatch(request)
    }

    /// **The one way the world moves in this file**: a real `workspace.fork`, which is the
    /// compare-and-set advance of the lineage. Nothing here supersedes a snapshot by
    /// writing state behind the daemon's back — that would prove a fact about a graft
    /// rather than about the protocol.
    fn advance(&mut self, base: &WorkspaceHandle, body: &str, request: &str) -> WorkspaceHandle {
        let outcome = self.daemon.dispatch(&OperationRequest {
            envelope: keyed(
                envelope("workspace.fork", "agent:builder", "cap_builder", request),
                &format!("idem-{request}"),
            ),
            arguments: Arguments::WorkspaceFork(WorkspaceForkRequest {
                base: base.clone(),
                overlay: Optional::Present(vec![FileOverlay {
                    path: "README.md".to_owned(),
                    content: body.as_bytes().to_vec(),
                }]),
                patches: Optional::Absent,
            }),
        });
        ok(&outcome);
        let forked = match &outcome.payload {
            Payload::WorkspaceFork(response) => response.snapshot.clone(),
            other => panic!("expected a workspace.fork payload, got {other:?}"),
        };
        self.known.workspaces.insert(forked.clone());
        forked
    }

    /// `workspace.fork` presenting `base`, without asserting the answer.
    ///
    /// A handle a *successful* fork mints is registered with [`Known`], so the fingerprint
    /// can see it. A refused fork mints nothing, so a refusal probe registers nothing and
    /// the two readings around it stay comparable.
    fn try_fork(&mut self, base: &WorkspaceHandle, request: &str) -> OperationOutcome {
        let outcome = self.daemon.dispatch(&OperationRequest {
            envelope: keyed(
                envelope("workspace.fork", "agent:builder", "cap_builder", request),
                &format!("idem-{request}"),
            ),
            arguments: Arguments::WorkspaceFork(WorkspaceForkRequest {
                base: base.clone(),
                overlay: Optional::Present(vec![FileOverlay {
                    path: "README.md".to_owned(),
                    content: b"# a third edit\n".to_vec(),
                }]),
                patches: Optional::Absent,
            }),
        });
        if let Payload::WorkspaceFork(response) = &outcome.payload {
            self.known.workspaces.insert(response.snapshot.clone());
        }
        outcome
    }

    fn try_seal(&mut self, snapshot: &WorkspaceHandle, request: &str) -> OperationOutcome {
        self.daemon.dispatch(&OperationRequest {
            envelope: keyed(
                envelope("workspace.seal", "agent:builder", "cap_builder", request),
                &format!("idem-{request}"),
            ),
            arguments: Arguments::WorkspaceSeal(WorkspaceSealRequest {
                snapshot: snapshot.clone(),
            }),
        })
    }

    fn try_start(&mut self, snapshot: &WorkspaceHandle, request: &str) -> OperationOutcome {
        self.daemon.dispatch(&OperationRequest {
            envelope: on(
                budgeted(
                    keyed(
                        envelope("verification.start", "agent:runner", "cap_runner", request),
                        &format!("idem-{request}"),
                    ),
                    CLOSING_CEILING,
                ),
                snapshot,
            ),
            arguments: Arguments::VerificationStart(VerificationStartRequest {
                target: target(TargetKind::AllClaims, "DieHard"),
                portfolio: Portfolio::Interactive,
                context_policy: Optional::Absent,
                priority_class: Optional::Absent,
            }),
        })
    }

    /// `task.resume` under a raised ceiling, so a refusal that leaked would have something
    /// to leak: a budget write, a region, a milestone, a published campaign record.
    fn try_resume(&mut self, request: &str) -> OperationOutcome {
        let continuation = self.continuation.clone();
        self.daemon.dispatch(&OperationRequest {
            envelope: budgeted(
                keyed(
                    envelope("task.resume", "agent:runner", "cap_runner", request),
                    &format!("idem-{request}"),
                ),
                CLOSING_CEILING,
            ),
            arguments: Arguments::TaskResume(TaskResumeRequest {
                continuation,
                budget: Optional::Present(budget(CLOSING_CEILING, Optional::Absent)),
            }),
        })
    }

    fn try_expand(&mut self, pin: Option<&WorkspaceHandle>, request: &str) -> OperationOutcome {
        let mut request_envelope = keyed(
            envelope("context.expand", "agent:reader", "cap_reader", request),
            &format!("idem-{request}"),
        );
        request_envelope.budget =
            Optional::Present(budget(0, Optional::Present(ByteCount::new(16384))));
        if let Some(snapshot) = pin {
            request_envelope = on(request_envelope, snapshot);
        }
        self.daemon.dispatch(&OperationRequest {
            envelope: request_envelope,
            arguments: Arguments::ContextExpand(ContextExpandRequest {
                context: ContextHandle::new(PACK).expect("a context handle"),
                anchor: "e_ack".to_owned(),
                relation: continuumd::protocol::vocabulary::ExpansionRelation::SourceSpan,
                depth: Optional::Absent,
            }),
        })
    }

    fn try_verify(
        &mut self,
        expected: Optional<EvidenceStatus>,
        request: &str,
    ) -> OperationOutcome {
        let evidence = self.evidence.clone();
        let mut request_envelope = keyed(
            envelope("evidence.verify", "agent:reader", "cap_reader", request),
            &format!("idem-{request}"),
        );
        request_envelope.budget = Optional::Present(budget(0, Optional::Absent));
        self.daemon.dispatch(&OperationRequest {
            envelope: request_envelope,
            arguments: Arguments::EvidenceVerify(EvidenceVerifyRequest {
                evidence,
                expected_status: expected,
            }),
        })
    }

    fn task_status(&mut self, request: &str) -> TaskStatus {
        let task = self.task.clone();
        let outcome = self.daemon.dispatch(&OperationRequest {
            envelope: envelope("task.status", "agent:reader", "cap_reader", request),
            arguments: Arguments::TaskStatus(TaskStatusRequest { task }),
        });
        match &outcome.payload {
            Payload::TaskStatus(record) => record.status,
            other => panic!("expected a task.status payload, got {other:?}"),
        }
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of(&self.daemon, &self.known)
    }

    /// The lineage's head, as the daemon holds it.
    ///
    /// **This is a source-tree identity, not a `ws_*` handle.** A lineage advances over
    /// `descriptor.source().identity()`, while the wire names a snapshot by the content
    /// identity of its whole *descriptor*. The two are different values for the same
    /// snapshot, which is why [`source_identity_of`] exists and why finding F2 is stronger
    /// than "a field was dropped": what was dropped is not even in the caller's vocabulary.
    ///
    /// Not reachable over the wire at all — finding F3 — so this file reads it through the
    /// state surface in order to *state* the gap.
    fn head(&self) -> StoreHandle {
        self.daemon
            .state()
            .lineage(&self.lineage)
            .expect("the world holds its own lineage")
            .identity()
            .clone()
    }
}

// =========================================================================================
// C — the instrument: everything a refusal could have moved
// =========================================================================================

/// The daemon's whole effect surface, as one comparable value.
///
/// Compared as ordered lists rather than as a digest, so a failure names the row that
/// moved. Deliberately outside it: the admission ledger, the store's authorization audit
/// log, and the idempotency ledger — all three *must* move on a refusal (RFC 0027 P5), so a
/// fingerprint including them could never be equal across one.
/// [`a_refused_stale_write_is_still_audited`] asserts that direction separately.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Fingerprint {
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
    context_packs: Vec<String>,
}

impl Fingerprint {
    fn of(daemon: &Daemon, known: &Known) -> Self {
        let token =
            identity::capability_to_store(&cap("cap_root")).expect("`cap_root` is a store token");
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
                    .expect("`cap_root` confers read on a published artifact");
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
        let continuations = known
            .continuations
            .iter()
            .map(|handle| format!("{handle:?} => {:?}", state.tasks().continuation(handle)))
            .collect();
        let workspaces = known
            .workspaces
            .iter()
            .map(|handle| format!("{handle:?} => {:?}", state.workspace(handle)))
            .collect();

        let mut lineage_names = known.lineages.clone();
        for handle in &known.workspaces {
            if let Some(record) = state.workspace(handle) {
                lineage_names.insert(record.lineage.clone());
            }
        }
        let lineages = lineage_names
            .iter()
            .map(|fork| format!("{fork:?} => {:?}", state.lineage(fork)))
            .collect();

        Self {
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
            staged: known
                .commitments
                .iter()
                .map(|commitment| format!("{commitment:?} => {:?}", state.staged(commitment)))
                .collect(),
            context_packs: vec![format!(
                "{PACK} => {:?}",
                state
                    .context_pack(&ContextHandle::new(PACK).expect("a context handle"))
                    .map(|record| record.snapshot().clone())
            )],
        }
    }
}

/// Drive one probe and assert both halves of the criterion at once: the typed refusal, and
/// a byte-identical effect surface across it.
fn refuses_with_no_effect(
    world: &mut World,
    expected: ErrorCode,
    what: &str,
    probe: impl FnOnce(&mut World) -> OperationOutcome,
) -> OperationOutcome {
    let before = world.fingerprint();
    let outcome = probe(world);
    let after = world.fingerprint();
    assert_eq!(
        code(&outcome),
        Some(expected),
        "{what}: expected {expected:?}, got {:?}",
        outcome.envelope
    );
    assert!(
        matches!(outcome.payload, Payload::None),
        "{what}: a refusal carries no response body"
    );
    assert_eq!(before, after, "{what}: the refusal moved observable state");
    outcome
}

// =========================================================================================
// D — the census
// =========================================================================================

/// The eight namespaces this build serves.
const SERVED_NAMESPACES: &[&str] = &[
    "context",
    "evidence",
    "intent",
    "observe",
    "task",
    "verification",
    "whiteboard",
    "workspace",
];

/// The IDL types that carry a world-view, and the key each is.
const CARRIER_TYPES: &[(&str, &str)] = &[
    ("WorkspaceHandle", "K2"),
    ("ContinuationHandle", "K3"),
    ("SnapshotEpochs", "K4"),
];

/// The request field *names* that carry a world-view whose type does not say so.
///
/// `EvidenceStatus` is not a carrier by type: `evidence.query.statuses` is a filter, and
/// filtering by a status the graph has moved past is a legitimate query rather than a stale
/// view. The guard is the *name* `expected_status`, which the IDL documents as "the status
/// the caller expects the claim to currently hold".
const CARRIER_FIELDS: &[(&str, &str)] = &[("expected_status", "K5")];

/// Request-body types the census descends one level into.
///
/// `workspace.create` carries its K4 epoch pin nested inside `SnapshotComponents`, so a
/// scan of top-level fields alone would miss it — and a census that missed a carrier would
/// be exactly the failure mode this file exists to rule out.
fn nested(ty: &str) -> Option<StructSpec> {
    match ty {
        "SnapshotComponents" => Some(StructSpec::of::<SnapshotComponents>()),
        _ => None,
    }
}

/// The declared carrier table: operation → (field path, key), sorted.
///
/// This is the *claim*. [`census_the_request_side_carrier_table_is_exactly_what_the_registry_declares`]
/// derives the same table from `registry::OPERATIONS` and asserts they are equal, so a
/// carrier added to the protocol and not to this table fails the build rather than going
/// unprobed.
const DECLARED_CARRIERS: &[(&str, &str, &str)] = &[
    ("evidence.verify", "expected_status", "K5"),
    ("task.resume", "continuation", "K3"),
    ("workspace.create", "components.epochs", "K4"),
    ("workspace.create_by_reference", "epochs", "K4"),
    ("workspace.diff", "after", "K2"),
    ("workspace.diff", "before", "K2"),
    ("workspace.fork", "base", "K2"),
    ("workspace.seal", "snapshot", "K2"),
];

/// Scan one operation's request body for carriers, descending one level where [`nested`]
/// says to.
fn carriers_of(spec: &OperationSpec) -> Vec<(String, &'static str)> {
    let mut found = Vec::new();
    scan(&spec.request, "", &mut found);
    found.sort();
    found
}

fn scan(body: &StructSpec, prefix: &str, found: &mut Vec<(String, &'static str)>) {
    for field in body.fields {
        let path = if prefix.is_empty() {
            field.name.to_owned()
        } else {
            format!("{prefix}.{}", field.name)
        };
        if let Some((_, key)) = CARRIER_FIELDS.iter().find(|(n, _)| *n == field.name) {
            found.push((path.clone(), *key));
            continue;
        }
        if let Some((_, key)) = CARRIER_TYPES.iter().find(|(t, _)| *t == field.ty) {
            found.push((path.clone(), *key));
            continue;
        }
        if let Some(inner) = nested(field.ty) {
            scan(&inner, &path, found);
        }
    }
}

/// Every registry operation whose namespace this build serves.
fn servable() -> Vec<&'static OperationSpec> {
    registry::OPERATIONS
        .iter()
        .filter(|spec| {
            spec.name
                .split_once('.')
                .is_some_and(|(namespace, _)| SERVED_NAMESPACES.contains(&namespace))
        })
        .collect()
}

#[test]
fn census_the_servable_surface_is_thirty_operations_in_eight_families() {
    // The bound every count in this file is stated against. `Arguments` is a closed enum
    // with one variant per servable operation, so the arithmetic is a fact about the build
    // rather than about this table.
    let served = servable();
    assert_eq!(
        served.len(),
        30,
        "the eight registered families serve thirty of the registry's {} operations",
        registry::OPERATION_COUNT
    );
    assert_eq!(registry::OPERATION_COUNT, 75);

    // And the census's own denominator: how many of all seventy-five declare a carrier.
    // The census's own denominator, derived rather than described: which of all
    // seventy-five declare a K2–K5 carrier at all. Seven of the nine are servable; the two
    // that are not are named here so the gap between "declares a carrier" and "this file
    // probes it" is a list rather than a number.
    let with_carriers: Vec<&str> = registry::OPERATIONS
        .iter()
        .filter(|spec| !carriers_of(spec).is_empty())
        .map(|spec| spec.name)
        .collect();
    assert_eq!(
        with_carriers,
        vec![
            "workspace.create",
            "workspace.create_by_reference",
            "workspace.fork",
            "workspace.diff",
            "workspace.seal",
            "correspondence.drift",
            "repair.resume",
            "task.resume",
            "evidence.verify",
        ],
        "nine of the seventy-five declare a request-side carrier"
    );
    let unservable: Vec<&&str> = with_carriers
        .iter()
        .filter(|name| !served.iter().any(|spec| spec.name == **name))
        .collect();
    assert_eq!(
        unservable,
        vec![&"correspondence.drift", &"repair.resume"],
        "and exactly two of them are outside this build's servable surface"
    );
}

#[test]
fn census_the_request_side_carrier_table_is_exactly_what_the_registry_declares() {
    let mut derived: Vec<(String, String, &'static str)> = Vec::new();
    for spec in servable() {
        for (path, key) in carriers_of(spec) {
            derived.push((spec.name.to_owned(), path, key));
        }
    }
    derived.sort();

    let declared: Vec<(String, String, &'static str)> = DECLARED_CARRIERS
        .iter()
        .map(|(op, path, key)| ((*op).to_owned(), (*path).to_owned(), *key))
        .collect();

    assert_eq!(
        derived, declared,
        "the carrier table and the registry disagree: a carrier was added, removed, or \
         renamed without this file's probes following it"
    );
}

#[test]
fn census_the_envelope_carrier_k1_is_universal_and_the_daemon_admits_the_code_for_it() {
    // K1 is not a request-body field, so no scan finds it: `rule errors.common` states it
    // as a property of the *call* — "every operation taking a non-null `snapshot` MAY
    // additionally return `StaleSnapshot`". The two halves of that sentence are asserted
    // here against the daemon's own reading of the rule.
    for spec in servable() {
        assert!(
            errors::admits_with_snapshot(spec, ErrorCode::StaleSnapshot, true),
            "{}: a call naming a snapshot must admit StaleSnapshot",
            spec.name
        );
    }
    // And the complement: an operation that names no snapshot admits it only where its own
    // `errors` clause does. Exactly one of the thirty declares it outright, which is why
    // the check below is over the difference rather than over all of them.
    let declaring: Vec<&str> = servable()
        .into_iter()
        .filter(|spec| spec.errors.contains(&ErrorCode::StaleSnapshot))
        .map(|spec| spec.name)
        .collect();
    assert_eq!(declaring, vec!["task.resume"]);
    for spec in servable() {
        if spec.errors.contains(&ErrorCode::StaleSnapshot) {
            continue;
        }
        assert!(
            !errors::admits_with_snapshot(spec, ErrorCode::StaleSnapshot, false),
            "{}: a call naming no snapshot has no route to StaleSnapshot",
            spec.name
        );
    }
}

#[test]
fn census_the_continuation_carrier_declares_an_epoch_dimension_probed_under_g1_04() {
    // K3 has two dimensions. This file probes the snapshot one; the epoch one is
    // `gate_g1_04_acceptance.rs`'s, over twelve probes, and re-deriving it here would be
    // the duplication this bone is explicitly not for. What is asserted here is only that
    // the dimension is declared, so the delegation names something real.
    let resume = registry::operation("task.resume").expect("task.resume is registered");
    for declared in [
        ErrorCode::StaleSnapshot,
        ErrorCode::ContinuationEpochMismatch,
        ErrorCode::EpochUnsupported,
    ] {
        assert!(
            resume.errors.contains(&declared),
            "task.resume declares {declared:?}"
        );
    }
}

#[test]
fn negative_control_the_census_flags_a_synthetic_carrier_the_table_does_not_cover() {
    // The census is only worth its assertion if it *can* fail. A synthetic operation whose
    // request body is `workspace.seal`'s — one `snapshot: WorkspaceHandle` — is a carrier
    // the declared table does not name. Feeding it through the same scan must produce a
    // row the table does not contain.
    let synthetic = OperationSpec {
        name: "synthetic.carrier",
        authority: AuthorityLevel::Propose,
        annotations: &[],
        request: StructSpec::of::<WorkspaceSealRequest>(),
        response: StructSpec::of::<WorkspaceSealResponse>(),
        verdict: None,
        events: None,
        errors: &[],
    };
    let found = carriers_of(&synthetic);
    assert_eq!(
        found,
        vec![("snapshot".to_owned(), "K2")],
        "the scan finds the synthetic operation's carrier"
    );
    assert!(
        !DECLARED_CARRIERS
            .iter()
            .any(|(op, _, _)| *op == "synthetic.carrier"),
        "and the declared table does not cover it, so the equality assertion would fail"
    );

    // The other direction: the scan is not a tautology that matches every handle. A
    // `TaskHandle` names an object, not a view of the world, and must not be found.
    let status = registry::operation("task.status").expect("task.status is registered");
    assert!(
        carriers_of(status).is_empty(),
        "task.status names a task, which is an object rather than a world-view"
    );
}

// =========================================================================================
// E — the refusal probes, one per carrier and consuming operation
// =========================================================================================

#[test]
fn control_a_resume_that_is_admitted_moves_the_fingerprint() {
    // The companion every zero-delta assertion in section E needs, for the heaviest
    // operation of the set. `task.resume` under a raised ceiling opens a region, writes a
    // budget, advances the task and publishes a campaign record — so the equality asserted
    // by [`carrier_k3_task_resume_refuses_a_continuation_pinned_to_a_superseded_snapshot`]
    // is a measurement of a refusal rather than of an operation that does nothing anyway.
    let mut world = world();
    let before = world.fingerprint();
    let admitted = world.try_resume("req_live_resume");
    assert_ne!(
        admitted.envelope.status,
        ResultStatus::Error,
        "an untouched world admits the resume: {:?}",
        admitted.envelope.error
    );
    let after = world.fingerprint();
    assert_ne!(before, after, "an admitted resume moves the world");
    assert_ne!(before.tasks, after.tasks, "and specifically the task table");
    assert_ne!(
        before.regions_opened, after.regions_opened,
        "and it opened a region, which a refusal must not"
    );
}

#[test]
fn carrier_k2_workspace_fork_refuses_a_superseded_base() {
    let mut world = world();
    let origin = world.origin.clone();
    world.advance(&origin, "# moved on\n", "req_advance");
    refuses_with_no_effect(
        &mut world,
        ErrorCode::StaleSnapshot,
        "workspace.fork over a superseded base",
        |world| world.try_fork(&origin, "req_stale_fork"),
    );
}

#[test]
fn carrier_k2_workspace_seal_refuses_a_superseded_snapshot() {
    let mut world = world();
    let origin = world.origin.clone();
    world.advance(&origin, "# moved on\n", "req_advance");
    refuses_with_no_effect(
        &mut world,
        ErrorCode::StaleSnapshot,
        "workspace.seal over a superseded snapshot",
        |world| world.try_seal(&origin, "req_stale_seal"),
    );
}

#[test]
fn carrier_k1_verification_start_refuses_a_superseded_envelope_snapshot() {
    let mut world = world();
    let origin = world.origin.clone();
    world.advance(&origin, "# moved on\n", "req_advance");
    refuses_with_no_effect(
        &mut world,
        ErrorCode::StaleSnapshot,
        "verification.start pinned to a superseded snapshot",
        |world| world.try_start(&origin, "req_stale_start"),
    );
}

#[test]
fn carrier_k3_task_resume_refuses_a_continuation_pinned_to_a_superseded_snapshot() {
    let mut world = world();
    let origin = world.origin.clone();
    world.advance(&origin, "# moved on\n", "req_advance");
    // The continuation is untouched: what moved is the world it pinned. That is the
    // criterion's own sentence, rather than a hand-edited pin.
    refuses_with_no_effect(
        &mut world,
        ErrorCode::StaleSnapshot,
        "task.resume of a continuation whose snapshot was superseded",
        |world| world.try_resume("req_stale_resume"),
    );
    assert_eq!(
        world.task_status("req_after"),
        TaskStatus::Suspended,
        "the task is still exactly where it parked"
    );
}

#[test]
fn carrier_k6_verification_start_refuses_an_unsealed_snapshot() {
    // The second clause of `StaleSnapshot`'s own definition: "or is not sealed where
    // sealing is required". Reached without any graft — `workspace.create` with
    // `seal: false` is the operation that produces an unsealed snapshot.
    let mut world = world();
    let files: Vec<Commitment> = vec![
        world
            .daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new(MODULE_PATH).expect("a workspace path"),
                DIE_HARD_MODEL.as_bytes().to_vec(),
            )
            .expect("staging names its content"),
        world
            .daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new("README.md").expect("a workspace path"),
                b"# unsealed\n".to_vec(),
            )
            .expect("staging names its content"),
    ];
    let configuration = world
        .daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("default.model.toml").expect("a workspace path"),
            DIE_HARD_CONFIG.as_bytes().to_vec(),
        )
        .expect("staging names its content");
    let intent = world.intent.clone();
    let served = world.served.clone();
    let created = world.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "workspace.create",
                "agent:builder",
                "cap_builder",
                "req_unsealed",
            ),
            "idem-unsealed",
        ),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: components(&files, &configuration, &intent, &served),
            overlay: Optional::Absent,
            seal: Optional::Present(false),
        }),
    });
    ok(&created);
    let unsealed = match &created.payload {
        Payload::WorkspaceCreate(response) => {
            assert!(!response.sealed, "the create did not seal it");
            response.snapshot.clone()
        }
        other => panic!("expected a workspace.create payload, got {other:?}"),
    };
    world.known.workspaces.insert(unsealed.clone());
    world.known.commitments.extend(files);
    world.known.commitments.insert(configuration);

    refuses_with_no_effect(
        &mut world,
        ErrorCode::StaleSnapshot,
        "verification.start over an unsealed snapshot",
        |world| world.try_start(&unsealed, "req_unsealed_start"),
    );
}

#[test]
fn carrier_k4_workspace_create_refuses_components_declaring_a_superseded_epoch() {
    // The world moves in a second way here, and it is the one an epoch pin measures: the
    // *deployment* advances. A caller holding a components declaration minted against
    // `semantic-1` presents it to a daemon serving `semantic-2`.
    let mut advanced = epochs();
    advanced.semantic = Nullable::Value(epoch("semantic-2"));
    let mut world = world_serving(&advanced);

    let files: Vec<Commitment> = vec![
        world
            .daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new(MODULE_PATH).expect("a workspace path"),
                DIE_HARD_MODEL.as_bytes().to_vec(),
            )
            .expect("staging names its content"),
    ];
    let configuration = world
        .daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("default.model.toml").expect("a workspace path"),
            DIE_HARD_CONFIG.as_bytes().to_vec(),
        )
        .expect("staging names its content");
    world.known.commitments.extend(files.iter().cloned());
    world.known.commitments.insert(configuration.clone());
    let intent = world.intent.clone();
    // The stale declaration: the epochs the *previous* deployment served.
    let stale = components(&files, &configuration, &intent, &epochs());

    refuses_with_no_effect(
        &mut world,
        ErrorCode::EpochUnsupported,
        "workspace.create declaring a superseded semantic epoch",
        |world| {
            world.dispatch(&OperationRequest {
                envelope: keyed(
                    envelope(
                        "workspace.create",
                        "agent:builder",
                        "cap_builder",
                        "req_stale_epoch",
                    ),
                    "idem-stale-epoch",
                ),
                arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
                    components: stale,
                    overlay: Optional::Absent,
                    seal: Optional::Present(true),
                }),
            })
        },
    );
}

#[test]
fn carrier_k5_evidence_verify_refuses_a_lost_compare_and_set() {
    let mut world = world();
    // The world moves: the claim is verified once, so its status is no longer `proposed`.
    let promoted = world.try_verify(Optional::Absent, "req_promote");
    ok(&promoted);

    // And a caller still holding the pre-promotion view loses the compare-and-set. Nothing
    // is written — the guard is checked before the promotion, not after it.
    refuses_with_no_effect(
        &mut world,
        ErrorCode::StatusConflict,
        "evidence.verify guarded on a superseded status",
        |world| world.try_verify(Optional::Present(EvidenceStatus::Proposed), "req_lost"),
    );
}

#[test]
fn scope_workspace_diff_answers_uniformly_and_so_cannot_be_probed() {
    // `workspace.diff` declares two K2 carriers and consults neither, because the RFC 0031
    // lane has not shipped. That is not staleness handling — it is the absence of a lane —
    // and the honest reading is that the criterion is *undetermined* for this operation
    // rather than satisfied by it. The assertion is that the answer is the same for a
    // current pair and for a superseded one, which is what makes it uninformative.
    let mut world = world();
    let origin = world.origin.clone();
    let head = world.advance(&origin, "# moved on\n", "req_advance");

    let ask = |world: &mut World, before: &WorkspaceHandle, after: &WorkspaceHandle, id: &str| {
        world
            .dispatch(&OperationRequest {
                envelope: budgeted(
                    envelope("workspace.diff", "agent:builder", "cap_builder", id),
                    0,
                ),
                arguments: Arguments::WorkspaceDiff(WorkspaceDiffRequest {
                    before: before.clone(),
                    after: after.clone(),
                    layers: vec![DiffLayer::Textual],
                }),
            })
            .error_code()
    };
    let superseded = ask(&mut world, &origin, &head, "req_diff_stale");
    let current = ask(&mut world, &head, &head, "req_diff_current");
    assert_eq!(superseded, Some(ErrorCode::UnsupportedSemanticFeature));
    assert_eq!(
        superseded, current,
        "the answer does not depend on the currency of what was named, so this operation \
         carries no evidence either way for G2-04"
    );
}

#[test]
fn a_refused_stale_write_is_still_audited() {
    // The fingerprint deliberately excludes the admission ledger. This asserts the other
    // direction, so "outside the instrument" does not silently become "unchecked": a
    // refused stale write is recorded, as RFC 0027 P5 requires whatever the answer was.
    let mut world = world();
    let origin = world.origin.clone();
    world.advance(&origin, "# moved on\n", "req_advance");
    let before = world.daemon.state().admissions().len();
    let refused = world.try_fork(&origin, "req_stale_fork");
    assert_eq!(code(&refused), Some(ErrorCode::StaleSnapshot));
    assert!(
        world.daemon.state().admissions().len() > before,
        "a refused stale write is still an admitted request, and is recorded"
    );
}

// =========================================================================================
// F — recovery sufficiency: is the refusal enough to act on?
// =========================================================================================

/// Every place a refusal could carry a machine-readable specific, as one string.
///
/// `Error.detail` is included deliberately even though `rule envelope.no_prose` makes it a
/// `&'static str`: if the daemon *had* interpolated the head into it, this search would
/// find it, and the finding would be "recoverable, but through prose" rather than "not
/// recoverable". It finds neither.
fn refusal_surface(outcome: &OperationOutcome) -> String {
    format!("{:?} | {:?}", outcome.envelope, outcome.data)
}

#[test]
fn recovery_the_stale_snapshot_refusal_names_neither_the_current_head_nor_a_way_to_find_it() {
    let mut world = world();
    let origin = world.origin.clone();
    let head = world.advance(&origin, "# moved on\n", "req_advance");

    for (what, refused) in [
        ("workspace.fork", world.try_fork(&origin, "req_r_fork")),
        ("workspace.seal", world.try_seal(&origin, "req_r_seal")),
        (
            "verification.start",
            world.try_start(&origin, "req_r_start"),
        ),
        ("task.resume", world.try_resume("req_r_resume")),
    ] {
        assert_eq!(code(&refused), Some(ErrorCode::StaleSnapshot), "{what}");
        let error = refused
            .envelope
            .error
            .value()
            .expect("a refusal carries an error object");

        // RFC 0027 H8: "every row above carries a `recovery` list computed under N2, so
        // 're-seal', 're-base' […] is executable rather than described". It is empty.
        assert!(
            error.recovery.is_empty(),
            "{what}: F1 has been fixed — update this file's verdict"
        );
        assert!(
            error.data.is_absent(),
            "{what}: StaleSnapshot declares no `Error.data` shape"
        );
        assert!(
            refused.envelope.next_operations.is_empty(),
            "{what}: no next operation is offered either"
        );
        assert!(
            !error.retryable,
            "{what}: an identical retry cannot succeed, which is correct and is also why \
             the caller needs something to change *to*"
        );

        // And the identity a caller would have to re-base onto appears nowhere in the
        // whole refusal — not in `detail`, not in `data`, not in any envelope field.
        assert!(
            !refusal_surface(&refused).contains(head.as_str()),
            "{what}: the refusal names the current head somewhere after all — update this \
             file's verdict"
        );
    }
}

#[test]
fn recovery_the_value_layer_computes_the_current_head_that_the_wire_refusal_drops() {
    // F2. This is the load-bearing half of F1: the daemon is not missing a computation, it
    // is discarding one. `check_current` — the exact call `daemon::workspace`,
    // `daemon::task` and `daemon::verification` all make — returns the identity the caller
    // needs, and the wire mapping replaces it with a constant string.
    let mut world = world();
    let origin = world.origin.clone();
    let head = world.advance(&origin, "# moved on\n", "req_advance");

    let stale_identity = source_identity_of(&world.daemon, &origin);
    let current_identity = source_identity_of(&world.daemon, &head);
    let fork = world
        .daemon
        .state()
        .lineage(&world.lineage)
        .expect("the world holds its own lineage")
        .clone();
    let Err(LineageError::Stale(stale)) = check_current(&fork, &stale_identity) else {
        panic!("the superseded identity is `Stale` in its own lineage");
    };
    assert_eq!(stale.stale(), &stale_identity);
    assert_eq!(
        stale.current(),
        &current_identity,
        "the value layer names the identity a caller has to re-derive against"
    );

    // And that value reaches no wire field. Two spellings are searched for, because the
    // identity the lineage holds and the `ws_*` the caller would have to present are
    // different values of the same snapshot — F2's sting: even the discarded value is not
    // directly presentable, so publishing it would need a mapping this protocol has no
    // operation for.
    assert_ne!(
        current_identity.identity(),
        head.as_str(),
        "the lineage head identity and the `ws_*` handle are distinct spellings"
    );
    let refused = world.try_fork(&origin, "req_drop");
    assert_eq!(code(&refused), Some(ErrorCode::StaleSnapshot));
    let surface = refusal_surface(&refused);
    assert!(!surface.contains(current_identity.identity()));
    assert!(!surface.contains(head.as_str()));
}

#[test]
fn recovery_no_servable_operation_reports_a_lineages_current_head() {
    // F3. If the refusal does not carry the head, the remaining question is whether a
    // refused agent can *ask*. It cannot: `task.status` is the only servable read that
    // returns a `WorkspaceHandle` at all, and the one it returns is the task's pinned
    // snapshot — the stale one.
    let mut world = world();
    let origin = world.origin.clone();
    let head = world.advance(&origin, "# moved on\n", "req_advance");
    assert_ne!(head, origin);

    let task = world.task.clone();
    let record = world.dispatch(&OperationRequest {
        envelope: envelope("task.status", "agent:reader", "cap_reader", "req_ts"),
        arguments: Arguments::TaskStatus(TaskStatusRequest { task }),
    });
    ok(&record);
    let Payload::TaskStatus(status) = &record.payload else {
        panic!("expected a task.status payload");
    };
    assert_eq!(
        status.snapshot,
        Nullable::Value(origin.clone()),
        "task.status reports the pinned snapshot, which is precisely the stale one"
    );

    // The two reads a refused agent can otherwise reach, over handles it already holds,
    // and neither names the head.
    let intent = world.intent.clone();
    let got_intent = world.dispatch(&OperationRequest {
        envelope: envelope("intent.get", "agent:reader", "cap_reader", "req_ig"),
        arguments: Arguments::IntentGet(IntentGetRequest { intent }),
    });
    ok(&got_intent);
    let evidence = world.evidence.clone();
    let got_evidence = world.dispatch(&OperationRequest {
        envelope: envelope("evidence.get", "agent:reader", "cap_reader", "req_eg"),
        arguments: Arguments::EvidenceGet(EvidenceGetRequest {
            evidence,
            inline: Optional::Present(true),
        }),
    });
    ok(&got_evidence);
    for (what, outcome) in [
        ("task.status", &record),
        ("intent.get", &got_intent),
        ("evidence.get", &got_evidence),
    ] {
        assert!(
            !format!("{:?} | {:?}", outcome.envelope, outcome.payload).contains(head.as_str()),
            "{what} names the current head after all — update this file's verdict"
        );
    }

    // Stated mechanically as well as by probe: the daemon's own view of the head is
    // reachable from the state surface and from nowhere on the wire. The head it holds is
    // the *source* identity of the record the new `ws_*` names — a third spelling again,
    // and one no response struct in the registry declares a field for.
    assert_eq!(
        world.head(),
        source_identity_of(&world.daemon, &head),
        "the head exists in state; nothing on the wire reports it"
    );
    assert!(
        registry::OPERATIONS
            .iter()
            .filter(|spec| servable().iter().any(|entry| entry.name == spec.name))
            .flat_map(|spec| spec.response.fields)
            .all(|field| field.name != "head" && field.name != "current"),
        "no servable response declares a head-or-current field to carry it in"
    );
}

#[test]
fn recovery_an_epoch_refusal_names_the_daemons_current_epochs_in_a_typed_field() {
    // F4, and the contrast that makes F1 a design omission rather than a limitation of the
    // envelope: for the epoch carrier the recovery channel already exists and is required.
    // `rule envelope.epochs_named` puts all six on *every* result, error included, so the
    // refusal hands back exactly what has to be re-declared.
    let mut advanced = epochs();
    advanced.semantic = Nullable::Value(epoch("semantic-2"));
    let mut world = world_serving(&advanced);

    let files = vec![
        world
            .daemon
            .state_mut()
            .stage(
                &Blake3Identity,
                WorkspacePath::new(MODULE_PATH).expect("a workspace path"),
                DIE_HARD_MODEL.as_bytes().to_vec(),
            )
            .expect("staging names its content"),
    ];
    let configuration = world
        .daemon
        .state_mut()
        .stage(
            &Blake3Identity,
            WorkspacePath::new("default.model.toml").expect("a workspace path"),
            DIE_HARD_CONFIG.as_bytes().to_vec(),
        )
        .expect("staging names its content");
    let intent = world.intent.clone();
    let stale = components(&files, &configuration, &intent, &epochs());
    let refused = world.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "workspace.create",
                "agent:builder",
                "cap_builder",
                "req_epoch",
            ),
            "idem-epoch",
        ),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: stale,
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    });
    assert_eq!(code(&refused), Some(ErrorCode::EpochUnsupported));
    assert_eq!(
        refused.envelope.epochs, advanced,
        "the refusal names every current epoch, so re-declaring needs no second request"
    );
    assert_eq!(
        refused.envelope.epochs.semantic,
        Nullable::Value(epoch("semantic-2")),
        "including the one the caller got wrong"
    );

    // And the recovery works: the same request re-declared against what the refusal named
    // is admitted.
    let fixed = components(&files, &configuration, &intent, &refused.envelope.epochs);
    let accepted = world.dispatch(&OperationRequest {
        envelope: keyed(
            envelope(
                "workspace.create",
                "agent:builder",
                "cap_builder",
                "req_epoch_fixed",
            ),
            "idem-epoch-fixed",
        ),
        arguments: Arguments::WorkspaceCreate(WorkspaceCreateRequest {
            components: fixed,
            overlay: Optional::Absent,
            seal: Optional::Present(true),
        }),
    });
    ok(&accepted);
}

#[test]
fn recovery_a_lost_status_guard_is_recoverable_by_a_declared_re_read() {
    // F5. `StatusConflict`'s own definition is "Re-read and retry", and unlike the snapshot
    // carrier the re-read exists: `evidence.get`, `read` authority, over the handle the
    // caller already presented. So the recovery needs no new wire field and no out-of-band
    // knowledge — the handle *is* the channel.
    let mut world = world();
    ok(&world.try_verify(Optional::Absent, "req_promote"));
    let lost = world.try_verify(Optional::Present(EvidenceStatus::Proposed), "req_lost");
    assert_eq!(code(&lost), Some(ErrorCode::StatusConflict));

    let evidence = world.evidence.clone();
    let reread = world.dispatch(&OperationRequest {
        envelope: envidence_get_envelope("req_reread"),
        arguments: Arguments::EvidenceGet(EvidenceGetRequest {
            evidence,
            inline: Optional::Present(true),
        }),
    });
    ok(&reread);
    let Payload::EvidenceGet(response) = &reread.payload else {
        panic!("expected an evidence.get payload");
    };
    let node = response
        .node
        .value()
        .expect("the node the caller already holds a handle for");
    let rendered = String::from_utf8(node.as_bytes().to_vec()).expect("canonical JSON is UTF-8");
    assert!(
        rendered.contains("observed"),
        "the re-read reports the status the guard should have named: {rendered}"
    );

    // And the corrected guard is admitted, which is the whole of the recovery.
    let retried = world.try_verify(Optional::Present(EvidenceStatus::Observed), "req_retry");
    ok(&retried);
}

fn envidence_get_envelope(request: &str) -> RequestEnvelope {
    envelope("evidence.get", "agent:reader", "cap_reader", request)
}

// =========================================================================================
// G — the complement: what must NOT refuse
// =========================================================================================

#[test]
fn complement_reads_on_explicit_handles_do_not_refuse_after_the_world_moves() {
    // The criterion is not satisfied by a daemon that refuses everything once anything
    // moves. Every servable read a caller can reach with the handles it already holds is
    // driven *after* the lineage advanced, and each must answer.
    let mut world = world();
    let origin = world.origin.clone();
    let head = world.advance(&origin, "# moved on\n", "req_advance");

    let task = world.task.clone();
    let intent = world.intent.clone();
    let evidence = world.evidence.clone();
    for (what, request) in [
        (
            "task.status",
            OperationRequest {
                envelope: envelope("task.status", "agent:reader", "cap_reader", "req_c1"),
                arguments: Arguments::TaskStatus(TaskStatusRequest { task }),
            },
        ),
        (
            "intent.get",
            OperationRequest {
                envelope: envelope("intent.get", "agent:reader", "cap_reader", "req_c2"),
                arguments: Arguments::IntentGet(IntentGetRequest { intent }),
            },
        ),
        (
            "evidence.get",
            OperationRequest {
                envelope: envelope("evidence.get", "agent:reader", "cap_reader", "req_c3"),
                arguments: Arguments::EvidenceGet(EvidenceGetRequest {
                    evidence,
                    inline: Optional::Absent,
                }),
            },
        ),
    ] {
        let outcome = world.dispatch(&request);
        assert_eq!(
            outcome.envelope.status,
            ResultStatus::Ok,
            "{what} consults no world-view and must not refuse: {:?}",
            outcome.envelope.error
        );
    }

    // And the superseded snapshot is still *readable* as an object: a fork off the new head
    // succeeds, so the lineage is alive rather than wedged by the refusals above.
    let next = world.try_fork(&head, "req_c4");
    ok(&next);
}

#[test]
fn complement_expanding_a_pack_pinned_to_a_superseded_snapshot_is_not_stale() {
    // F6, and the sharpest complement in the file. The pack is stated against the origin
    // snapshot. After the lineage advances, that snapshot is superseded — and expanding the
    // pack *still succeeds*, because RFC 0028 requires history to stay readable: "refusing
    // to expand a pack because its snapshot was superseded would make exactly the history
    // the pack exists to explain unreadable".
    //
    // So `context.expand`'s predicate is agreement, not currency. Both predicates answer
    // `StaleSnapshot`, and nothing in the code tells an agent which one refused it.
    let mut world = world();
    let origin = world.origin.clone();
    let head = world.advance(&origin, "# moved on\n", "req_advance");

    let unpinned = world.try_expand(None, "req_x1");
    ok(&unpinned);

    let pinned_to_superseded = world.try_expand(Some(&origin), "req_x2");
    assert_eq!(
        pinned_to_superseded.envelope.status,
        ResultStatus::Ok,
        "a pack stated against a superseded snapshot is history, and stays readable: {:?}",
        pinned_to_superseded.envelope.error
    );

    // And the *current* head is what this operation calls stale, because it disagrees with
    // the pack. The inversion is the finding.
    let pinned_to_current = world.try_expand(Some(&head), "req_x3");
    assert_eq!(
        code(&pinned_to_current),
        Some(ErrorCode::StaleSnapshot),
        "naming the lineage's actual head is what `context.expand` refuses"
    );
}

// =========================================================================================
// H — TOCTOU at the API grain
// =========================================================================================

#[test]
fn toctou_the_write_presenting_the_reads_view_refuses_and_the_same_write_on_a_fresh_view_lands() {
    // Read, world changes, write. Both directions, one test, so neither can pass by the
    // daemon refusing or admitting uniformly.
    let mut world = world();

    // 1. Read: the agent learns the world's state through a real operation.
    let read = world.dispatch(&OperationRequest {
        envelope: envelope("task.status", "agent:reader", "cap_reader", "req_read"),
        arguments: Arguments::TaskStatus(TaskStatusRequest {
            task: world.task.clone(),
        }),
    });
    ok(&read);
    let Payload::TaskStatus(record) = &read.payload else {
        panic!("expected a task.status payload");
    };
    let observed = record
        .snapshot
        .value()
        .cloned()
        .expect("the task pins a snapshot");
    assert_eq!(observed, world.origin);

    // 2. The world changes underneath: another agent forks.
    let origin = world.origin.clone();
    let head = world.advance(&origin, "# a concurrent edit\n", "req_other");

    // 3a. The write presenting the read's view refuses, with no effect.
    let before = world.fingerprint();
    let refused = world.try_seal(&observed, "req_write_stale");
    assert_eq!(code(&refused), Some(ErrorCode::StaleSnapshot));
    assert_eq!(
        before,
        world.fingerprint(),
        "the refused write is not a partial write"
    );

    // 3b. The same write presenting a fresh view lands, and moves the world.
    let accepted = world.try_seal(&head, "req_write_fresh");
    ok(&accepted);
    assert_ne!(
        before,
        world.fingerprint(),
        "the fresh write is a real write, so 3a's equality measured a refusal rather than \
         an inert operation"
    );
}

// =========================================================================================
// I — negative controls
// =========================================================================================

#[test]
fn negative_control_rewinding_the_lineage_lets_the_refused_write_land_and_the_fingerprint_sees_it()
{
    // The control the delivering evidence has none of. It answers two questions at once:
    //
    // - is the currency check what refused the write, or something incidental about the
    //   request? Rewinding the lineage — and changing nothing else — admits it, so the
    //   check is what refused it;
    // - can the fingerprint see a stale write that *lands*? It moves, so an equality
    //   assertion across a refusal is a measurement rather than a tautology.
    //
    // The rewind uses `DaemonState::put_lineage`, the unconditional insert whose misuse by
    // `workspace.create` was bn-n1xou's defect. Here it is deliberate and in-test: the
    // daemon's own source is untouched.
    let mut world = world();
    let origin = world.origin.clone();
    let before_advance = world
        .daemon
        .state()
        .lineage(&world.lineage)
        .expect("the world holds its own lineage")
        .clone();
    world.advance(&origin, "# moved on\n", "req_advance");

    let refused = world.try_fork(&origin, "req_control_refused");
    assert_eq!(code(&refused), Some(ErrorCode::StaleSnapshot));

    // The mutant: the lineage forgets the advance, so the presented view is current again.
    world.daemon.state_mut().put_lineage(before_advance);
    let before = world.fingerprint();
    let landed = world.try_fork(&origin, "req_control_landed");
    ok(&landed);
    let after = world.fingerprint();
    assert_ne!(
        before, after,
        "the write the staleness check withheld is one the fingerprint can see"
    );
    assert_ne!(
        before.lineages, after.lineages,
        "and specifically it advanced the lineage"
    );
    assert_ne!(
        before.workspaces, after.workspaces,
        "and minted a workspace record"
    );
}

#[test]
fn negative_control_a_weak_two_field_predicate_misses_what_the_fingerprint_catches() {
    // The delivering evidence's no-effect predicate is `task.status == Suspended` and
    // `cost.states == 3`. This drives a write that a stale request *did* land — through the
    // rewind above — and shows the weak predicate reports it clean while the fingerprint
    // reports the two rows that moved.
    let mut world = world();
    let origin = world.origin.clone();
    let before_advance = world
        .daemon
        .state()
        .lineage(&world.lineage)
        .expect("the world holds its own lineage")
        .clone();
    world.advance(&origin, "# moved on\n", "req_advance");
    world.daemon.state_mut().put_lineage(before_advance);

    let weak_before = world.task_status("req_weak_before");
    let before = world.fingerprint();
    ok(&world.try_fork(&origin, "req_weak_landed"));
    let weak_after = world.task_status("req_weak_after");
    let after = world.fingerprint();

    assert_eq!(
        weak_before, weak_after,
        "the weak predicate sees nothing: the task never moved"
    );
    assert_eq!(weak_after, TaskStatus::Suspended);
    assert_ne!(
        before, after,
        "the strict instrument sees the write the weak one missed"
    );
}
